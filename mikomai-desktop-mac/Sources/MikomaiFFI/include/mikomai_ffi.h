#ifndef MIKOMAI_FFI_H
#define MIKOMAI_FFI_H

#include <stdint.h>

typedef struct {
    int32_t status;
    char *message;
} MikomaiResult;

typedef void (*MikomaiStreamCallback)(const char *chunk, int32_t is_done, void *context);

MikomaiResult mikomai_chat(const char *message);
MikomaiResult mikomai_chat_with_paths(const char *message, const char *documents_dir, const char *knowledge_dir);
MikomaiResult mikomai_model_load(const char *path);
MikomaiResult mikomai_model_status(void);
MikomaiResult mikomai_model_cancel(void);
MikomaiResult mikomai_device_registry_read(const char *path);
MikomaiResult mikomai_assistant_chat(const char *message, const char *history, const char *documents_dir, const char *knowledge_dir);
/* Text attachments only. Image, PDF, and non-text file markers are rejected; vision inference is not supported. */
MikomaiResult mikomai_assistant_chat_with_attachments(const char *message, const char *history, const char *documents_dir, const char *knowledge_dir, const char *attachments);
MikomaiResult mikomai_assistant_chat_streaming(const char *message, const char *history, const char *documents_dir, const char *knowledge_dir, const char *attachments, MikomaiStreamCallback callback, void *context);
MikomaiResult mikomai_test_tcp_connection(const char *host, uint16_t port, uint32_t timeout_ms);
MikomaiResult mikomai_set_inference_params(float temperature, float repetition_penalty, uint32_t n_ctx, uint32_t max_new_tokens);
MikomaiResult mikomai_operation_plan_create(const char *target, const char *target_snapshot_json, const char *commands_json, const char *rationale);
MikomaiResult mikomai_operation_plan_approve(const char *id, const char *plan_hash);
MikomaiResult mikomai_operation_plan_get(const char *id);
MikomaiResult mikomai_operation_plan_begin(const char *id, const char *plan_hash);
MikomaiResult mikomai_operation_plan_finish(const char *id, int32_t succeeded);
void mikomai_result_free(MikomaiResult result);

#endif
