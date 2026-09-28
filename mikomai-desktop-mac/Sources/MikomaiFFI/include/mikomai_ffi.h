#ifndef MIKOMAI_FFI_H
#define MIKOMAI_FFI_H

#include <stdint.h>

typedef struct {
    int32_t status;
    char *message;
} MikomaiResult;

MikomaiResult mikomai_chat(const char *message);
MikomaiResult mikomai_chat_with_paths(const char *message, const char *documents_dir, const char *knowledge_dir);
MikomaiResult mikomai_model_load(const char *path);
MikomaiResult mikomai_model_status(void);
MikomaiResult mikomai_model_cancel(void);
MikomaiResult mikomai_device_registry_read(const char *path);
MikomaiResult mikomai_assistant_chat(const char *message, const char *history, const char *documents_dir, const char *knowledge_dir);
MikomaiResult mikomai_assistant_chat_with_attachments(const char *message, const char *history, const char *documents_dir, const char *knowledge_dir, const char *attachments);
void mikomai_result_free(MikomaiResult result);

#endif
