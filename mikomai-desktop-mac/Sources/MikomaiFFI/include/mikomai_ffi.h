#ifndef MIKOMAI_FFI_H
#define MIKOMAI_FFI_H

#include <stdint.h>

typedef struct {
    int32_t status;
    char *message;
} MikomaiResult;

MikomaiResult mikomai_chat(const char *message);
void mikomai_result_free(MikomaiResult result);

#endif
