use std::ffi::CString;

pub trait LegacyListener: Send + Sync { fn event(&self,kind:String,text:String,done:bool); }
pub fn invoke(op:&str,args:&[String],listener:Option<std::sync::Arc<dyn LegacyListener>>)->Result<String,String> {
if op=="mikomai_operation_execute_approved" { return crate::native_execution::execute_approved(args.first().ok_or("id missing")?,args.get(1).ok_or("hash missing")?); }
let strings:Vec<CString>=args.iter().map(|s|CString::new(s.as_str()).map_err(|e|e.to_string())).collect::<Result<_,_>>()?;
let p=|i:usize| strings.get(i).map(|s|s.as_ptr()).unwrap_or(std::ptr::null());
let n=|i:usize| args.get(i).and_then(|s|s.parse::<f64>().ok()).unwrap_or(0.);
let retained;
let listener=if op=="mikomai_watch_start" {retained=listener.clone(); if let Some(l)=retained { crate::shared_service().watch_listener.set(l).map_err(|_|"watch listener already installed")?; } crate::shared_service().watch_listener.get().cloned()} else {listener};
let context=if op=="mikomai_watch_start" {crate::shared_service().watch_listener.get().map(|l|l as *const std::sync::Arc<dyn LegacyListener> as *mut std::ffi::c_void).unwrap_or(std::ptr::null_mut())} else {listener.as_ref().map(|l|l as *const std::sync::Arc<dyn LegacyListener> as *mut std::ffi::c_void).unwrap_or(std::ptr::null_mut())};
let raw=unsafe { match op {
"mikomai_dispatch_mode" => crate::mikomai_dispatch_mode(p(0),p(1)),
"mikomai_agent_chat_streaming" => crate::mikomai_agent_chat_streaming(p(0),p(1),p(2),p(3),p(4),p(5),Some(stream_callback),Some(crate::native_execution::tool_callback),Some(crate::native_execution::plan_callback),context),
"mikomai_agent_task_list" => crate::mikomai_agent_task_list(),
"mikomai_agent_task_history" => crate::mikomai_agent_task_history(p(0)),
"mikomai_agent_task_delete" => crate::mikomai_agent_task_delete(p(0)),
"mikomai_agent_task_delete_all" => crate::mikomai_agent_task_delete_all(),
"mikomai_watch_start" => crate::mikomai_watch_start(p(0),Some(crate::native_execution::tool_callback),Some(notification_callback),context),
"mikomai_watch_stop" => crate::mikomai_watch_stop(),
"mikomai_watch_list" => crate::mikomai_watch_list(),
"mikomai_watch_create" => crate::mikomai_watch_create(p(0)),
"mikomai_watch_update" => crate::mikomai_watch_update(p(0),p(1)),
"mikomai_watch_enable" => crate::mikomai_watch_enable(p(0)),
"mikomai_watch_disable" => crate::mikomai_watch_disable(p(0)),
"mikomai_watch_delete" => crate::mikomai_watch_delete(p(0)),
"mikomai_watch_run_now" => crate::mikomai_watch_run_now(p(0)),
"mikomai_chat" => crate::mikomai_chat(p(0)),
"mikomai_chat_with_paths" => crate::mikomai_chat_with_paths(p(0),p(1),p(2)),
"mikomai_model_load" => crate::mikomai_model_load(p(0)),
"mikomai_model_status" => crate::mikomai_model_status(),
"mikomai_model_select_backend" => crate::mikomai_model_select_backend(p(0)),
"mikomai_model_cancel" => crate::mikomai_model_cancel(),
"mikomai_configure_vision" => crate::mikomai_configure_vision(n(0) as i32,p(1)),
"mikomai_device_registry_read" => crate::mikomai_device_registry_read(p(0)),
"mikomai_assistant_chat" => crate::mikomai_assistant_chat(p(0),p(1),p(2),p(3)),
"mikomai_assistant_chat_with_attachments" => crate::mikomai_assistant_chat_with_attachments(p(0),p(1),p(2),p(3),p(4)),
"mikomai_assistant_chat_streaming" => crate::mikomai_assistant_chat_streaming(p(0),p(1),p(2),p(3),p(4),Some(stream_callback),context),
"mikomai_test_tcp_connection" => crate::mikomai_test_tcp_connection(p(0),n(1) as u16,n(2) as u32),
"mikomai_set_inference_params" => crate::mikomai_set_inference_params(n(0) as f32,n(1) as f32,n(2) as u32,n(3) as u32),
"mikomai_operation_plan_create" => crate::mikomai_operation_plan_create(p(0),p(1),p(2),p(3)),
"mikomai_operation_plan_create_generic" => crate::mikomai_operation_plan_create_generic(p(0),p(1),p(2),p(3),p(4)),
"mikomai_operation_plan_approve" => crate::mikomai_operation_plan_approve(p(0),p(1)),
"mikomai_operation_plan_get" => crate::mikomai_operation_plan_get(p(0)),
"mikomai_operation_audit_list" => crate::mikomai_operation_audit_list(),
"mikomai_operation_plan_begin" => crate::mikomai_operation_plan_begin(p(0),p(1)),
"mikomai_operation_plan_finish" => crate::mikomai_operation_plan_finish(p(0),n(1) as i32),
"mikomai_operation_execute_approved" => crate::mikomai_operation_execute_approved(p(0),p(1),p(2)),
"mikomai_native_query" => crate::mikomai_native_query(p(0)),
_=>return Err("unsupported bridge operation".into()),
}};
crate::consume_result(raw)
}
extern "C" fn stream_callback(text:*const std::ffi::c_char,done:i32,context:*mut std::ffi::c_void) {
unsafe { if !context.is_null() { let sink=&*(context as *const std::sync::Arc<dyn LegacyListener>); let text=std::ffi::CStr::from_ptr(text).to_string_lossy().into_owned();sink.event("stream".into(),text,done!=0); }}
}
extern "C" fn notification_callback(text:*const std::ffi::c_char,context:*mut std::ffi::c_void) {
unsafe { if !context.is_null() { let sink=&*(context as *const std::sync::Arc<dyn LegacyListener>); sink.event("notification".into(),std::ffi::CStr::from_ptr(text).to_string_lossy().into_owned(),false); }}
}