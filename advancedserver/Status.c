#include <Status.h>
#include <Log.h>
#include <cJSON.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <io/File.h>
#include <io/Dir.h>

Status g_status = {
    .surv_win_rounds = 0,
    .exe_win_rounds = 0,
    .exe_crashed_rounds = 0,
    .draw_rounds = 0,
    .tails_shots = 0,
    .exeller_clones_placed = 0,
    .exeller_clones_activated = 0,
    .timeouts = 0,
    .damage_taken = 0,
    .total_escaped = 0,
    .total_died = 0,
    .total_demonised = 0,
    .eggman_mines_placed = 0,
    .cream_rings_spawned = 0,
    .tails_hits = 0,
    .total_stuns = 0,
    .exetior_bring_spawned = 0
};

bool status_init(void)
{
    FILE* file = fopen(STATUS_FILE, "r");
    if (!file)
    {
        RAssert(status_save());

        file = fopen(STATUS_FILE, "r");
        if (!file)
        {
            Warn("Failed to save default config file properly!");
            return false;
        }
    }

    char buffer[512];
    size_t len = fread(buffer, 1, sizeof(buffer) - 1, file);
    fclose(file);
    buffer[len] = '\0';

    cJSON* json = cJSON_Parse(buffer);
    if (!json) {
        Err("Failed to parse status file: %s", cJSON_GetErrorPtr());
        return false;
    }

    g_status.surv_win_rounds = cJSON_GetObjectItem(json, "surv_win_rounds")->valueint;
    g_status.exe_win_rounds = cJSON_GetObjectItem(json, "exe_win_rounds")->valueint;
    g_status.exe_crashed_rounds = cJSON_GetObjectItem(json, "exe_crashed_rounds")->valueint;
    g_status.draw_rounds = cJSON_GetObjectItem(json, "draw_rounds")->valueint;
    g_status.tails_shots = cJSON_GetObjectItem(json, "tails_shots")->valueint;
    g_status.exeller_clones_placed = cJSON_GetObjectItem(json, "exeller_clones_placed")->valueint;
    g_status.exeller_clones_activated = cJSON_GetObjectItem(json, "exeller_clones_activated")->valueint;
    g_status.timeouts = cJSON_GetObjectItem(json, "timeouts")->valueint;
    g_status.damage_taken = cJSON_GetObjectItem(json, "damage_taken")->valueint;
    g_status.total_escaped = cJSON_GetObjectItem(json, "total_escaped")->valueint;
    g_status.total_died = cJSON_GetObjectItem(json, "total_died")->valueint;
    g_status.total_demonised = cJSON_GetObjectItem(json, "total_demonised")->valueint;
    g_status.eggman_mines_placed = cJSON_GetObjectItem(json, "eggman_mines_placed")->valueint;
    g_status.cream_rings_spawned = cJSON_GetObjectItem(json, "cream_rings_spawned")->valueint;
    g_status.tails_hits = cJSON_GetObjectItem(json, "tails_hits")->valueint;
    g_status.total_stuns = cJSON_GetObjectItem(json, "total_stuns")->valueint;
    g_status.exetior_bring_spawned = cJSON_GetObjectItem(json, "exetior_bring_spawned")->valueint;


    cJSON_Delete(json);
    return true;
}

bool status_save(void)
{
    cJSON* json = cJSON_CreateObject();
    if (!json) return false;

    cJSON_AddNumberToObject(json, "surv_win_rounds", g_status.surv_win_rounds);
    cJSON_AddNumberToObject(json, "exe_win_rounds", g_status.exe_win_rounds);
    cJSON_AddNumberToObject(json, "exe_crashed_rounds", g_status.exe_crashed_rounds);
    cJSON_AddNumberToObject(json, "draw_rounds", g_status.draw_rounds);
    cJSON_AddNumberToObject(json, "tails_shots", g_status.tails_shots);
    cJSON_AddNumberToObject(json, "exeller_clones_placed", g_status.exeller_clones_placed);
    cJSON_AddNumberToObject(json, "exeller_clones_activated", g_status.exeller_clones_activated);
    cJSON_AddNumberToObject(json, "timeouts", g_status.timeouts);
    cJSON_AddNumberToObject(json, "damage_taken", g_status.damage_taken);
    cJSON_AddNumberToObject(json, "total_escaped", g_status.total_escaped);
    cJSON_AddNumberToObject(json, "total_died", g_status.total_died);
    cJSON_AddNumberToObject(json, "total_demonised", g_status.total_demonised);
    cJSON_AddNumberToObject(json, "eggman_mines_placed", g_status.eggman_mines_placed);
    cJSON_AddNumberToObject(json, "cream_rings_spawned", g_status.cream_rings_spawned);
    cJSON_AddNumberToObject(json, "tails_hits", g_status.tails_hits);
    cJSON_AddNumberToObject(json, "total_stuns", g_status.total_stuns);
    cJSON_AddNumberToObject(json, "exetior_bring_spawned", g_status.exetior_bring_spawned);

    char* out = cJSON_Print(json);
    FILE* file = fopen(STATUS_FILE, "w");
    if (!file) {
        Warn("Could not open status file %s for writing.", STATUS_FILE);
        free(out);
        cJSON_Delete(json);
        return false;
    }

    fwrite(out, 1, strlen(out), file);
    fclose(file);
    free(out);
    cJSON_Delete(json);

    return true;
}
