#ifndef CONFIG_H
#define CONFIG_H

#include <cJSON.h>
#include <io/Threads.h>
#include <stdbool.h>
#include <stdint.h>

#define CONFIG_FILE "Config.json"

struct Config;
typedef struct
{
    uint16_t            port;
    uint16_t            server_count;
} Networking;
typedef struct
{
    uint16_t            target_version;
    bool                disable_version_validating;
} Versioning;
typedef struct
{
    uint8_t             maximum_players_per_lobby;
    bool                ip_validation;
    uint16_t            ping_limit;
    uint16_t	        player_maximum_errors;
    Versioning          versioning;
} Pairing;
typedef struct
{
    bool                log_debug;
    bool                log_to_file;
} Logging;
typedef struct
{
    bool                ban_ip;
    bool                ban_udid;
    bool                ban_nickname;
    uint8_t	            op_default_level;
    bool                banhammer_friendly_fire;
    bool                enforce_whitelist;
} Moderation;
typedef struct
{
    uint8_t             cooldown;
    bool                autoban_leavers;
} Votekick;
typedef struct
{
    Moderation          moderation;
    Votekick            votekick;
    char                message_of_the_day[80];
    uint8_t             lobby_timeout_timer;
    uint8_t             lobby_start_timer;
    char                server_location[64];
    bool                apply_textchat_fixes;
    bool                authoritarian_mode;
    uint8_t             lobby_ready_required_percentage;
    bool                kick_unready_before_starting;
    char                hosts_name[30];
    bool                anonymous_mode;
} LobbyMisc;
typedef struct
{
    bool                enabled;
    uint8_t             timer;
    bool                map_list[21];
    bool                exclude_last_map;
} MapSelection;
typedef struct
{
    bool                enable;
    bool                charselect_mod_unlocked;
    bool                allow_foreign_characters;
    uint8_t             charselect_timer;
} CharacterSelection;
typedef struct
{
    bool                disable_timer;
    bool                singleplayer;
} Banana;
typedef struct
{
    bool                overhell;
} GMCycle;
typedef struct
{
    bool                palette_anticheat;
    bool                zone_anticheat;
    bool                distance_anticheat;
    bool                ability_anticheat;
    bool                data_based_anticheat;
} Anticheat;
typedef struct
{
    bool                enabled;
    uint8_t             red_ring_chance;
} Rings;
typedef struct
{
    uint8_t             timer;
} Spikes;
typedef struct
{
    Rings               rings;
    Spikes              spikes;
} Global;

typedef struct
{
    uint8_t             amount;
    uint8_t             required_for_exit;
} Shards;
typedef struct
{
    bool                enabled;
    uint8_t             ring_chance;
    uint8_t             red_ring_chance;
} Slugs;
typedef struct
{
    Shards              shards;
    Slugs               slugs;
} RavineMist;

typedef struct
{
    uint16_t            delay;
} Gas;
typedef struct
{
    Gas                 gas;
} YouCantRun;

typedef struct
{
    uint8_t             recharge_strength;
    uint8_t             recharge_timer;
    uint8_t             use_cost;
} Eye;
typedef struct
{
    uint16_t	        delay;
    uint8_t             warning;
    uint8_t             shocking_time;
} Chain;
typedef struct
{
    Eye                 eye;
    Chain               chain;
} LimbCity;

typedef struct
{
    uint8_t             switch_timer;
    uint8_t             switch_timer_chase;
    uint8_t             switch_warning_timer;
    uint8_t             switch_warning_timer_chase;
} NotPerfect;

typedef struct
{
    uint8_t             timer;
    uint8_t             timer_offset;
} Speedbox;
typedef struct
{
    Speedbox            speedbox;
} KindAndFair;

typedef struct
{
    bool                enabled;
} Walls;
typedef struct
{
    Walls               walls;
} Act9;

typedef struct
{
    bool                enabled;
} Snowballs;
typedef struct
{
    uint8_t             regeneration_timer;
} IceConfig;
typedef struct
{
    Snowballs           snowballs;
    IceConfig           ice;
} NastyParadise;

typedef struct
{
    bool                enabled;
} BlackRings;
typedef struct
{
    BlackRings          black_rings;
} PricelessFreedom;

typedef struct
{
    uint8_t             timer_offset;
    uint8_t             timer;
} ThunderConfig;
typedef struct
{
    ThunderConfig       thunder;
} Hills;

typedef struct
{
    uint8_t             delay;
} AcidConfig;
typedef struct
{
    AcidConfig          acid;
} TortureCave;

typedef struct
{
    uint8_t             timer;
    uint8_t             timer_offset;
    double              acceleration;
} Stalactites;
typedef struct
{
    double              shift_per_tick;
} Balls;
typedef struct
{
    Stalactites         stalactites;
    Balls               balls;
} DarkTower;

typedef struct
{
    uint8_t             toggle_delay;
} Doors;
typedef struct
{
    Doors               doors;
} HauntingDream;

typedef struct
{
    double              velocity;
} DummyConfig;
typedef struct
{
    DummyConfig         dummy;
    BlackRings          black_rings;
} FartZone;

typedef struct
{
    RavineMist          ravine_mist;
    YouCantRun          you_cant_run;
    LimbCity            limb_city;
    NotPerfect          not_perfect;
    KindAndFair         kind_and_fair;
    Act9                act9;
    NastyParadise       nasty_paradise;
    PricelessFreedom    priceless_freedom;
    Hills               hills;
    TortureCave         torture_cave;
    DarkTower           dark_tower;
    HauntingDream       haunting_dream;
    FartZone            fart_zone;
} MapSpecific;

typedef struct
{
    int8_t              projectile_speed;
    uint8_t             projectile_timeout_timer;
} Tails;
typedef struct
{
    Tails               tails;
} CharacterSpecific;

typedef struct
{
    Global              global;
    MapSpecific         map_specific;
    CharacterSpecific   character_specific;
} EntitiesMisc;
typedef struct
{
    uint8_t             respawn_time;
    uint8_t             sudden_death_timer;
    uint8_t             ring_appearance_timer;
    uint8_t             escape_time;
    uint8_t             demonization_percentage;
    bool                exe_camp_penalty;
    bool                hide_player_characters;
    bool                enable_achievements;
    bool                enable_sounds;
    uint16_t            gametimers_ceiling;
    bool                match_respawn_and_game_timers;
    uint8_t             ending_timer;
    EntitiesMisc        entities_misc;
    Anticheat           anticheat;
    Banana              banana;
    GMCycle             gmcycle;
} Gameplay;
typedef struct
{
    bool                enabled;
    uint8_t             timer;
    bool                pride;
} ResultsMisc;
typedef struct
{
    uint8_t             interface_scale;
} GUI;
typedef struct
{
    bool                ignore_inadequate_configuration;
} Other;
typedef struct
{
    Mutex               map_list_lock;
    Networking          networking;
    Pairing             pairing;
    Logging             logging;
} ServerConfig;
typedef struct
{
    LobbyMisc           lobby_misc;
    MapSelection        map_selection;
    CharacterSelection  character_selection;
    Gameplay            gameplay;
    ResultsMisc         results_misc;
} StatesConfig;
typedef struct
{
    GUI                 gui;
    Other               other;
} MiscellaneousConfig;
typedef struct
{
    Mutex               map_list_lock;
    ServerConfig        server_config;
    StatesConfig        states;
    MiscellaneousConfig miscellaneous;
} Config;

extern Config g_config;

bool config_init(void);
bool config_save(void);

#endif
