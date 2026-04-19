#include <io/File.h>
#include <Log.h>
#include <cJSON.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <io/Dir.h>

bool write_default(const char* filename, const char* default_str)
{
    FILE* file = fopen(filename, "r");
    if (!file)
    {
        file = fopen(filename, "w");

        if (!file)
        {
            Warn("Failed to open %s for writing.", filename);
            return false;
        }

        fwrite(default_str, 1, strlen(default_str), file);
        fclose(file);
    }

    return true;
}

bool collection_save(const char* file, cJSON* value)
{
    char* buffer = cJSON_Print(value);
    FILE* f = fopen(file, "w");
    if (!f)
    {
        Warn("Failed to open %s for writing.", file);
        return false;
    }

    fwrite(buffer, 1, strlen(buffer), f);
    fclose(f);
    free(buffer);

    return true;
}

bool collection_init(cJSON** output, const char* file, const char* default_value)
{
    RAssert(write_default(file, default_value));

    FILE* f = fopen(file, "r");
    if (!f)
    {
        Warn("what de fuck");
        return false;
    }

    fseek(f, 0, SEEK_END);
    size_t len = ftell(f);
    char* buffer = malloc(len);
    if (!buffer)
    {
        Warn("Failed to allocate buffer for a list!");
        return false;
    }

    fseek(f, 0, SEEK_SET);
    fread(buffer, 1, len, f);
    fclose(f);

    *output = cJSON_ParseWithLength(buffer, len);
    if (!(*output))
    {
        Err("Failed to parse %s: %s", file, cJSON_GetErrorPtr());
        *output = cJSON_CreateObject();
        return false;
    }
    else
        Debug("%s loaded.", file);

    free(buffer);
    return true;
}
