//! Stable C ABI compatibility layer. Application state lives in mikomai-app.
pub use mikomai_app::{
    load_local_model, local_arp_state, local_model_chat, local_model_chat_with_callback,
    test_tcp_connection_core,
};
pub use mikomai_app::{
    MikomaiOperationCompletionCallback, MikomaiPlanCallback, MikomaiResult, MikomaiStreamCallback,
    MikomaiToolCallback, MikomaiWatchNotificationCallback,
};
use std::ffi::c_char;

#[no_mangle]
pub unsafe extern "C" fn mikomai_operation_plan_create(
    target: *const c_char,
    target_snapshot_json: *const c_char,
    commands_json: *const c_char,
    rationale: *const c_char,
) -> MikomaiResult {
    mikomai_app::mikomai_operation_plan_create(
        target,
        target_snapshot_json,
        commands_json,
        rationale,
    )
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_operation_plan_create_generic(
    target: *const c_char,
    tool_id: *const c_char,
    target_snapshot_json: *const c_char,
    args_json: *const c_char,
    rationale: *const c_char,
) -> MikomaiResult {
    mikomai_app::mikomai_operation_plan_create_generic(
        target,
        tool_id,
        target_snapshot_json,
        args_json,
        rationale,
    )
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_operation_plan_approve(
    id: *const c_char,
    hash: *const c_char,
) -> MikomaiResult {
    mikomai_app::mikomai_operation_plan_approve(id, hash)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_operation_plan_get(id: *const c_char) -> MikomaiResult {
    mikomai_app::mikomai_operation_plan_get(id)
}

#[no_mangle]
pub extern "C" fn mikomai_operation_audit_list() -> MikomaiResult {
    mikomai_app::mikomai_operation_audit_list()
}

#[no_mangle]
pub extern "C" fn mikomai_agent_task_list() -> MikomaiResult {
    mikomai_app::mikomai_agent_task_list()
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_agent_task_history(id: *const c_char) -> MikomaiResult {
    mikomai_app::mikomai_agent_task_history(id)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_agent_task_delete(id: *const c_char) -> MikomaiResult {
    mikomai_app::mikomai_agent_task_delete(id)
}

#[no_mangle]
pub extern "C" fn mikomai_agent_task_delete_all() -> MikomaiResult {
    mikomai_app::mikomai_agent_task_delete_all()
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_operation_plan_begin(
    id: *const c_char,
    hash: *const c_char,
) -> MikomaiResult {
    mikomai_app::mikomai_operation_plan_begin(id, hash)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_operation_plan_finish(
    id: *const c_char,
    succeeded: i32,
) -> MikomaiResult {
    mikomai_app::mikomai_operation_plan_finish(id, succeeded)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_operation_execute_approved(
    id: *const c_char,
    hash: *const c_char,
    credentials_json: *const c_char,
) -> MikomaiResult {
    mikomai_app::mikomai_operation_execute_approved(id, hash, credentials_json)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_operation_execute_approved_async(
    id: *const c_char,
    hash: *const c_char,
    credentials_json: *const c_char,
    callback: Option<MikomaiOperationCompletionCallback>,
    context: *mut std::ffi::c_void,
) -> MikomaiResult {
    mikomai_app::mikomai_operation_execute_approved_async(
        id,
        hash,
        credentials_json,
        callback,
        context,
    )
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_set_inference_params(
    temperature: f32,
    repetition_penalty: f32,
    n_ctx: u32,
    max_new_tokens: u32,
) -> MikomaiResult {
    mikomai_app::mikomai_set_inference_params(
        temperature,
        repetition_penalty,
        n_ctx,
        max_new_tokens,
    )
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_model_load(path: *const c_char) -> MikomaiResult {
    mikomai_app::mikomai_model_load(path)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_model_status() -> MikomaiResult {
    mikomai_app::mikomai_model_status()
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_model_select_backend(name: *const c_char) -> MikomaiResult {
    mikomai_app::mikomai_model_select_backend(name)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_model_cancel() -> MikomaiResult {
    mikomai_app::mikomai_model_cancel()
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_device_registry_read(path: *const c_char) -> MikomaiResult {
    mikomai_app::mikomai_device_registry_read(path)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_model_chat(prompt: *const c_char) -> MikomaiResult {
    mikomai_app::mikomai_model_chat(prompt)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_assistant_chat(
    message: *const c_char,
    history: *const c_char,
    documents_dir: *const c_char,
    knowledge_dir: *const c_char,
) -> MikomaiResult {
    mikomai_app::mikomai_assistant_chat(message, history, documents_dir, knowledge_dir)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_dispatch_mode(
    message: *const c_char,
    devices_json: *const c_char,
) -> MikomaiResult {
    mikomai_app::mikomai_dispatch_mode(message, devices_json)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_watch_start(
    storage_path: *const c_char,
    tool_callback: Option<MikomaiToolCallback>,
    notification_callback: Option<MikomaiWatchNotificationCallback>,
    context: *mut std::ffi::c_void,
) -> MikomaiResult {
    mikomai_app::mikomai_watch_start(storage_path, tool_callback, notification_callback, context)
}

#[no_mangle]
pub extern "C" fn mikomai_watch_stop() -> MikomaiResult {
    mikomai_app::mikomai_watch_stop()
}

#[no_mangle]
pub extern "C" fn mikomai_watch_list() -> MikomaiResult {
    mikomai_app::mikomai_watch_list()
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_watch_create(request_json: *const c_char) -> MikomaiResult {
    mikomai_app::mikomai_watch_create(request_json)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_watch_update(
    id: *const c_char,
    request_json: *const c_char,
) -> MikomaiResult {
    mikomai_app::mikomai_watch_update(id, request_json)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_watch_enable(id: *const c_char) -> MikomaiResult {
    mikomai_app::mikomai_watch_enable(id)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_watch_disable(id: *const c_char) -> MikomaiResult {
    mikomai_app::mikomai_watch_disable(id)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_watch_delete(id: *const c_char) -> MikomaiResult {
    mikomai_app::mikomai_watch_delete(id)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_watch_run_now(id: *const c_char) -> MikomaiResult {
    mikomai_app::mikomai_watch_run_now(id)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_agent_chat_streaming(
    message: *const c_char,
    history: *const c_char,
    documents_dir: *const c_char,
    knowledge_dir: *const c_char,
    attachments: *const c_char,
    devices_json: *const c_char,
    callback: Option<MikomaiStreamCallback>,
    tool_callback: Option<MikomaiToolCallback>,
    plan_callback: Option<MikomaiPlanCallback>,
    context: *mut std::ffi::c_void,
) -> MikomaiResult {
    mikomai_app::mikomai_agent_chat_streaming(
        message,
        history,
        documents_dir,
        knowledge_dir,
        attachments,
        devices_json,
        callback,
        tool_callback,
        plan_callback,
        context,
    )
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_assistant_chat_with_attachments(
    message: *const c_char,
    history: *const c_char,
    documents_dir: *const c_char,
    knowledge_dir: *const c_char,
    attachments: *const c_char,
) -> MikomaiResult {
    mikomai_app::mikomai_assistant_chat_with_attachments(
        message,
        history,
        documents_dir,
        knowledge_dir,
        attachments,
    )
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_assistant_chat_streaming(
    message: *const c_char,
    history: *const c_char,
    documents_dir: *const c_char,
    knowledge_dir: *const c_char,
    attachments: *const c_char,
    callback: Option<MikomaiStreamCallback>,
    context: *mut std::ffi::c_void,
) -> MikomaiResult {
    mikomai_app::mikomai_assistant_chat_streaming(
        message,
        history,
        documents_dir,
        knowledge_dir,
        attachments,
        callback,
        context,
    )
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_test_tcp_connection(
    host: *const c_char,
    port: u16,
    timeout_ms: u32,
) -> MikomaiResult {
    mikomai_app::mikomai_test_tcp_connection(host, port, timeout_ms)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_chat(message: *const c_char) -> MikomaiResult {
    mikomai_app::mikomai_chat(message)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_chat_with_paths(
    message: *const c_char,
    documents_dir: *const c_char,
    knowledge_dir: *const c_char,
) -> MikomaiResult {
    mikomai_app::mikomai_chat_with_paths(message, documents_dir, knowledge_dir)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_result_free(result: MikomaiResult) {
    mikomai_app::mikomai_result_free(result)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_configure_vision(
    enabled: i32,
    projector_path: *const c_char,
) -> MikomaiResult {
    mikomai_app::mikomai_configure_vision(enabled, projector_path)
}

#[no_mangle]
pub unsafe extern "C" fn mikomai_native_query(request: *const c_char) -> MikomaiResult {
    mikomai_app::mikomai_native_query(request)
}
