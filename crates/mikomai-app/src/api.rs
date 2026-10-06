//! Synchronous submission/query/cancellation with ordered recoverable events.
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use std::{collections::HashMap,sync::{Arc,Mutex,OnceLock}};
use tokio::sync::watch;
use crate::scheduling::{TaskScheduler};
#[derive(Clone,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case")]
pub enum Command {
    Chat {message:String,history:String,documents_dir:String,knowledge_dir:String,attachments:String,devices_json:String,agent:bool},
    ReadDevice {target:String,commands:Vec<String>,timeout_seconds:u32},
    ExecuteApproved {plan_id:String,plan_hash:String},
    Contract {fixture_json:String},
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
pub struct TaskEvent {pub task_id:String,pub seq:u64,pub version:u32,pub kind:String,pub payload:String}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct Snapshot {pub task_id:String,pub state:String,pub seq:u64,pub result:String,pub events:Vec<TaskEvent>}
pub trait EventListener:Send+Sync {fn on_event(&self,event:TaskEvent);}
struct Task {snapshot:Snapshot,cancel:watch::Sender<bool>,resume:watch::Sender<u64>}
pub struct Engine {tasks:Mutex<HashMap<String,Task>>,listeners:Mutex<Vec<Arc<dyn EventListener>>>,pub(crate) scheduler:TaskScheduler}
impl Default for Engine {fn default()->Self {Self{tasks:Mutex::new(HashMap::new()),listeners:Mutex::new(Vec::new()),pub(crate) scheduler:TaskScheduler::default()}}}
pub fn engine()->&'static Arc<Engine> {static ENGINE:OnceLock<Arc<Engine>>=OnceLock::new();ENGINE.get_or_init(||Arc::new(Engine::default()))}
impl Engine {
    pub fn subscribe(&self,listener:Arc<dyn EventListener>){self.listeners.lock().unwrap().push(listener);}
    pub fn query(&self,id:&str)->Result<Snapshot,String> {
        if let Some(task)=self.tasks.lock().map_err(|_|"task state unavailable")?.get(id) {return Ok(task.snapshot.clone());}
        let value=crate::shared_service().load_document("tasks")?.ok_or("task not found")?;
        serde_json::from_value(value["scheduled"][id].clone()).map_err(|_|"task not found or malformed".into())
    }
    fn event(&self,id:&str,kind:&str,payload:String)->Result<(),String> {
        let event={
            let mut tasks=self.tasks.lock().map_err(|_|"task state unavailable")?;
            let task=tasks.get_mut(id).ok_or("task not found")?;
            let mut snapshot=task.snapshot.clone();
            snapshot.seq+=1;
            if ["queued","running","waiting_device","awaiting_user","awaiting_approval","completed","failed","cancelled","unknown"].contains(&kind) {snapshot.state=kind.into();}
            if ["completed","failed","unknown","awaiting_user","awaiting_approval"].contains(&kind) {snapshot.result=payload.clone();}
            let event=TaskEvent{task_id:id.into(),seq:snapshot.seq,version:1,kind:kind.into(),payload};
            snapshot.events.push(event.clone());
            crate::shared_service().update_internal("tasks",|value| {
                if value.get("scheduled").is_none() {value["scheduled"]=json!({});}
                value["scheduled"][id]=serde_json::to_value(&snapshot).map_err(|e|e.to_string())?; Ok(())
            })?;
            task.snapshot=snapshot;event
        };
        // Release all storage/state locks before callbacks; listeners may query/reenter.
        let listeners=self.listeners.lock().map_err(|_|"listeners unavailable")?.clone();
        for listener in listeners {let _=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||listener.on_event(event.clone())));}
        Ok(())
    }
    pub fn submit(self:&Arc<Self>,command:Command)->Result<String,String> {
        let id=uuid::Uuid::new_v4().to_string();let (sender,receiver)=watch::channel(false);let (resume,_)=watch::channel(0);
        self.tasks.lock().map_err(|_|"task state unavailable")?.insert(id.clone(),Task{snapshot:Snapshot{task_id:id.clone(),state:"queued".into(),seq:0,result:String::new(),events:Vec::new()},cancel:sender,resume});
        if let Err(error)=self.event(&id,"queued",String::new()) {self.tasks.lock().unwrap().remove(&id);return Err(error);}
        let engine=self.clone();let task_id=id.clone();
        crate::shared_service().runtime()?.spawn(async move {
            let result=engine.run(&task_id,command,receiver.clone()).await;
            let (state,text)=match result {Ok((state,text))=>(state,text),Err(error)=>if error.starts_with("unknown:") {("unknown".into(),error)} else if *receiver.borrow() {("cancelled".into(),String::new())} else {("failed".into(),error)}};
            if let Err(error)=engine.event(&task_id,&state,text) {
                if let Some(task)=engine.tasks.lock().unwrap().get_mut(&task_id) {task.snapshot.state="failed".into();task.snapshot.result=format!("task result could not be saved: {error}");}
            }
        });
        Ok(id)
    }
    pub fn resume(&self,id:&str)->Result<(),String> {
        let tasks=self.tasks.lock().map_err(|_|"task state unavailable")?;
        let task=tasks.get(id).ok_or("task not found")?;
        if task.snapshot.state!="awaiting_user" {return Err("task is not waiting for a lock decision".into());}
        task.resume.send_modify(|generation|*generation+=1); Ok(())
    }
    pub(crate) async fn wait_device(&self,id:&str,key:&str,write:bool,serial:bool,cancel:&mut watch::Receiver<bool>)->Result<crate::scheduling::DeviceLease,String> {
        let mut resume=self.tasks.lock().map_err(|_|"task state unavailable")?.get(id).ok_or("task not found")?.resume.subscribe();
        let settings=crate::shared_service().load_document("settings")?.unwrap_or(json!({}));
        let timeout=settings[if write {"deviceWriteLockWaitSeconds"} else {"deviceReadLockWaitSeconds"}].as_u64().filter(|n|*n>0&&*n<=3600).unwrap_or(if write {300} else {60});
        loop {
            self.event(id,"waiting_device",String::new())?;
            let waiting=crate::shared_service().device_locks.get_or_init(Default::default).acquire(key,write,serial,false,crate::portable_app_data_dir()?.join("locks"),cancel);
            match tokio::time::timeout(std::time::Duration::from_secs(timeout),waiting).await {
                Ok(result)=>{
                    let lease=result?;
                    lock_audit(key,"lock_acquired",lease.waited.as_millis() as u64)?;
                    return Ok(lease);
                },
                Err(_)=>{
                    lock_audit(key,"lock_wait_timeout",timeout*1000)?;
                    self.event(id,"awaiting_user",json!({"reason":"device_lock_timeout","device":key,"wait_seconds":timeout,"message":"機器のロック待ちが続いています。待機を続けるか中止してください。"}).to_string())?;
                    if *cancel.borrow() {return Err("cancelled".into());}
                    tokio::select! {_=resume.changed()=>{},_=cancel.changed()=>return Err("cancelled".into())}
                }
            }
        }
    }
    pub fn cancel(&self,id:&str)->Result<(),String> {
        self.tasks.lock().map_err(|_|"task state unavailable")?.get(id).ok_or("task not found")?.cancel.send(true).map_err(|_|"task has ended".into())
    }
    async fn run(self:&Arc<Self>,id:&str,command:Command,mut cancel:watch::Receiver<bool>)->Result<(String,String),String> {
        let mut permit=Some(self.scheduler.acquire(&mut cancel).await?);
        self.event(id,"running",String::new())?;
        match command {
            Command::Contract{fixture_json}=>{
                let fixture:Value=serde_json::from_str(&fixture_json).map_err(|e|e.to_string())?;
                for state in fixture["states"].as_array().ok_or("contract states missing")? {
                    let state=state.as_str().ok_or("invalid contract state")?;
                    if matches!(state,"awaiting_user"|"awaiting_approval"|"waiting_device") {permit.take();self.event(id,state,String::new())?;}
                    else if state=="running" {if permit.is_none(){permit=Some(self.scheduler.acquire(&mut cancel).await?);}self.event(id,state,String::new())?;}
                    else {return Err("invalid contract transition".into());}
                }
                Ok(("completed".into(),fixture["result"].as_str().unwrap_or("").into()))
            },
            Command::ReadDevice{target,commands,timeout_seconds}=>{
                let connection=crate::native_execution::connection(&target)?;
                permit.take();self.event(id,"waiting_device",String::new())?;
                let result=crate::native_execution::read_task(id,&connection,commands,timeout_seconds,&mut cancel,&self.scheduler).await?;
                if result.status=="completed" {Ok(("completed".into(),result.payload.to_string()))} else {Ok((result.status,result.payload.to_string()))}
            },
            Command::ExecuteApproved{plan_id,plan_hash}=>{
                // Lock waiting never occupies a scheduler slot.
                permit.take();self.event(id,"waiting_device",String::new())?;
                let task_cancel=cancel.clone();let task_id=id.to_string();
                let result=tokio::task::spawn_blocking(move||{let _scope=TaskContext::enter(task_id);crate::native_execution::execute_approved_with_cancel(&plan_id,&plan_hash,task_cancel)}).await.map_err(|_|"operation worker failed")??;
                Ok(("completed".into(),result))
            },
            Command::Chat{message,history,documents_dir,knowledge_dir,attachments,devices_json,agent}=>{
                permit.take();
                permit=Some(self.scheduler.acquire(&mut cancel).await?);
                let sink:Arc<dyn crate::owned_bridge::LegacyListener>=Arc::new(ChatSink{engine:self.clone(),id:id.into()});
                let args=vec![message,history,documents_dir,knowledge_dir,attachments,devices_json];
                let task_cancel=cancel.clone();let task_id=id.to_string();
                let mut job=tokio::task::spawn_blocking(move||{let _task_scope=TaskContext::enter(task_id);let _scope=crate::scheduling::CancellationScope::enter(task_cancel);crate::owned_bridge::invoke(if agent {"mikomai_agent_chat_streaming"} else {"mikomai_assistant_chat_streaming"},&args,Some(sink))});
                let response=tokio::select! {response=&mut job=>response.map_err(|_|"chat worker failed")?,_=cancel.changed()=>{let _=job.await;return Err("cancelled".into());}}?;
                let state=if response.contains("__ASK_HUMAN__") {"awaiting_user"} else if response.contains("__MIKOMAI_APPROVAL_PLAN__")||response.contains("承認") {"awaiting_approval"} else {"completed"};
                drop(permit);
                Ok((state.into(),response))
            }
        }
    }
}
struct ChatSink {engine:Arc<Engine>,id:String}
impl crate::owned_bridge::LegacyListener for ChatSink {fn event(&self,_kind:String,text:String,done:bool) {
    let kind=if text.starts_with("__MIKOMAI_DEBUG__") {"debug"} else if text.starts_with("__MIKOMAI_APPROVAL_PLAN__") {"operation_plan"} else {"stream"};
    if self.engine.event(&self.id,kind,json!({"text":text,"done":done}).to_string()).is_err() {let _=self.engine.cancel(&self.id);}
}}

thread_local! {static TASK_ID:std::cell::RefCell<Option<String>>=const {std::cell::RefCell::new(None)};}
pub(crate) struct TaskContext(Option<String>);
impl TaskContext {pub(crate) fn enter(id:String)->Self{Self(TASK_ID.with(|value|value.replace(Some(id))))}}
impl Drop for TaskContext {fn drop(&mut self){TASK_ID.with(|value|{value.replace(self.0.take());});}}
pub(crate) fn current_task_id()->Option<String>{TASK_ID.with(|value|value.borrow().clone())}
pub(crate) fn lock_audit(key:&str,phase:&str,waited_ms:u64)->Result<(),String>{
    crate::operation_audit_log()?.append(&mikomai_core::audit::record("device_lock".into(),Some(key.into()),mikomai_core::domain::OperationClass::ReadOnly,phase,&json!({"waited_ms":waited_ms})))
}
