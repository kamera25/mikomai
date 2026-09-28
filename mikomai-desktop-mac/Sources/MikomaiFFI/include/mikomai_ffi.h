#ifndef MIKOMAI_FFI_H
#define MIKOMAI_FFI_H

#include <stdint.h>

typedef struct {
    int32_t status;
    char *message;
} MikomaiResult;

MikomaiResult mikomai_chat(const char *message);
MikomaiResult mikomai_chat_with_paths(const char *message, const char *documents_dir, const char *knowledge_dir);
void mikomai_result_free(MikomaiResult result);

#endif
