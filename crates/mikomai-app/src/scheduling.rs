//! Fair execution slots, priority inference, and per-device/advisory exclusion.
use std::{collections::HashMap,fs::{File,OpenOptions},path::PathBuf,sync::{Arc,Mutex},time::{Duration,Instant}};
use tokio::sync::{Notify,OwnedRwLockReadGuard,OwnedRwLockWriteGuard,RwLock,Semaphore,OwnedSemaphorePermit,watch};
use sha2::{Digest,Sha256};
use fs2::FileExt;

pub struct TaskScheduler(Arc<Semaphore>);
impl Default for TaskScheduler { fn default()->Self { Self(Arc::new(Semaphore::new(5))) } }
impl TaskScheduler {
    pub async fn acquire(&self,cancel:&mut watch::Receiver<bool>)->Result<OwnedSemaphorePermit,String> {
        if *cancel.borrow() {return Err("cancelled".into());}
        tokio::select! { p=self.0.clone().acquire_owned()=>p.map_err(|_|"scheduler closed".into()), _=cancel.changed()=>Err("cancelled".into()) }
    }
}
#[derive(Clone,Copy,Eq,PartialEq,Ord,PartialOrd)]
pub enum Priority { Watch, Planner, FinalAnswer }
struct InferenceState { active:bool, active_id:u64, next:u64, queued:Vec<(u64,Priority)> }
pub struct InferenceQueue { state:Mutex<InferenceState>,changed:Notify }
impl Default for InferenceQueue {fn default()->Self {Self{state:Mutex::new(InferenceState{active:false,active_id:0,next:0,queued:Vec::new()}),changed:Notify::new()}}}
pub struct InferenceLease(Arc<InferenceQueue>,u64,Option<tokio::task::JoinHandle<()>>);
impl Drop for InferenceLease { fn drop(&mut self) { if let Some(job)=self.2.take(){job.abort();} self.0.state.lock().unwrap().active=false; self.0.changed.notify_waiters(); } }
struct WaitingInference { queue:Arc<InferenceQueue>,id:u64,claimed:bool }
impl Drop for WaitingInference {fn drop(&mut self){if !self.claimed {self.queue.state.lock().unwrap().queued.retain(|q|q.0!=self.id);self.queue.changed.notify_waiters();}}}
impl InferenceQueue {
    pub async fn acquire(self:&Arc<Self>,priority:Priority,cancel:&mut watch::Receiver<bool>)->Result<InferenceLease,String> {
        let id={let mut s=self.state.lock().unwrap();s.next+=1;let id=s.next;s.queued.push((id,priority));id};
        let mut waiting=WaitingInference{queue:self.clone(),id,claimed:false};
        loop {
            let changed=self.changed.notified();
            tokio::pin!(changed); changed.as_mut().enable();
            if *cancel.borrow() { return Err("cancelled".into()); }
            {
                let mut s=self.state.lock().unwrap();
                let best=s.queued.iter().max_by_key(|(id,p)|(*p,std::cmp::Reverse(*id))).map(|q|q.0);
                if !s.active && best==Some(id) {s.active=true;s.active_id=id;s.queued.retain(|q|q.0!=id);waiting.claimed=true;return Ok(InferenceLease(self.clone(),id,None));}
            }
            tokio::select! {_=changed.as_mut()=>{},_=cancel.changed()=>return Err("cancelled".into())}
        }
    }
}
#[derive(Default)]
pub struct DeviceLockManager { devices:Mutex<HashMap<String,Arc<RwLock<()>>>> }
pub enum LocalGuard { Read(OwnedRwLockReadGuard<()>),Write(OwnedRwLockWriteGuard<()>) }
pub struct DeviceLease { _local:LocalGuard, _file:Option<File>,pub waited:Duration }
impl Drop for DeviceLease { fn drop(&mut self){if let Some(file)=&self._file {let _=FileExt::unlock(file);}} }
impl DeviceLockManager {
    pub async fn acquire(&self,key:&str,write:bool,serial:bool,watch_run:bool,lock_root:PathBuf,cancel:&mut watch::Receiver<bool>)->Result<DeviceLease,String> {
        if *cancel.borrow() {return Err("cancelled".into());}
        let start=Instant::now(); let exclusive=write||serial;
        let lock=self.devices.lock().map_err(|_|"device locks poisoned")?.entry(key.into()).or_insert_with(||Arc::new(RwLock::new(()))).clone();
        let local=if watch_run {
            if exclusive {LocalGuard::Write(lock.try_write_owned().map_err(|_|"watch skipped: device busy")?)} else {LocalGuard::Read(lock.try_read_owned().map_err(|_|"watch skipped: device busy")?)}
        } else if exclusive {
            LocalGuard::Write(tokio::select! {g=lock.write_owned()=>g,_=cancel.changed()=>return Err("cancelled".into())})
        } else {LocalGuard::Read(tokio::select! {g=lock.read_owned()=>g,_=cancel.changed()=>return Err("cancelled".into())})};
        let file=if exclusive {
            std::fs::create_dir_all(&lock_root).map_err(|e|e.to_string())?;
            let path=lock_root.join(format!("{:x}.lock",Sha256::digest(key.as_bytes())));
            let file=OpenOptions::new().create(true).truncate(false).read(true).write(true).open(path).map_err(|e|e.to_string())?;
            loop {
                match file.try_lock_exclusive() {
                    Ok(())=>break,
                    Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>{
                        if watch_run {return Err("watch skipped: device busy in another process".into());}
                        tokio::select! {_=tokio::time::sleep(Duration::from_millis(25))=>{},_=cancel.changed()=>return Err("cancelled".into())}
                    },Err(e)=>return Err(e.to_string())
                }
            }
            Some(file)
        } else {None};
        Ok(DeviceLease{_local:local,_file:file,waited:start.elapsed()})
    }
}

thread_local! {static CANCELLATION:std::cell::RefCell<Option<watch::Receiver<bool>>>=const{std::cell::RefCell::new(None)};}
pub struct CancellationScope(Option<watch::Receiver<bool>>);
impl CancellationScope {pub fn enter(cancel:watch::Receiver<bool>)->Self {Self(CANCELLATION.with(|c|c.replace(Some(cancel))))}}
impl Drop for CancellationScope {fn drop(&mut self){CANCELLATION.with(|c|{c.replace(self.0.take());});}}
pub fn inference_lease(priority:Priority)->Result<InferenceLease,String> {
    static QUEUE:std::sync::OnceLock<Arc<InferenceQueue>>=std::sync::OnceLock::new();
    let queue=QUEUE.get_or_init(||Arc::new(InferenceQueue::default()));
    let (_sender,default)=watch::channel(false);
    let mut cancel=CANCELLATION.with(|c|c.borrow().clone()).unwrap_or(default);
    let mut lease=crate::shared_service().run(queue.acquire(priority,&mut cancel))??;
    crate::llm_runtime::reset_cancellation();
    let active_id=lease.1;
    let observed=queue.clone();
    lease.2=Some(crate::shared_service().runtime()?.spawn(async move {
        if cancel.changed().await.is_ok() && *cancel.borrow() {
            let state=observed.state.lock().unwrap();
            if state.active && state.active_id==active_id {crate::llm_runtime::cancel();}
        }
    }));
    Ok(lease)
}

pub fn current_cancellation()->Option<watch::Receiver<bool>> {CANCELLATION.with(|c|c.borrow().clone())}
