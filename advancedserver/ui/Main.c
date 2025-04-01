#include "Config.h"
#include <string.h>
#include <ui/Main.h>
#include <ui/Components.h>
#include <ui/Presets.h>
#include <io/Threads.h>
#include <SDL.h>
#include <SDL_image.h>
#include <Lib.h>
#include <Maps.h>
#include <Server.h>
#include <States.h>
#include <stdbool.h>
#include <stdio.h>

int				lobby = 0;
int				text_index = 0;
int				text_length = 0;
int				g_mouseWheel = 0;
int				g_pmouseWheel = 0;
PlayerAction	player_action = { PAP_HIDDEN, NULL };

bool main_menu(Component* component);
bool options_menu(Component* component);
bool players_menu(Component* component);
bool info_menu(Component* component);

bool back_to_lobby(Component* component);
bool practice_mode(Component* component);

bool map_list_changed(Component* component);
bool map_list_reset(Component* component);

Component* main_components[] =
{
	(Component*)&(ImageCreate(16, 72, 348, 302, 647, 0, 348, 302)),

	(Component*)&(ButtonCreate(420, 354, 128, 20, 16, 6, "back to lobby", back_to_lobby, 647, 342, 128, 20)),
	(Component*)&(ButtonCreate(420, 316, 128, 20, 8, 6, "force test mode", practice_mode, 647, 342, 128, 20)),
};

Component* base_components[] =
{
	(Component*)&(ButtonCreate(59, 452, 32, 8, 0, 0, "main", main_menu, 0, 0, 0, 0)),
	(Component*)&(ButtonCreate(190, 452, 54, 8, 0, 0, "options", options_menu, 0, 0, 0, 0)),
	(Component*)&(ButtonCreate(372, 452, 53, 8, 0, 0, "players", players_menu, 0, 0, 0, 0)),
	(Component*)&(ButtonCreate(544, 452, 29, 8, 0, 0, "info", info_menu, 0, 0, 0, 0)),
};

Component* options_components[] =
{
	(Component*)&(MapListCreate(0, 60, 420, 354, map_list_changed)),
	(Component*)&(ImageCreate(0, 0, 640, 59, 0, 0, 640, 59)),
	(Component*)&(ImageCreate(0, 419, 640, 61, 0, 419, 640, 61)),
	(Component*)&(ImageCreate(424, 0, 7, 480, 640, 0, 7, 480)),

	(Component*)&(MapListPresetCreate(448, 64, 128, 40)),
	(Component*)&(ButtonCreate(448, 112, 128, 20, 16, 6, "disable all", map_list_reset, 647, 342, 128, 20)),
	(Component*)&(PingLimitCreate(448, 142, 128, 40)),
	(Component*)&(TButtonCreate(448, 188, 128, 20, 10, 6, "zone anticheat", (ButtonCallback)config_save, &g_config.gameplay.anticheat.zone_anticheat, true, 647, 342, 128, 20)),
	(Component*)&(TButtonCreate(448, 216, 128, 20, 0, 6, "camp tease (pride)", (ButtonCallback)config_save, &g_config.results_misc.pride, false, 647, 342, 128, 20)),
};

Component* players_components[] =
{
	(Component*)&(PlayerListCreate(16, 64)),
	(Component*)&(PlayerListConfigCreate(216, 64, playerlist_op_update)),
	(Component*)&(PlayerListConfigCreate(416, 64, playerlist_bannedips_update)),
};

Component* player_action_components[] =
{
	(Component*)&(LabelCreate(32, 64, player_data)),
	(Component*)&(LabelCreate(446, 64, "actions")),

	(Component*)&(ButtonCreate(32, 32, 29, 8, 0, 0, "back", ui_button_back, 0, 0, 0, 0)),
	(Component*)&(ButtonCreate(426, 84, 29, 8, 0, 0, "kick", ui_button_kick, 0, 0, 0, 0)),
	(Component*)&(ButtonCreate(426, 96, 29, 8, 0, 0, "ban", ui_button_ban, 0, 0, 0, 0)),
	(Component*)&(ButtonCreate(426, 108, 29, 8, 0, 0, "op", ui_button_op, 0, 0, 0, 0)),
};

Component* info_components[] =
{
	(Component*)&(LabelCreate(4, 64, "advanced ser\nver. " SERVER_VERSION)),
};

void log_msg(const char* type, const char* message)
{
	char msg[512];

	if (strcmp(type, DEBUG_TYPE) == 0)
		snprintf(msg, 512, "|%s~", message);
	else if (strcmp(type, WARN_TYPE) == 0)
		snprintf(msg, 512, "`%s~", message);
	else if (strcmp(type, ERROR_TYPE) == 0)
		snprintf(msg, 512, "\\%s~", message);
	else
		snprintf(msg, 512, "~%s", message);

	if (text_length > 31)
	{
		for (int i = 1; i < 32; i++)
			strncpy(text_buffer[i - 1], text_buffer[i], 255);
	}

	strncpy(text_buffer[text_index], msg, 255);

	if (text_index < 31)
		text_index++;

	if (text_length < 32)
		text_length++;
}

int server_loop(void)
{
	if (!disaster_init())
		return 1;

	return disaster_run();
}

int console_loop(void)
{
	SDL_Quit();

#ifdef WIN32
	AllocConsole();
	(void)freopen("CONOUT$", "w", stdout);
#endif

	return server_loop();
}

int main(int argc, char** argv)
{
	if (argc > 1)
	{
		for (int i = 1; i < argc; i++)
		{
			if (strcmp(argv[i], "--nogui") == 0)
				return console_loop();
		}
	}

	if (SDL_Init(SDL_INIT_VIDEO) < 0)
	{
		char msg[1024];
		snprintf(msg, 1024, "%s\n\nPress OK to fallback to console mode.", SDL_GetError());

		SDL_ShowSimpleMessageBox(SDL_MESSAGEBOX_ERROR, "Error (Report to dev)", msg, NULL);
		return console_loop();
	}

	if (!config_init()) {
		SDL_ShowSimpleMessageBox(SDL_MESSAGEBOX_ERROR, "Error (Report to dev)", "Unable to load config file!", NULL);
	}

	SDL_Window* window;
	SDL_Renderer* renderer;
	if (!(window = SDL_CreateWindow("AdvancedSer ver. " SERVER_VERSION, SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED, 640 * g_config.gui.interface_scale, 480 * g_config.gui.interface_scale, SDL_WINDOW_SHOWN)))
	{
		char msg[1024];
		snprintf(msg, 1024, "%s\n\nPress OK to fallback to console mode.", SDL_GetError());

		SDL_ShowSimpleMessageBox(SDL_MESSAGEBOX_ERROR, "Error (Report to dev)", msg, NULL);
		return console_loop();
	}

	if (!(renderer = SDL_CreateRenderer(window, -1, SDL_RENDERER_ACCELERATED)))
	{
		char msg[1024];
		snprintf(msg, 1024, "%s\n\nPress OK to fallback to console mode.", SDL_GetError());

		SDL_ShowSimpleMessageBox(SDL_MESSAGEBOX_ERROR, "Error (Report to dev)", msg, NULL);
		return console_loop();
	}

	if (!resources_load(renderer))
		return console_loop();

	log_hook(log_msg);
	SDL_RenderSetLogicalSize(renderer, 640 * g_config.gui.interface_scale, 480 * g_config.gui.interface_scale);
	SDL_RenderSetVSync(renderer, true);

	Label label;
	Thread thr;
	ThreadSpawn(thr, server_loop, NULL);

	bool running = true;

	while (running)
	{
		SDL_Event ev;
		while (SDL_PollEvent(&ev))
		{
			switch (ev.type)
			{
			case SDL_QUIT:
				running = false;
				break;

			case SDL_MOUSEWHEEL:
				g_mouseWheel = ev.wheel.y;
				break;
			}
		}
		
		SDL_RenderClear(renderer);
		{
			SDL_Rect src = (SDL_Rect){ 0, 0, 640, 480 };
			SDL_Rect dst = (SDL_Rect){ 0, 0, 640 * g_config.gui.interface_scale, 480 * g_config.gui.interface_scale };
			SDL_RenderCopy(renderer, g_textureSheet, &src, &dst);

			switch (state)
			{
			case UST_MAIN:
			{
				for (int i = 0; i < sizeof(main_components) / sizeof(Component*); i++)
					main_components[i]->update(renderer, main_components[i]);

				for (int i = 0; i < text_length; i++)
				{
					label = LabelCreate(23, 80 + i * 9, text_buffer[i]);
					label.update(renderer, (Component*)&label);
				}

				break;
			}

			case UST_OPTIONS:
			{
				for (int i = 0; i < sizeof(options_components) / sizeof(Component*); i++)
					options_components[i]->update(renderer, options_components[i]);

				break;
			}

			case UST_PLAYERS:
			{
				switch (player_action.panel_state)
				{
					case PAP_HIDDEN:
						for (int i = 0; i < sizeof(players_components) / sizeof(Component*); i++)
							players_components[i]->update(renderer, players_components[i]);
						break;
					case PAP_PEER:
						refresh_peer_info(player_action.peer);
						for (int i = 0; i < sizeof(player_action_components) / sizeof(Component*); i++)
							player_action_components[i]->update(renderer, player_action_components[i]);
						break;
				}
				break;
			}

			case UST_INFO:
			{
				for (int i = 0; i < sizeof(info_components) / sizeof(Component*); i++)
					info_components[i]->update(renderer, info_components[i]);

				break;
			}
			}

			g_mouseWheel = 0;

			if (!player_action.panel_state)
				for (int i = 0; i < sizeof(base_components) / sizeof(Component*); i++)
					base_components[i]->update(renderer, base_components[i]);
		}
		SDL_RenderPresent(renderer);
		SDL_Delay(32);
	}

	return 0;
}

bool main_menu(Component* component)
{
	state = UST_MAIN;
	return true;
}

bool options_menu(Component* component)
{
	state = UST_OPTIONS;
	return true;
}

bool players_menu(Component* component)
{
	state = UST_PLAYERS;
	return true;
}

bool info_menu(Component* component)
{
	state = UST_INFO;
	return true;
}

bool back_to_lobby(Component* component)
{
	Server* server = disaster_get(lobby);

	if(!server)
	{
		SDL_ShowSimpleMessageBox(SDL_MESSAGEBOX_ERROR, "Error (Report to dev)", "Server is not running (Check logs in Info tab)!", NULL);
		return false;
	}

	MutexLock(server->state_lock);
	{
		if (server->state == ST_GAME) {
			if (!game_end(server, ED_TIMEOVER, false)) {
				SDL_ShowSimpleMessageBox(SDL_MESSAGEBOX_ERROR, "Error (Report to dev)", "Failed to return to lobby: game_end() returned FALSE", NULL);
			}
		} else if(server->state != ST_LOBBY && !lobby_init(server))
			if (!lobby_init(server)) {
				SDL_ShowSimpleMessageBox(SDL_MESSAGEBOX_ERROR, "Error (Report to dev)", "Failed to return to lobby: lobby_init() returned FALSE", NULL);
			}
	}
	MutexUnlock(server->state_lock);
	return true;
}

bool practice_mode(Component* component)
{
	Server* server = disaster_get(lobby);

	if(!server)
	{
		SDL_ShowSimpleMessageBox(SDL_MESSAGEBOX_ERROR, "Error (Report to dev)", "Server is not running (Check logs in Info tab)!", NULL);
		return false;
	}

	MutexLock(server->state_lock);
	{
		if(server->state == ST_LOBBY && server_ingame(server) > 1)
		{
			if(!charselect_init(20, server))
			{
				SDL_ShowSimpleMessageBox(SDL_MESSAGEBOX_ERROR, "Error (Report to dev)", "Failed to set practice mode: charselect_init() returned FALSE", NULL);
			}
		}
	}
	MutexUnlock(server->state_lock);

	return true;
}

bool map_list_changed(Component* component)
{
	MapListPreset* preset_list = (MapListPreset*)options_components[4]; // Be careful, when you change "options" interface!!!
	preset_list->preset = PRESET_CUSTOM;

	for (int i = 0; i < PRESET_COUNT; i++)
	{
		if (memcmp(g_config.map_selection.map_list, g_defaultPresets[i].values, sizeof(g_config.map_selection.map_list)) == 0)
		{
			preset_list->preset = i;
			break;
		}
	}

	return true;
}

bool map_list_reset(Component* component)
{
	MapListPreset* preset_list = (MapListPreset*)options_components[4]; // Be careful, when you change "options" interface!!!
	preset_list->preset = PRESET_CUSTOM;

	MutexLock(g_config.map_list_lock);
	memset(g_config.map_selection.map_list, 0, sizeof(g_config.map_selection.map_list));
	MutexUnlock(g_config.map_list_lock);
	return true;
}

bool ui_update_delete(Component* component)
{
	DeleteButton* delete = (DeleteButton*)component;
	if (cJSON_HasObjectItem(delete->root, delete->key))
	{
		cJSON_DeleteItemFromObject(delete->root, delete->key);

		if (!collection_save(delete->collection, delete->root))
			SDL_ShowSimpleMessageBox(SDL_MESSAGEBOX_ERROR, "Error (Report to dev)", "Failed to save collection!", NULL);
	}
	return false;
}

void ui_button_back()
{
	player_action.panel_state = PAP_HIDDEN;
	player_action.peer = NULL;
	return;
}

bool ui_button_op(struct _Component* component)
{
	if (!player_action.peer) {
		SDL_ShowSimpleMessageBox(SDL_MESSAGEBOX_ERROR, "Error", "Peer is NULL, maybe player has left.", NULL);
		ui_button_back();
		return true;
	}

	Button* button = (Button*)component;
	Server* server = player_action.peer->server;

	bool res = true;
	MutexLock(server->state_lock);
	{
		PeerData* peer = player_action.peer;

		server_send_msg(server, peer->peer, CLRCODE_GRN "you're an operator now");
		res = op_add(peer->nickname.value, peer->ip.value);
		peer->op = g_config.lobby_misc.moderation.op_default_level;
	}
	MutexUnlock(server->state_lock);

	RAssert(res);
	return false;
}

bool ui_button_kick(struct _Component* component)
{
	if (!player_action.peer) {
		SDL_ShowSimpleMessageBox(SDL_MESSAGEBOX_ERROR, "Error", "Peer is NULL, maybe player has left.", NULL);
		ui_button_back();
		return true;
	}

	Button* button = (Button*)component;
	Server* server = player_action.peer->server;

	bool res = true;
	MutexLock(server->state_lock);
	{
		PeerData* peer = player_action.peer;

		server_disconnect(server, peer->peer, DR_KICKEDBYHOST, NULL);
		res = timeout_set(peer->nickname.value, peer->udid.value, peer->ip.value, time(NULL) + 5);
	}
	MutexUnlock(server->state_lock);

	RAssert(res);
	ui_button_back();
	return false;
}

bool ui_button_ban(struct _Component* component)
{
	if (!player_action.peer) {
		SDL_ShowSimpleMessageBox(SDL_MESSAGEBOX_ERROR, "Error", "Peer is NULL, maybe player has left.", NULL);
		ui_button_back();
	}

	Button* button = (Button*)component;
	Server* server = disaster_get(lobby);

	bool res = true;
	MutexLock(server->state_lock);
	{
		PeerData* peer = player_action.peer;

		server_disconnect(server, peer->peer, DR_BANNEDBYHOST, NULL);
		res = ban_add(peer->nickname.value, peer->udid.value, peer->ip.value);
	}
	MutexUnlock(server->state_lock);

	RAssert(res);
	ui_button_back();
	return false;
}