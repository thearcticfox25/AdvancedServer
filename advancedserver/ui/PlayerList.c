#include "Config.h"
#include "Moderation.h"
#include "cJSON.h"
#include "io/Threads.h"
#include <io/File.h>
#include <ui/Main.h>
#include <ui/Components.h>
#include <ui/Presets.h>
#include <Colors.h>
#include <stdio.h>

void refresh_peer_info(PeerData* peer) {
    snprintf(player_data, 1024, "peer's data:\n\
    nickname: %s (id: %d)\n\
    endpoint: %s\n\
    device id: %s\n\
    op level: %d\n\
    exe chance: %d\n\
    in game: %d\n\
    surv char id: %d\n\
    exe char id: %d\n\
", peer->nickname, peer->id, peer->ip, peer->udid, peer->op, peer->exe_chance, peer->in_game, peer->surv_char, peer->exe_char);
    return;
}

bool playerlist_update(SDL_Renderer* renderer, struct _Component* component)
{
    PlayerList* list = (PlayerList*)component;
    SDL_Rect src = { 995, 0, 182, 302 };
    SDL_Rect dst = { list->x * g_config.gui.interface_scale, list->y * g_config.gui.interface_scale, list->w * g_config.gui.interface_scale, list->h * g_config.gui.interface_scale };


    SDL_Point mouse;
    float scale_x, scale_y;
    Uint32 flags = SDL_GetMouseState(&mouse.x, &mouse.y);
    SDL_RenderGetScale(renderer, &scale_x, &scale_y);

    mouse.x /= scale_x;
    mouse.y /= scale_y;

    if (!(flags & SDL_BUTTON(1)))
        list->clicked = false;

    SDL_RenderCopy(renderer, g_textureSheet, &src, &dst);

    Label title = LabelCreate(list->x + 48, list->y + 6, "player list");
    title.update(renderer, (Component*)&title);

    Label label = LabelCreate(list->x + 8, list->y + 8, "");

    Server* server = disaster_get(lobby);

    for(int i = 0; i < server->peers.capacity; i++)
    {
        PeerData* peer = (PeerData*)server->peers.ptr[i];
        if (!peer)
            continue;

        if(peer->nickname.len == 0)
            continue;

        label.y += 10 * g_config.gui.interface_scale;

        SDL_Rect check = (SDL_Rect){ (label.x - 2) * g_config.gui.interface_scale, (label.y - 4) * g_config.gui.interface_scale, 132 * g_config.gui.interface_scale, 12 * g_config.gui.interface_scale };
        char text[128];

        snprintf(text, 128, "%s", peer->nickname.value);

        if(!list->clicked && SDL_PointInRect(&mouse, &check))
        {
            SDL_SetRenderDrawColor(renderer, 73, 0, 0, 255);
            SDL_RenderFillRect(renderer, &check);
            if (flags & SDL_BUTTON(1)) {
                list->clicked = true;
                player_action.peer = peer;
                player_action.panel_state = PAP_PEER;
            }
        }

        SDL_SetRenderDrawColor(renderer, 255, 255, 255, 255);
        label.text = text;
        label.update(renderer, (Component*)&label);
    }

    return true;
}

bool playerlist_bannedips_update(SDL_Renderer* renderer, struct _Component* component)
{
    PlayerListConfig* list = (PlayerListConfig*)component;
    SDL_Rect src = { 995, 0, 182, 302 };
    SDL_Rect dst = { list->x * g_config.gui.interface_scale, list->y * g_config.gui.interface_scale, list->w * g_config.gui.interface_scale, list->h * g_config.gui.interface_scale };

    SDL_Point mouse;
    float scale_x, scale_y;
    Uint32 flags = SDL_GetMouseState(&mouse.x, &mouse.y);
    SDL_RenderGetScale(renderer, &scale_x, &scale_y);

    mouse.x /= scale_x;
    mouse.y /= scale_y;

    if (!(flags & SDL_BUTTON(1)))
        list->clicked = false;

    SDL_RenderCopy(renderer, g_textureSheet, &src, &dst);

    Label title = LabelCreate(list->x + 56, list->y + 6, "banned ips");
    title.update(renderer, (Component*)&title);

    Label label = LabelCreate(list->x + 8, list->y + 8, "");
    DeleteButton delete = (DeleteButton){ 1177, 56, 43, 8, button_update, list->x + 20 * 2, list->y - 2 + 8, 43, 8, ui_update_delete, false, BANNED_IPS_FILE, g_banned_ips, NULL };

    MutexLock(g_banMut);
    {
        if (SDL_PointInRect(&mouse, &dst))
        {
            list->page -= g_mouseWheel;
            list->page = SDL_clamp(list->page, 0, SDL_max((int)ceil(cJSON_GetArraySize(g_banned_ips) / 9.0) - 1, 0));
        }

        size_t i = 0;
        for (cJSON* ban = g_banned_ips->child; ban; ban = ban->next)
        {
            if (!ban)
                continue;

            if (i < list->page * 9)
            {
                i++;
                continue;
            }
            else if (i >= list->page * 9 + 9)
                break;

            label.y += 8 * g_config.gui.interface_scale;
            delete.d_y += 8 * g_config.gui.interface_scale;

            SDL_Rect check = (SDL_Rect){ (label.x - 2) * g_config.gui.interface_scale, (label.y - 4) * g_config.gui.interface_scale, 132 * g_config.gui.interface_scale, 12 * g_config.gui.interface_scale };
            
            if(!list->clicked && SDL_PointInRect(&mouse, &check))
            {                
                SDL_SetRenderDrawColor(renderer, 73, 0, 0, 255);
                SDL_RenderFillRect(renderer, &check);
                if (flags & SDL_BUTTON(1))
                    ban_revoke(ban->string);
            }
            
            i++;
        }
    }
    MutexUnlock(g_banMut);

    return true;
}

bool playerlist_bannedudids_update(SDL_Renderer* renderer, struct _Component* component)
{
    PlayerListConfig* list = (PlayerListConfig*)component;
    SDL_Rect src = { 995, 0, 182, 302 };
    SDL_Rect dst = { list->x * g_config.gui.interface_scale, list->y * g_config.gui.interface_scale, list->w * g_config.gui.interface_scale, list->h * g_config.gui.interface_scale };

    SDL_Point mouse;
    float scale_x, scale_y;
    Uint32 flags = SDL_GetMouseState(&mouse.x, &mouse.y);
    SDL_RenderGetScale(renderer, &scale_x, &scale_y);

    mouse.x /= scale_x;
    mouse.y /= scale_y;

    if (!(flags & SDL_BUTTON(1)))
        list->clicked = false;

    SDL_RenderCopy(renderer, g_textureSheet, &src, &dst);

    Label title = LabelCreate(list->x + 56, list->y + 6, "banned ips");
    title.update(renderer, (Component*)&title);

    Label label = LabelCreate(list->x + 8, list->y + 8, "");

    MutexLock(g_banMut);
    {
        if (SDL_PointInRect(&mouse, &dst))
        {
            list->page -= g_mouseWheel;
            list->page = SDL_clamp(list->page, 0, SDL_max((int)ceil(cJSON_GetArraySize(g_banned_ips) / 9.0) - 1, 0));
        }

        size_t i = 0;
        for (cJSON* ban = g_banned_ips->child; ban; ban = ban->next)
        {
            if (!ban)
                continue;

            if (i < list->page * 9)
            {
                i++;
                continue;
            }
            else if (i >= list->page * 9 + 9)
                break;

            label.y += 8 * g_config.gui.interface_scale;

            SDL_Rect check = (SDL_Rect){ (label.x - 2) * g_config.gui.interface_scale, (label.y - 4) * g_config.gui.interface_scale, 132 * g_config.gui.interface_scale, 12 * g_config.gui.interface_scale };

            if (!list->clicked && SDL_PointInRect(&mouse, &check))
            {
                SDL_SetRenderDrawColor(renderer, 73, 0, 0, 255);
                SDL_RenderFillRect(renderer, &check);
                if (flags & SDL_BUTTON(1))
                    ban_revoke(ban->string);
            }
            else
            {
                char text[128];
                snprintf(text, 128, "%s" CLRCODE_RST " (%s)", cJSON_GetStringValue(ban), strstr(ban->string, "-") != NULL ? "udid" : "ip");
                SDL_SetRenderDrawColor(renderer, 255, 255, 255, 255);

                label.text = text;
                label.update(renderer, (Component*)&label);
            }

            i++;
        }
    }
    MutexUnlock(g_banMut);

    return true;
}

bool playerlist_op_update(SDL_Renderer* renderer, struct _Component* component)
{
    PlayerListConfig* list = (PlayerListConfig*)component;
    SDL_Rect src = { 995, 0, 182, 302 };
    SDL_Rect dst = { list->x * g_config.gui.interface_scale, list->y * g_config.gui.interface_scale, list->w * g_config.gui.interface_scale, list->h * g_config.gui.interface_scale };

    SDL_Point mouse;
    float scale_x, scale_y;
    Uint32 flags = SDL_GetMouseState(&mouse.x, &mouse.y);
    SDL_RenderGetScale(renderer, &scale_x, &scale_y);

    mouse.x /= scale_x;
    mouse.y /= scale_y;

    if (!(flags & SDL_BUTTON(1)))
        list->clicked = false;

    SDL_RenderCopy(renderer, g_textureSheet, &src, &dst);

    Label title = LabelCreate(list->x + 10, list->y + 6, "server administrators");
    title.update(renderer, (Component*)&title);

    Label label = LabelCreate(list->x + 8, list->y + 8, "");
    DeleteButton delete = (DeleteButton){1177, 56, 43, 8, button_update, list->x + 20 * 2, list->y - 2 + 8, 43, 8, ui_update_delete, false, OPERATORS_FILE, g_ops, NULL};

    MutexLock(g_opMut);
    {        
        if (SDL_PointInRect(&mouse, &dst))
        {
            list->page -= g_mouseWheel;
            list->page = SDL_clamp(list->page, 0, SDL_max((int)ceil(cJSON_GetArraySize(g_ops) / 9.0) - 1, 0));
        }

        size_t i = 0;
        for (cJSON* op = g_ops->child; op; op = op->next)
        {
            if (!op)
                continue;

            if (i < list->page * 9)
            {
                i++;
                continue;
            }
            else if (i >= list->page * 9 + 9)
                break;

            label.y += 8 * g_config.gui.interface_scale;
            delete.d_y += 8 * g_config.gui.interface_scale;

            SDL_Rect check = (SDL_Rect){ (label.x - 2) * g_config.gui.interface_scale, (label.y - 4) * g_config.gui.interface_scale, 132 * g_config.gui.interface_scale, 12 * g_config.gui.interface_scale };
            char text[128];
            if(!list->clicked && SDL_PointInRect(&mouse, &check))
            {                
                SDL_SetRenderDrawColor(renderer, 73, 0, 0, 255);
                SDL_RenderFillRect(renderer, &check);
                if (flags & SDL_BUTTON(1)) {
                    op_revoke(op->string);
                }
            }
            else
            {
                SDL_SetRenderDrawColor(renderer, 255, 255, 255, 255);
                label.text = cJSON_GetStringValue(cJSON_GetArrayItem(op, 1));
                label.update(renderer, (Component*)&label);
            }

            i++;
        }
    }
    MutexUnlock(g_opMut);
    return true;
}