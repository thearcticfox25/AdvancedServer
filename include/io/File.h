#ifndef FILE_H
#define FILE_H

#include <cJSON.h>
#include <io/Threads.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdbool.h>

bool collection_save(const char* file, cJSON* value);
bool collection_init(cJSON** output, const char* file, const char* default_value);

#endif
