#ifndef STATUS_H
#define STATUS_H

#include <cJSON.h>
#include <io/Threads.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdbool.h>

#define STATUS_FILE "Status.json"

typedef struct {
    uint32_t surv_win_rounds;
    uint32_t exe_win_rounds;
    uint32_t exe_crashed_rounds;
    uint32_t draw_rounds;
    uint32_t tails_shots;
    uint32_t exeller_clones_placed;
    uint32_t exeller_clones_activated;
    uint32_t timeouts;
    uint32_t damage_taken;
    uint32_t total_escaped;
    uint32_t total_died;
    uint32_t total_demonised;
    uint32_t eggman_mines_placed;
    uint32_t cream_rings_spawned;
    uint32_t tails_hits;
    uint32_t total_stuns;
    uint32_t exetior_bring_spawned;
} Status;

extern Status g_status;

bool status_init(void);
bool status_save(void);

#endif
