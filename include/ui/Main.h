#ifndef MAIN_H
#define MAIN_H
#include <cJSON.h>
#include <ui/Components.h>
#include <Server.h>
#include <Lib.h>

enum
{
	UST_MAIN,
	UST_OPTIONS,
	UST_PLAYERS,
	UST_INFO
}				state;
typedef struct
{
	enum
	{
		PAP_HIDDEN,
		PAP_PEER
	}			panel_state;
	PeerData*	peer;
} PlayerAction;

int				lobby;
char			text_buffer[32][256];
char			player_data[1024];
int				text_index;
int				text_length;
int 			g_mouseWheel;
int 			g_pmouseWheel;
PlayerAction	player_action;

bool ui_update_delete(struct _Component* component);

bool ui_button_op(struct _Component* component);
bool ui_button_kick(struct _Component* component);
bool ui_button_ban(struct _Component* component);

void ui_button_back();

#endif