#include <Config.h>
#include <Lib.h>
#include <Log.h>
#include <cJSON.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <io/File.h>
#include <io/Dir.h>
#include <io/Threads.h>

#ifdef SYS_USE_SDL2
#include <ui/Main.h>
#endif

Config g_config = {
    .networking = {
        .port = 8606,
        .server_count = 1
    },
    .pairing = {
        .maximum_players_per_lobby = 7,
        .ip_validation = true,
        .ping_limit = 250,
        .player_maximum_errors = 30000,
        .versioning = {
            .target_version = 1101,
            .disable_version_validating = false
        }
    },
    .logging = {
        .log_debug = false,
        .log_to_file = false
    },
    .lobby_misc = {
        .moderation = {
            .ban_ip = true,
            .ban_udid = true,
            .ban_nickname = false,
            .op_default_level = 3,
            .banhammer_friendly_fire = false
        },
        .message_of_the_day = "\\mods are disallowed on this server",
        .lobby_timeout_timer = 25,
        .lobby_start_timer = 5,
        .votekick_cooldown = 30,
        .apply_textchat_fixes = false,
        .authoritarian_mode = false,
        .lobby_ready_required_percentage = 100,
        .kick_unready_before_starting = true,
        .server_location = "Saint Petersburg",
        .hosts_name = "That Arctic Furry",
        .anonymous_mode = false
    },
    .map_selection = {
        .enabled = true,
        .exclude_last_map = true,
        .timer = 30,
        .map_list = { true, true, true, true, true, true, true, true, true, true, true, true, true, true, true, true, true, true, true, true, false }
    },
    .character_selection = {
        .enable = true,
        .charselect_mod_unlocked = false,
        .charselect_timer = 30,
        .allow_foreign_characters = false
    },
    .gameplay = {
        .respawn_time = 30,
        .sudden_death_timer = 120,
        .ring_appearance_timer = 60,
        .escape_time = 50,
        .demonization_percentage = 50,
        .exe_camp_penalty = true,
        .hide_player_characters = false,
        .enable_achievements = true,
        .enable_sounds = true,
        .gametimers_ceiling = 570,
        .match_respawn_and_game_timers = false,
        .entities_misc = {
            .global = {
                .rings = {
                    .enabled = true,
                    .red_ring_chance = 11
                },
                .spikes = {
                    .timer = 2
                },
            },
            .map_specific = {
                .ravine_mist = {
                    .shards = {
                        .amount = 7,
                        .required_for_exit = 6
                    },
                    .slugs = {
                        .enabled = true,
                        .ring_chance = 40,
                        .red_ring_chance = 10
                    }
                },
                .you_cant_run = {
                    .gas = {
                        .delay = 6
                    }
                },
                .limb_city = {
                    .chain = {
                        .delay = 8,
                        .warning = 2,
                        .shocking_time = 2
                    },
                    .eye = {
                        .recharge_strength = 10,
                        .recharge_timer = 2,
                        .use_cost = 20
                    }
                },
                .not_perfect = {
                    .switch_timer = 5,
                    .switch_timer_chase = 3,
                    .switch_warning_timer = 15,
                    .switch_warning_timer_chase = 2
                },
                .kind_and_fair = {
                    .speedbox = {
                        .timer = 25,
                        .timer_offset = 5
                    }
                },
                .act9 = {
                    .walls = {
                        .enabled = true
                    }
                },
                .nasty_paradise = {
                    .snowballs = {
                        .enabled = true
                    },
                    .ice = {
                        .regeneration_timer = 15
                    }
                },
                .priceless_freedom = {
                    .black_rings = {
                        .enabled = true
                    }
                },
                .hills = {
                    .thunder = {
                        .timer = 15,
                        .timer_offset = 5
                    }
                },
                .torture_cave = {
                    .acid = {
                        .delay = 4
                    }
                },
                .dark_tower = {
                    .stalactites = {
                        .timer = 25,
                        .timer_offset = 5,
                        .acceleration = 0.164
                    },
                    .balls = {
                        .shift_per_tick = 0.015
                    }
                },
                .haunting_dream = {
                    .doors = {
                        .toggle_delay = 10
                    }
                },
                .fart_zone = {
                    .dummy = {
                        .velocity = 0.046875
                    },
                    .black_rings = {
                        .enabled = true
                    }
                }
            },
            .character_specific = {
                .tails = {
                    .projectile_speed = 14,
                    .projectile_timeout_timer = 5
                }
            }
        },
        .anticheat = {
            .palette_anticheat = true,
            .zone_anticheat = true,
            .distance_anticheat = true,
            .ability_anticheat = true,
            .data_based_anticheat = true
        },
        .competitive_mode = {
            .enable = false,
            .eliminate_count = 0,
            .campers_are_first = false
        },
        .banana = {
            .disable_timer = false,
            .singleplayer = false
        },
        .gmcycle = {
            .overhell = false
        }
    },
    .results_misc = {
        .enabled = true,
        .timer = 15,
        .pride = true
    },
    .gui = {
        .interface_scale = 2
    },
    .other = {
        .ignore_inadequate_configuration = false,
    }
};

bool config_verify(void)
{
    if (
    (
        g_config.gameplay.ring_appearance_timer < g_config.gameplay.escape_time
        && !g_config.gameplay.banana.disable_timer
        && !g_config.gameplay.gmcycle.overhell
    )
    || g_config.gameplay.entities_misc.map_specific.ravine_mist.shards.amount > 12
    || g_config.gameplay.entities_misc.map_specific.ravine_mist.shards.amount <
    g_config.gameplay.entities_misc.map_specific.ravine_mist.shards.required_for_exit
    || g_config.gameplay.entities_misc.map_specific.ravine_mist.slugs.ring_chance +
    g_config.gameplay.entities_misc.map_specific.ravine_mist.slugs.red_ring_chance > 100
    || g_config.lobby_misc.lobby_ready_required_percentage > 100
    || g_config.gameplay.competitive_mode.eliminate_count > g_config.pairing.maximum_players_per_lobby
    || g_config.gui.interface_scale > 6
    || g_config.gui.interface_scale < 1
        )
        return false;

    return true;
}

bool config_init(void)
{
	MutexCreate(g_config.map_list_lock);
	
	// Try to open config
	FILE* file = fopen(CONFIG_FILE, "r");
	if (!file)
	{
		RAssert(config_save());

		// Reopen
		file = fopen(CONFIG_FILE, "r");
		if (!file)
		{
			Warn("Failed to save default config file properly!");
            return false;
		}
	}

    char buffer[5000] = { 0 };
    size_t len = fread(buffer, 1, 5000, file);
	fclose(file);

	cJSON* json = cJSON_ParseWithLength(buffer, len);
	if (!json)
	{
		Err("Failed to parse %s: %s", CONFIG_FILE, cJSON_GetErrorPtr());
		return false;
	}
	else
		Debug("%s loaded.", CONFIG_FILE);

    cJSON* networking = cJSON_GetObjectItem(json, "networking");
    {
        g_config.networking.port = cJSON_GetObjectItem(networking, "port")->valueint;
        g_config.networking.server_count = cJSON_GetObjectItem(networking, "server_count")->valueint;
    }

    cJSON* pairing = cJSON_GetObjectItem(json, "pairing");
    {
        g_config.pairing.maximum_players_per_lobby = cJSON_GetObjectItem(pairing, "maximum_players_per_lobby")->valueint;
        g_config.pairing.ip_validation = cJSON_IsTrue(cJSON_GetObjectItem(pairing, "ip_validation"));
        g_config.pairing.ping_limit = cJSON_GetObjectItem(pairing, "ping_limit")->valueint;
        g_config.pairing.player_maximum_errors = cJSON_GetObjectItem(pairing, "player_maximum_errors")->valueint;
        cJSON* versioning = cJSON_GetObjectItem(pairing, "versioning");
        {
            g_config.pairing.versioning.target_version = cJSON_GetObjectItem(versioning, "target_version")->valueint;
            g_config.pairing.versioning.disable_version_validating = cJSON_IsTrue(cJSON_GetObjectItem(versioning, "disable_version_validating"));
        }
    }

    cJSON* logging = cJSON_GetObjectItem(json, "logging");
    {
        g_config.logging.log_debug = cJSON_IsTrue(cJSON_GetObjectItem(logging, "log_debug"));
        g_config.logging.log_to_file = cJSON_IsTrue(cJSON_GetObjectItem(logging, "log_to_file"));
    }

    cJSON* lobby_misc = cJSON_GetObjectItem(json, "lobby_misc");
    {
        cJSON* moderation = cJSON_GetObjectItem(lobby_misc, "moderation");
        {
            g_config.lobby_misc.moderation.ban_ip = cJSON_IsTrue(cJSON_GetObjectItem(moderation, "ban_ip"));
            g_config.lobby_misc.moderation.ban_udid = cJSON_IsTrue(cJSON_GetObjectItem(moderation, "ban_udid"));
            g_config.lobby_misc.moderation.ban_nickname = cJSON_IsTrue(cJSON_GetObjectItem(moderation, "ban_nickname"));
            g_config.lobby_misc.moderation.op_default_level = cJSON_GetObjectItem(moderation, "op_default_level")->valueint;
            g_config.lobby_misc.moderation.banhammer_friendly_fire = cJSON_IsTrue(cJSON_GetObjectItem(moderation, "banhammer_friendly_fire"));
        }
        snprintf(g_config.lobby_misc.message_of_the_day, 80, "%s", cJSON_GetStringValue(cJSON_GetObjectItem(lobby_misc, "message_of_the_day")));
        g_config.lobby_misc.lobby_timeout_timer = cJSON_GetObjectItem(lobby_misc, "lobby_timeout_timer")->valueint;
        g_config.lobby_misc.lobby_start_timer = cJSON_GetObjectItem(lobby_misc, "lobby_start_timer")->valueint;
        g_config.lobby_misc.votekick_cooldown = cJSON_GetObjectItem(lobby_misc, "votekick_cooldown")->valueint;
        snprintf(g_config.lobby_misc.server_location, 64, "%s", cJSON_GetStringValue(cJSON_GetObjectItem(lobby_misc, "server_location")));
        g_config.lobby_misc.apply_textchat_fixes = cJSON_IsTrue(cJSON_GetObjectItem(lobby_misc, "apply_textchat_fixes"));
        g_config.lobby_misc.authoritarian_mode = cJSON_IsTrue(cJSON_GetObjectItem(lobby_misc, "authoritarian_mode"));
        g_config.lobby_misc.lobby_ready_required_percentage = cJSON_GetObjectItem(lobby_misc, "lobby_ready_required_percentage")->valueint;
        g_config.lobby_misc.kick_unready_before_starting = cJSON_IsTrue(cJSON_GetObjectItem(lobby_misc, "kick_unready_before_starting"));
        snprintf(g_config.lobby_misc.hosts_name, 30, "%s", cJSON_GetStringValue(cJSON_GetObjectItem(lobby_misc, "hosts_name")));
        g_config.lobby_misc.anonymous_mode = cJSON_IsTrue(cJSON_GetObjectItem(lobby_misc, "anonymous_mode"));
    }

    cJSON* map_selection = cJSON_GetObjectItem(json, "map_selection");
    {
        g_config.map_selection.enabled = cJSON_IsTrue(cJSON_GetObjectItem(map_selection, "enabled"));
        g_config.map_selection.exclude_last_map = cJSON_IsTrue(cJSON_GetObjectItem(map_selection, "exclude_last_map"));
        g_config.map_selection.timer = cJSON_GetObjectItem(map_selection, "timer")->valueint;
        cJSON* map_list = cJSON_GetObjectItem(map_selection, "map_list");
        if (cJSON_IsArray(map_list))
        {
            for (int i = 0; i < 21; ++i)
            {
                g_config.map_selection.map_list[i] = cJSON_IsTrue(cJSON_GetArrayItem(map_list, i));
            }
        }

        cJSON *character_selection = cJSON_GetObjectItem(json, "character_selection");
        g_config.character_selection.enable = cJSON_IsTrue(cJSON_GetObjectItem(character_selection, "enable"));
        g_config.character_selection.charselect_mod_unlocked = cJSON_IsTrue(cJSON_GetObjectItem(character_selection, "charselect_mod_unlocked"));
        g_config.character_selection.allow_foreign_characters = cJSON_IsTrue(cJSON_GetObjectItem(character_selection, "allow_foreign_characters"));
        g_config.character_selection.charselect_timer = cJSON_GetObjectItem(character_selection, "charselect_timer")->valueint;
    }

    cJSON *gameplay = cJSON_GetObjectItem(json, "gameplay");
    {
        g_config.gameplay.respawn_time = cJSON_GetObjectItem(gameplay, "respawn_time")->valueint;
        g_config.gameplay.sudden_death_timer = cJSON_GetObjectItem(gameplay, "sudden_death_timer")->valueint;
        g_config.gameplay.ring_appearance_timer = cJSON_GetObjectItem(gameplay, "ring_appearance_timer")->valueint;
        g_config.gameplay.escape_time = cJSON_GetObjectItem(gameplay, "escape_time")->valueint;
        g_config.gameplay.demonization_percentage = cJSON_GetObjectItem(gameplay, "demonization_percentage")->valueint;
        g_config.gameplay.exe_camp_penalty = cJSON_IsTrue(cJSON_GetObjectItem(gameplay, "exe_camp_penalty"));
        g_config.gameplay.hide_player_characters = cJSON_IsTrue(cJSON_GetObjectItem(gameplay, "hide_player_characters"));
        g_config.gameplay.enable_achievements = cJSON_IsTrue(cJSON_GetObjectItem(gameplay, "enable_achievements"));
        g_config.gameplay.enable_sounds = cJSON_IsTrue(cJSON_GetObjectItem(gameplay, "enable_sounds"));
        g_config.gameplay.gametimers_ceiling = cJSON_GetObjectItem(gameplay, "gametimers_ceiling")->valueint;
        g_config.gameplay.match_respawn_and_game_timers = cJSON_IsTrue(cJSON_GetObjectItem(gameplay, "match_respawn_and_game_timers"));

        cJSON* anticheat = cJSON_GetObjectItem(gameplay, "anticheat");
        {
            g_config.gameplay.anticheat.palette_anticheat = cJSON_IsTrue(cJSON_GetObjectItem(anticheat, "palette_anticheat"));
            g_config.gameplay.anticheat.zone_anticheat = cJSON_IsTrue(cJSON_GetObjectItem(anticheat, "zone_anticheat"));
            g_config.gameplay.anticheat.distance_anticheat = cJSON_IsTrue(cJSON_GetObjectItem(anticheat, "distance_anticheat"));
            g_config.gameplay.anticheat.ability_anticheat = cJSON_IsTrue(cJSON_GetObjectItem(anticheat, "ability_anticheat"));
            g_config.gameplay.anticheat.data_based_anticheat = cJSON_IsTrue(cJSON_GetObjectItem(anticheat, "data_based_anticheat"));
        }

        cJSON* competitive_mode = cJSON_GetObjectItem(gameplay, "competitive_mode");
        {
            g_config.gameplay.competitive_mode.enable = cJSON_IsTrue(cJSON_GetObjectItem(competitive_mode, "enable"));
            g_config.gameplay.competitive_mode.eliminate_count = cJSON_GetObjectItem(competitive_mode, "eliminate_count")->valueint;
            g_config.gameplay.competitive_mode.campers_are_first = cJSON_IsTrue(cJSON_GetObjectItem(competitive_mode, "campers_are_first"));
        }

        cJSON* banana = cJSON_GetObjectItem(gameplay, "banana");
        {
            g_config.gameplay.banana.disable_timer = cJSON_IsTrue(cJSON_GetObjectItem(banana, "disable_timer"));
            g_config.gameplay.banana.singleplayer = cJSON_IsTrue(cJSON_GetObjectItem(banana, "singleplayer"));
        }

        cJSON* gmcycle = cJSON_GetObjectItem(gameplay, "gmcycle");
        {
            g_config.gameplay.gmcycle.overhell = cJSON_IsTrue(cJSON_GetObjectItem(gmcycle, "overhell"));
        }

        cJSON* entities_misc = cJSON_GetObjectItem(gameplay, "entities_misc");
        {
            cJSON* global = cJSON_GetObjectItem(entities_misc, "global");
            {
                cJSON* rings = cJSON_GetObjectItem(global, "rings");
                g_config.gameplay.entities_misc.global.rings.enabled = cJSON_IsTrue(cJSON_GetObjectItem(rings, "enabled"));
                g_config.gameplay.entities_misc.global.rings.red_ring_chance = cJSON_GetObjectItem(rings, "red_ring_chance")->valueint;

                cJSON* spikes = cJSON_GetObjectItem(global, "spikes");
                g_config.gameplay.entities_misc.global.spikes.timer = cJSON_GetObjectItem(spikes, "timer")->valueint;
            }

            cJSON* map_specific = cJSON_GetObjectItem(entities_misc, "map_specific");
            {
                cJSON* ravine_mist = cJSON_GetObjectItem(map_specific, "ravine_mist");
                {
                    cJSON* shards = cJSON_GetObjectItem(ravine_mist, "shards");
                    {
                        g_config.gameplay.entities_misc.map_specific.ravine_mist.shards.amount = cJSON_GetObjectItem(shards, "amount")->valueint;
                        g_config.gameplay.entities_misc.map_specific.ravine_mist.shards.required_for_exit = cJSON_GetObjectItem(shards, "required_for_exit")->valueint;
                    }

                    cJSON* slugs = cJSON_GetObjectItem(ravine_mist, "slugs");
                    {
                        g_config.gameplay.entities_misc.map_specific.ravine_mist.slugs.enabled = cJSON_IsTrue(cJSON_GetObjectItem(slugs, "enabled"));
                        g_config.gameplay.entities_misc.map_specific.ravine_mist.slugs.ring_chance = cJSON_GetObjectItem(slugs, "ring_chance")->valueint;
                        g_config.gameplay.entities_misc.map_specific.ravine_mist.slugs.red_ring_chance = cJSON_GetObjectItem(slugs, "red_ring_chance")->valueint;
                    }
                }

                cJSON* you_cant_run = cJSON_GetObjectItem(map_specific, "you_cant_run");
                {
                    cJSON* gas = cJSON_GetObjectItem(you_cant_run, "gas");
                    {
                        g_config.gameplay.entities_misc.map_specific.you_cant_run.gas.delay = cJSON_GetObjectItem(gas, "delay")->valueint;
                    }
                }

                cJSON* limp_city = cJSON_GetObjectItem(map_specific, "limp_city");
                {
                    cJSON* chain = cJSON_GetObjectItem(limp_city, "chain");
                    g_config.gameplay.entities_misc.map_specific.limb_city.chain.delay = cJSON_GetObjectItem(chain, "delay")->valueint;
                    g_config.gameplay.entities_misc.map_specific.limb_city.chain.warning = cJSON_GetObjectItem(chain, "warning")->valueint;
                    g_config.gameplay.entities_misc.map_specific.limb_city.chain.shocking_time = cJSON_GetObjectItem(chain, "shocking_time")->valueint;

                    cJSON* eye = cJSON_GetObjectItem(limp_city, "eye");
                    g_config.gameplay.entities_misc.map_specific.limb_city.eye.recharge_strength = cJSON_GetObjectItem(eye, "recharge_strength")->valueint;
                    g_config.gameplay.entities_misc.map_specific.limb_city.eye.recharge_timer = cJSON_GetObjectItem(eye, "recharge_timer")->valueint;
                    g_config.gameplay.entities_misc.map_specific.limb_city.eye.use_cost = cJSON_GetObjectItem(eye, "use_cost")->valueint;
                }

                cJSON* not_perfect = cJSON_GetObjectItem(map_specific, "not_perfect");
                g_config.gameplay.entities_misc.map_specific.not_perfect.switch_timer = cJSON_GetObjectItem(not_perfect, "switch_timer")->valueint;
                g_config.gameplay.entities_misc.map_specific.not_perfect.switch_timer_chase = cJSON_GetObjectItem(not_perfect, "switch_timer_chase")->valueint;
                g_config.gameplay.entities_misc.map_specific.not_perfect.switch_warning_timer = cJSON_GetObjectItem(not_perfect, "switch_warning_timer")->valueint;
                g_config.gameplay.entities_misc.map_specific.not_perfect.switch_warning_timer_chase = cJSON_GetObjectItem(not_perfect, "switch_warning_timer_chase")->valueint;

                cJSON* kind_and_fair = cJSON_GetObjectItem(map_specific, "kind_and_fair");

                cJSON* speedbox = cJSON_GetObjectItem(kind_and_fair, "speedbox");
                g_config.gameplay.entities_misc.map_specific.kind_and_fair.speedbox.timer = cJSON_GetObjectItem(speedbox, "timer")->valueint;
                g_config.gameplay.entities_misc.map_specific.kind_and_fair.speedbox.timer_offset = cJSON_GetObjectItem(speedbox, "timer_offset")->valueint;

                cJSON* act9 = cJSON_GetObjectItem(map_specific, "act9");

                cJSON* walls = cJSON_GetObjectItem(act9, "walls");
                g_config.gameplay.entities_misc.map_specific.act9.walls.enabled = cJSON_IsTrue(cJSON_GetObjectItem(walls, "enabled"));

                cJSON* nasty_paradise = cJSON_GetObjectItem(map_specific, "nasty_paradise");
                {
                    cJSON* snowballs = cJSON_GetObjectItem(nasty_paradise, "snowballs");
                    g_config.gameplay.entities_misc.map_specific.nasty_paradise.snowballs.enabled = cJSON_IsTrue(cJSON_GetObjectItem(snowballs, "enabled"));

                    cJSON* ice = cJSON_GetObjectItem(nasty_paradise, "ice");
                    g_config.gameplay.entities_misc.map_specific.nasty_paradise.ice.regeneration_timer = cJSON_GetObjectItem(ice, "regeneration_timer")->valueint;
                }

                cJSON* priceless_freedom = cJSON_GetObjectItem(map_specific, "priceless_freedom");
                {
                    cJSON* black_rings = cJSON_GetObjectItem(priceless_freedom, "black_rings");
                    {
                        g_config.gameplay.entities_misc.map_specific.priceless_freedom.black_rings.enabled = cJSON_IsTrue(cJSON_GetObjectItem(black_rings, "enabled"));
                    }
                }

                cJSON* hills = cJSON_GetObjectItem(map_specific, "hills");

                cJSON* thunder = cJSON_GetObjectItem(hills, "thunder");
                g_config.gameplay.entities_misc.map_specific.hills.thunder.timer = cJSON_GetObjectItem(thunder, "timer")->valueint;
                g_config.gameplay.entities_misc.map_specific.hills.thunder.timer_offset = cJSON_GetObjectItem(thunder, "timer_offset")->valueint;

                cJSON* torture_cave = cJSON_GetObjectItem(map_specific, "torture_cave");

                cJSON* acid = cJSON_GetObjectItem(torture_cave, "acid");
                g_config.gameplay.entities_misc.map_specific.torture_cave.acid.delay = cJSON_GetObjectItem(acid, "delay")->valueint;

                cJSON* dark_tower = cJSON_GetObjectItem(map_specific, "dark_tower");
                {
                    cJSON* stalactites = cJSON_GetObjectItem(dark_tower, "stalactites");
                    {
                        g_config.gameplay.entities_misc.map_specific.dark_tower.stalactites.timer = cJSON_GetObjectItem(stalactites, "timer")->valueint;
                        g_config.gameplay.entities_misc.map_specific.dark_tower.stalactites.timer_offset = cJSON_GetObjectItem(stalactites, "timer_offset")->valueint;
                        g_config.gameplay.entities_misc.map_specific.dark_tower.stalactites.acceleration = cJSON_GetObjectItem(stalactites, "acceleration")->valuedouble;
                    }

                    cJSON* balls = cJSON_GetObjectItem(dark_tower, "balls");
                    {
                        g_config.gameplay.entities_misc.map_specific.dark_tower.balls.shift_per_tick = cJSON_GetObjectItem(balls, "shift_per_tick")->valuedouble;
                    }
                }

                cJSON* haunting_dream = cJSON_GetObjectItem(map_specific, "haunting_dream");
                {
                    cJSON* doors = cJSON_GetObjectItem(haunting_dream, "doors");
                    {
                        g_config.gameplay.entities_misc.map_specific.haunting_dream.doors.toggle_delay = cJSON_GetObjectItem(doors, "toggle_delay")->valueint;
                    }
                }

                cJSON* fart_zone = cJSON_GetObjectItem(map_specific, "fart_zone");
                {
                    cJSON* dummy = cJSON_GetObjectItem(fart_zone, "dummy");
                    {
                        g_config.gameplay.entities_misc.map_specific.fart_zone.dummy.velocity = cJSON_GetObjectItem(dummy, "velocity")->valuedouble;
                    }

                    cJSON* fart_zone_black_rings = cJSON_GetObjectItem(fart_zone, "black_rings");
                    {
                        g_config.gameplay.entities_misc.map_specific.fart_zone.black_rings.enabled = cJSON_IsTrue(cJSON_GetObjectItem(fart_zone_black_rings, "enabled"));
                    }
                }
            }

            cJSON* character_specific = cJSON_GetObjectItem(entities_misc, "character_specific");
            {
                cJSON* tails = cJSON_GetObjectItem(character_specific, "tails");
                g_config.gameplay.entities_misc.character_specific.tails.projectile_speed = cJSON_GetObjectItem(tails, "projectile_speed")->valueint;
                g_config.gameplay.entities_misc.character_specific.tails.projectile_timeout_timer = cJSON_GetObjectItem(tails, "projectile_timeout_timer")->valueint;
            }
        }
    }

    cJSON* results_misc = cJSON_GetObjectItem(json, "results_misc");
    {
        g_config.results_misc.enabled = cJSON_IsTrue(cJSON_GetObjectItem(results_misc, "enabled"));
        g_config.results_misc.timer = cJSON_GetObjectItem(results_misc, "timer")->valueint;
        g_config.results_misc.pride = cJSON_IsTrue(cJSON_GetObjectItem(results_misc, "pride"));
    }

    cJSON* gui = cJSON_GetObjectItem(json, "gui");
    {
        g_config.gui.interface_scale = cJSON_GetObjectItem(gui, "interface_scale")->valueint;
    }

    cJSON* other = cJSON_GetObjectItem(json, "other");
    {
        g_config.other.ignore_inadequate_configuration = cJSON_IsTrue(cJSON_GetObjectItem(other, "ignore_inadequate_configuration"));
    }

    cJSON_Delete(json);

    if (!g_config.other.ignore_inadequate_configuration && !config_verify()) {
        Err("Ильич, ты чо, долбаēб что ли?");
        exit(1);
    }

	return true;
}

bool config_save(void)
{
    cJSON* json = cJSON_CreateObject();
    RAssert(json);
    {
        cJSON* networking = cJSON_CreateObject();
        {
            cJSON_AddNumberToObject(networking, "port", g_config.networking.port);
            cJSON_AddNumberToObject(networking, "server_count", g_config.networking.server_count);
        }
        cJSON_AddItemToObject(json, "networking", networking);

        cJSON* pairing = cJSON_CreateObject();
        {
            cJSON_AddNumberToObject(pairing, "maximum_players_per_lobby", g_config.pairing.maximum_players_per_lobby);
            cJSON_AddBoolToObject(pairing, "ip_validation", g_config.pairing.ip_validation);
            cJSON_AddNumberToObject(pairing, "ping_limit", g_config.pairing.ping_limit);
            cJSON_AddNumberToObject(pairing, "player_maximum_errors", g_config.pairing.player_maximum_errors);
            cJSON* versioning = cJSON_CreateObject();
            {
                cJSON_AddNumberToObject(versioning, "target_version", g_config.pairing.versioning.target_version);
                cJSON_AddBoolToObject(versioning, "disable_version_validating", g_config.pairing.versioning.disable_version_validating);
            }
            cJSON_AddItemToObject(pairing, "versioning", versioning);
        }
        cJSON_AddItemToObject(json, "pairing", pairing);

        cJSON* logging = cJSON_CreateObject();
        {
            cJSON_AddBoolToObject(logging, "log_debug", g_config.logging.log_debug);
            cJSON_AddBoolToObject(logging, "log_to_file", g_config.logging.log_to_file);
        }
        cJSON_AddItemToObject(json, "logging", logging);

        cJSON* lobby_misc = cJSON_CreateObject();
        {
            cJSON* moderation = cJSON_CreateObject();
            {
                cJSON_AddBoolToObject(moderation, "ban_ip", g_config.lobby_misc.moderation.ban_ip);
                cJSON_AddBoolToObject(moderation, "ban_udid", g_config.lobby_misc.moderation.ban_udid);
                cJSON_AddBoolToObject(moderation, "ban_nickname", g_config.lobby_misc.moderation.ban_nickname);
                cJSON_AddNumberToObject(moderation, "op_default_level", g_config.lobby_misc.moderation.op_default_level);
                cJSON_AddBoolToObject(moderation, "banhammer_friendly_fire", g_config.lobby_misc.moderation.banhammer_friendly_fire);
            }
            cJSON_AddItemToObject(lobby_misc, "moderation", moderation);

            cJSON_AddStringToObject(lobby_misc, "message_of_the_day", g_config.lobby_misc.message_of_the_day);
            cJSON_AddNumberToObject(lobby_misc, "lobby_timeout_timer", g_config.lobby_misc.lobby_timeout_timer);
            cJSON_AddNumberToObject(lobby_misc, "lobby_start_timer", g_config.lobby_misc.lobby_start_timer);
            cJSON_AddNumberToObject(lobby_misc, "votekick_cooldown", g_config.lobby_misc.votekick_cooldown);
            cJSON_AddStringToObject(lobby_misc, "server_location", g_config.lobby_misc.server_location);
            cJSON_AddBoolToObject(lobby_misc, "apply_textchat_fixes", g_config.lobby_misc.apply_textchat_fixes);
            cJSON_AddBoolToObject(lobby_misc, "authoritarian_mode", g_config.lobby_misc.authoritarian_mode);
            cJSON_AddNumberToObject(lobby_misc, "lobby_ready_required_percentage", g_config.lobby_misc.lobby_ready_required_percentage);
            cJSON_AddBoolToObject(lobby_misc, "kick_unready_before_starting", g_config.lobby_misc.kick_unready_before_starting);
            cJSON_AddStringToObject(lobby_misc, "hosts_name", g_config.lobby_misc.hosts_name);
            cJSON_AddBoolToObject(lobby_misc, "anonymous_mode", g_config.lobby_misc.anonymous_mode);
        }
        cJSON_AddItemToObject(json, "lobby_misc", lobby_misc);

        cJSON* map_selection = cJSON_CreateObject();
        {
            cJSON_AddBoolToObject(map_selection, "enabled", g_config.map_selection.enabled);
            cJSON_AddBoolToObject(map_selection, "exclude_last_map", g_config.map_selection.exclude_last_map);
            cJSON_AddNumberToObject(map_selection, "timer", g_config.map_selection.timer);
            cJSON* map_list = cJSON_CreateArray();
            for (int i = 0; i < 21; ++i) {
                cJSON_AddItemToArray(map_list, cJSON_CreateBool(g_config.map_selection.map_list[i]));
            }
            cJSON_AddItemToObject(map_selection, "map_list", map_list);
        }
        cJSON_AddItemToObject(json, "map_selection", map_selection);

        cJSON* character_selection = cJSON_CreateObject();
        {
            cJSON_AddBoolToObject(character_selection, "enable", g_config.character_selection.enable);
            cJSON_AddBoolToObject(character_selection, "charselect_mod_unlocked", g_config.character_selection.charselect_mod_unlocked);
            cJSON_AddBoolToObject(character_selection, "allow_foreign_characters", g_config.character_selection.allow_foreign_characters);
            cJSON_AddNumberToObject(character_selection, "charselect_timer", g_config.character_selection.charselect_timer);
        }
        cJSON_AddItemToObject(json, "character_selection", character_selection);

        cJSON* gameplay = cJSON_CreateObject();
        {
            cJSON_AddNumberToObject(gameplay, "respawn_time", g_config.gameplay.respawn_time);
            cJSON_AddNumberToObject(gameplay, "sudden_death_timer", g_config.gameplay.sudden_death_timer);
            cJSON_AddNumberToObject(gameplay, "ring_appearance_timer", g_config.gameplay.ring_appearance_timer);
            cJSON_AddNumberToObject(gameplay, "escape_time", g_config.gameplay.escape_time);
            cJSON_AddNumberToObject(gameplay, "demonization_percentage", g_config.gameplay.demonization_percentage);
            cJSON_AddBoolToObject(gameplay, "exe_camp_penalty", g_config.gameplay.exe_camp_penalty);
            cJSON_AddBoolToObject(gameplay, "hide_player_characters", g_config.gameplay.hide_player_characters);
            cJSON_AddBoolToObject(gameplay, "enable_achievements", g_config.gameplay.enable_achievements);
            cJSON_AddBoolToObject(gameplay, "enable_sounds", g_config.gameplay.enable_sounds);
            cJSON_AddNumberToObject(gameplay, "gametimers_ceiling", g_config.gameplay.gametimers_ceiling);
            cJSON_AddBoolToObject(gameplay, "match_respawn_and_game_timers", g_config.gameplay.match_respawn_and_game_timers);

            cJSON* anticheat = cJSON_CreateObject();
            {
                cJSON_AddBoolToObject(anticheat, "palette_anticheat", g_config.gameplay.anticheat.palette_anticheat);
                cJSON_AddBoolToObject(anticheat, "zone_anticheat", g_config.gameplay.anticheat.zone_anticheat);
                cJSON_AddBoolToObject(anticheat, "distance_anticheat", g_config.gameplay.anticheat.distance_anticheat);
                cJSON_AddBoolToObject(anticheat, "ability_anticheat", g_config.gameplay.anticheat.ability_anticheat);
                cJSON_AddBoolToObject(anticheat, "data_based_anticheat", g_config.gameplay.anticheat.data_based_anticheat);
            }
            cJSON_AddItemToObject(gameplay, "anticheat", anticheat);

            cJSON* competitive_mode = cJSON_CreateObject();
            {
                cJSON_AddBoolToObject(competitive_mode, "enable", g_config.gameplay.competitive_mode.enable);
                cJSON_AddNumberToObject(competitive_mode, "eliminate_count", g_config.gameplay.competitive_mode.eliminate_count);
                cJSON_AddBoolToObject(competitive_mode, "campers_are_first", g_config.gameplay.competitive_mode.campers_are_first);
            }
            cJSON_AddItemToObject(gameplay, "competitive_mode", competitive_mode);

            cJSON* banana = cJSON_CreateObject();
            {
                cJSON_AddBoolToObject(banana, "disable_timer", g_config.gameplay.banana.disable_timer);
                cJSON_AddBoolToObject(banana, "singleplayer", g_config.gameplay.banana.singleplayer);
            }
            cJSON_AddItemToObject(gameplay, "banana", banana);

            cJSON* gmcycle = cJSON_CreateObject();
            {
                cJSON_AddBoolToObject(gmcycle, "overhell", g_config.gameplay.gmcycle.overhell);
            }
            cJSON_AddItemToObject(gameplay, "gmcycle", gmcycle);

            cJSON* entities_misc = cJSON_CreateObject();
            {
                cJSON* global = cJSON_CreateObject();
                {
                    cJSON* rings = cJSON_CreateObject();
                    cJSON_AddBoolToObject(rings, "enabled", g_config.gameplay.entities_misc.global.rings.enabled);
                    cJSON_AddNumberToObject(rings, "red_ring_chance", g_config.gameplay.entities_misc.global.rings.red_ring_chance);
                    cJSON_AddItemToObject(global, "rings", rings);

                    cJSON* spikes = cJSON_CreateObject();
                    cJSON_AddNumberToObject(spikes, "timer", g_config.gameplay.entities_misc.global.spikes.timer);
                    cJSON_AddItemToObject(global, "spikes", spikes);
                }
                cJSON_AddItemToObject(entities_misc, "global", global);

                cJSON* map_specific = cJSON_CreateObject();
                {
                    cJSON* ravine_mist = cJSON_CreateObject();
                    {
                        cJSON* shards = cJSON_CreateObject();
                        {
                            cJSON_AddNumberToObject(shards, "amount", g_config.gameplay.entities_misc.map_specific.ravine_mist.shards.amount);
                            cJSON_AddNumberToObject(shards, "required_for_exit", g_config.gameplay.entities_misc.map_specific.ravine_mist.shards.required_for_exit);
                        }
                        cJSON_AddItemToObject(ravine_mist, "shards", shards);

                        cJSON* slugs = cJSON_CreateObject();
                        {
                            cJSON_AddBoolToObject(slugs, "enabled", g_config.gameplay.entities_misc.map_specific.ravine_mist.slugs.enabled);
                            cJSON_AddNumberToObject(slugs, "ring_chance", g_config.gameplay.entities_misc.map_specific.ravine_mist.slugs.ring_chance);
                            cJSON_AddNumberToObject(slugs, "red_ring_chance", g_config.gameplay.entities_misc.map_specific.ravine_mist.slugs.red_ring_chance);
                        }
                        cJSON_AddItemToObject(ravine_mist, "slugs", slugs);
                    }
                    cJSON_AddItemToObject(map_specific, "ravine_mist", ravine_mist);

                    cJSON* you_cant_run = cJSON_CreateObject();
                    {
                        cJSON* gas = cJSON_CreateObject();
                        cJSON_AddNumberToObject(gas, "delay", g_config.gameplay.entities_misc.map_specific.you_cant_run.gas.delay);
                        cJSON_AddItemToObject(you_cant_run, "gas", gas);
                    }
                    cJSON_AddItemToObject(map_specific, "you_cant_run", you_cant_run);

                    cJSON* limp_city = cJSON_CreateObject();

                    cJSON* chain = cJSON_CreateObject();
                    cJSON_AddNumberToObject(chain, "delay", g_config.gameplay.entities_misc.map_specific.limb_city.chain.delay);
                    cJSON_AddNumberToObject(chain, "warning", g_config.gameplay.entities_misc.map_specific.limb_city.chain.warning);
                    cJSON_AddNumberToObject(chain, "shocking_time", g_config.gameplay.entities_misc.map_specific.limb_city.chain.shocking_time);
                    cJSON_AddItemToObject(limp_city, "chain", chain);

                    cJSON* eye = cJSON_CreateObject();
                    cJSON_AddNumberToObject(eye, "recharge_strength", g_config.gameplay.entities_misc.map_specific.limb_city.eye.recharge_strength);
                    cJSON_AddNumberToObject(eye, "recharge_timer", g_config.gameplay.entities_misc.map_specific.limb_city.eye.recharge_timer);
                    cJSON_AddNumberToObject(eye, "use_cost", g_config.gameplay.entities_misc.map_specific.limb_city.eye.use_cost);
                    cJSON_AddItemToObject(limp_city, "eye", eye);

                    cJSON_AddItemToObject(map_specific, "limp_city", limp_city);

                    cJSON* not_perfect = cJSON_CreateObject();
                    cJSON_AddNumberToObject(not_perfect, "switch_timer", g_config.gameplay.entities_misc.map_specific.not_perfect.switch_timer);
                    cJSON_AddNumberToObject(not_perfect, "switch_timer_chase", g_config.gameplay.entities_misc.map_specific.not_perfect.switch_timer_chase);
                    cJSON_AddNumberToObject(not_perfect, "switch_warning_timer", g_config.gameplay.entities_misc.map_specific.not_perfect.switch_warning_timer);
                    cJSON_AddNumberToObject(not_perfect, "switch_warning_timer_chase", g_config.gameplay.entities_misc.map_specific.not_perfect.switch_warning_timer_chase);
                    cJSON_AddItemToObject(map_specific, "not_perfect", not_perfect);

                    cJSON* kind_and_fair = cJSON_CreateObject();

                    cJSON* speedbox = cJSON_CreateObject();
                    cJSON_AddNumberToObject(speedbox, "timer", g_config.gameplay.entities_misc.map_specific.kind_and_fair.speedbox.timer);
                    cJSON_AddNumberToObject(speedbox, "timer_offset", g_config.gameplay.entities_misc.map_specific.kind_and_fair.speedbox.timer_offset);
                    cJSON_AddItemToObject(kind_and_fair, "speedbox", speedbox);

                    cJSON_AddItemToObject(map_specific, "kind_and_fair", kind_and_fair);

                    cJSON* act9 = cJSON_CreateObject();

                    cJSON* walls = cJSON_CreateObject();
                    cJSON_AddBoolToObject(walls, "enabled", g_config.gameplay.entities_misc.map_specific.act9.walls.enabled);
                    cJSON_AddItemToObject(act9, "walls", walls);

                    cJSON_AddItemToObject(map_specific, "act9", act9);

                    cJSON* nasty_paradise = cJSON_CreateObject();

                    cJSON* snowballs = cJSON_CreateObject();
                    cJSON_AddBoolToObject(snowballs, "enabled", g_config.gameplay.entities_misc.map_specific.nasty_paradise.snowballs.enabled);
                    cJSON_AddItemToObject(nasty_paradise, "snowballs", snowballs);

                    cJSON* ice = cJSON_CreateObject();
                    cJSON_AddNumberToObject(ice, "regeneration_timer", g_config.gameplay.entities_misc.map_specific.nasty_paradise.ice.regeneration_timer);
                    cJSON_AddItemToObject(nasty_paradise, "ice", ice);

                    cJSON_AddItemToObject(map_specific, "nasty_paradise", nasty_paradise);

                    cJSON* priceless_freedom = cJSON_CreateObject();
                    {
                        cJSON* black_rings = cJSON_CreateObject();
                        {
                            cJSON_AddBoolToObject(black_rings, "enabled", g_config.gameplay.entities_misc.map_specific.priceless_freedom.black_rings.enabled);
                        }
                        cJSON_AddItemToObject(priceless_freedom, "black_rings", black_rings);
                    }
                    cJSON_AddItemToObject(map_specific, "priceless_freedom", priceless_freedom);

                    cJSON* hills = cJSON_CreateObject();

                    cJSON* thunder = cJSON_CreateObject();
                    cJSON_AddNumberToObject(thunder, "timer", g_config.gameplay.entities_misc.map_specific.hills.thunder.timer);
                    cJSON_AddNumberToObject(thunder, "timer_offset", g_config.gameplay.entities_misc.map_specific.hills.thunder.timer_offset);
                    cJSON_AddItemToObject(hills, "thunder", thunder);

                    cJSON_AddItemToObject(map_specific, "hills", hills);

                    cJSON* torture_cave = cJSON_CreateObject();

                    cJSON* acid = cJSON_CreateObject();
                    cJSON_AddNumberToObject(acid, "delay", g_config.gameplay.entities_misc.map_specific.torture_cave.acid.delay);
                    cJSON_AddItemToObject(torture_cave, "acid", acid);

                    cJSON_AddItemToObject(map_specific, "torture_cave", torture_cave);

                    cJSON* dark_tower = cJSON_CreateObject();

                    cJSON* stalactites = cJSON_CreateObject();
                    cJSON_AddNumberToObject(stalactites, "timer", g_config.gameplay.entities_misc.map_specific.dark_tower.stalactites.timer);
                    cJSON_AddNumberToObject(stalactites, "timer_offset", g_config.gameplay.entities_misc.map_specific.dark_tower.stalactites.timer_offset);
                    cJSON_AddNumberToObject(stalactites, "acceleration", g_config.gameplay.entities_misc.map_specific.dark_tower.stalactites.acceleration);
                    cJSON_AddItemToObject(dark_tower, "stalactites", stalactites);

                    cJSON* balls = cJSON_CreateObject();
                    cJSON_AddNumberToObject(balls, "shift_per_tick", g_config.gameplay.entities_misc.map_specific.dark_tower.balls.shift_per_tick);
                    cJSON_AddItemToObject(dark_tower, "balls", balls);

                    cJSON_AddItemToObject(map_specific, "dark_tower", dark_tower);

                    cJSON* haunting_dream = cJSON_CreateObject();
                    {
                        cJSON* doors = cJSON_CreateObject();
                        {
                            cJSON_AddNumberToObject(doors, "toggle_delay", g_config.gameplay.entities_misc.map_specific.haunting_dream.doors.toggle_delay);
                        }
                        cJSON_AddItemToObject(haunting_dream, "doors", doors);
                    }
                    cJSON_AddItemToObject(map_specific, "haunting_dream", haunting_dream);

                    cJSON* fart_zone = cJSON_CreateObject();
                    {
                        cJSON* dummy = cJSON_CreateObject();
                        {
                            cJSON_AddNumberToObject(dummy, "velocity", g_config.gameplay.entities_misc.map_specific.fart_zone.dummy.velocity);
                        }
                        cJSON_AddItemToObject(fart_zone, "dummy", dummy);

                        cJSON* fart_zone_black_rings = cJSON_CreateObject();
                        {
                            cJSON_AddBoolToObject(fart_zone_black_rings, "enabled", g_config.gameplay.entities_misc.map_specific.priceless_freedom.black_rings.enabled);
                        }
                        cJSON_AddItemToObject(fart_zone, "black_rings", fart_zone_black_rings);
                    }
                    cJSON_AddItemToObject(map_specific, "fart_zone", fart_zone);
                }
                cJSON_AddItemToObject(entities_misc, "map_specific", map_specific);

                cJSON* character_specific = cJSON_CreateObject();
                {
                    cJSON* tails = cJSON_CreateObject();
                    {
                        cJSON_AddNumberToObject(tails, "projectile_speed", g_config.gameplay.entities_misc.character_specific.tails.projectile_speed);
                        cJSON_AddNumberToObject(tails, "projectile_timeout_timer", g_config.gameplay.entities_misc.character_specific.tails.projectile_timeout_timer);
                    }
                    cJSON_AddItemToObject(character_specific, "tails", tails);
                }
                cJSON_AddItemToObject(entities_misc, "character_specific", character_specific);
            }
            cJSON_AddItemToObject(gameplay, "entities_misc", entities_misc);
        }
        cJSON_AddItemToObject(json, "gameplay", gameplay);

        cJSON* results_misc = cJSON_CreateObject();
        {
            cJSON_AddBoolToObject(results_misc, "enabled", g_config.results_misc.enabled);
            cJSON_AddNumberToObject(results_misc, "timer", g_config.results_misc.timer);
            cJSON_AddBoolToObject(results_misc, "pride", g_config.results_misc.pride);
        }
        cJSON_AddItemToObject(json, "results_misc", results_misc);

        cJSON* gui = cJSON_CreateObject();
        {
            cJSON_AddNumberToObject(gui, "interface_scale", g_config.gui.interface_scale);
        }
        cJSON_AddItemToObject(json, "gui", gui);

        cJSON* other = cJSON_CreateObject();
        {
            cJSON_AddBoolToObject(other, "ignore_inadequate_configuration", g_config.other.ignore_inadequate_configuration);
        }
        cJSON_AddItemToObject(json, "other", other);
    }
	RAssert(collection_save(CONFIG_FILE, json));
    cJSON_Delete(json);
	return true;
}
