#ifndef MIKOMAI_FFI_H
#define MIKOMAI_FFI_H

#include <stdint.h>

typedef struct {
    int32_t status;
    char *message;
} MikomaiResult;

typedef void (*MikomaiStreamCallback)(const char *chunk, int32_t is_done, void *context);
typedef int32_t (*MikomaiToolCallback)(const char *tool_id, const char *target_json, const char *args_json, char *output, uintptr_t output_capacity, void *context);
typedef int32_t (*MikomaiPlanCallback)(const char *target, const char *tool_id, const char *args_json, const char *rationale, char *output, uintptr_t output_capacity, void *context);
typedef void (*MikomaiWatchNotificationCallback)(const char *notification_json, void *context);

MikomaiResult mikomai_dispatch_mode(const char *message, const char *devices_json);
MikomaiResult mikomai_agent_chat_streaming(const char *message, const char *history, const char *documents_dir, const char *knowledge_dir, const char *attachments, const char *devices_json, MikomaiStreamCallback callback, MikomaiToolCallback tool_callback, MikomaiPlanCallback plan_callback, void *context);
MikomaiResult mikomai_agent_task_list(void);
MikomaiResult mikomai_agent_task_history(const char *id);
MikomaiResult mikomai_watch_start(const char *storage_path, MikomaiToolCallback tool_callback, MikomaiWatchNotificationCallback notification_callback, void *context);
MikomaiResult mikomai_watch_stop(void);
MikomaiResult mikomai_watch_list(void);
MikomaiResult mikomai_watch_create(const char *request_json);
MikomaiResult mikomai_watch_update(const char *id, const char *request_json);
MikomaiResult mikomai_watch_enable(const char *id);
MikomaiResult mikomai_watch_disable(const char *id);
MikomaiResult mikomai_watch_delete(const char *id);
MikomaiResult mikomai_watch_run_now(const char *id);

MikomaiResult mikomai_chat(const char *message);
MikomaiResult mikomai_chat_with_paths(const char *message, const char *documents_dir, const char *knowledge_dir);
MikomaiResult mikomai_model_load(const char *path);
MikomaiResult mikomai_model_status(void);
MikomaiResult mikomai_model_select_backend(const char *name);
MikomaiResult mikomai_model_cancel(void);
MikomaiResult mikomai_configure_vision(int32_t enabled, const char *projector_path);
MikomaiResult mikomai_device_registry_read(const char *path);
MikomaiResult mikomai_assistant_chat(const char *message, const char *history, const char *documents_dir, const char *knowledge_dir);
/* Legacy text or __MIKOMAI_ATTACHMENTS_V1__ JSON with text and PNG/JPEG images. */
MikomaiResult mikomai_assistant_chat_with_attachments(const char *message, const char *history, const char *documents_dir, const char *knowledge_dir, const char *attachments);
MikomaiResult mikomai_assistant_chat_streaming(const char *message, const char *history, const char *documents_dir, const char *knowledge_dir, const char *attachments, MikomaiStreamCallback callback, void *context);
MikomaiResult mikomai_test_tcp_connection(const char *host, uint16_t port, uint32_t timeout_ms);
MikomaiResult mikomai_set_inference_params(float temperature, float repetition_penalty, uint32_t n_ctx, uint32_t max_new_tokens);
MikomaiResult mikomai_operation_plan_create(const char *target, const char *target_snapshot_json, const char *commands_json, const char *rationale);
MikomaiResult mikomai_operation_plan_create_generic(const char *target, const char *tool_id, const char *target_snapshot_json, const char *args_json, const char *rationale);
MikomaiResult mikomai_operation_plan_approve(const char *id, const char *plan_hash);
MikomaiResult mikomai_operation_plan_get(const char *id);
MikomaiResult mikomai_operation_audit_list(void);
MikomaiResult mikomai_operation_plan_begin(const char *id, const char *plan_hash);
MikomaiResult mikomai_operation_plan_finish(const char *id, int32_t succeeded);
MikomaiResult mikomai_operation_execute_approved(const char *id, const char *plan_hash, const char *credentials_json);
void mikomai_result_free(MikomaiResult result);

#endif
