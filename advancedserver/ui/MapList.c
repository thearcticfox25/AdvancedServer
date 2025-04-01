#include <Log.h>
#include <Maps.h>
#include <ui/Resources.h>
#include <ui/Components.h>
#include <CMath.h>

const int low_bound = -(MAP_COUNT * 16);

bool maplist_update(SDL_Renderer* renderer, struct _Component* component)
{
    MapList* list = (MapList*)component;
	SDL_Rect bounds = { list->x * g_config.gui.interface_scale, list->y * g_config.gui.interface_scale, list->w * g_config.gui.interface_scale, list->h * g_config.gui.interface_scale };

    int mouse_x, mouse_y;
    float scale_x, scale_y;
    Uint32 flags = SDL_GetMouseState(&mouse_x, &mouse_y);
    SDL_RenderGetScale(renderer, &scale_x, &scale_y);

    mouse_x /= scale_x;
    mouse_y /= scale_y;

    bool mouse_down = false;
    if(flags & SDL_BUTTON(1))
    {
        if(!list->clicked)
        {
            mouse_down = true;
            list->clicked = true;
        }
    }
    else
        list->clicked = false;

    bool in_bounds = mouse_x >= bounds.x && mouse_y >= bounds.y && mouse_x < bounds.x + bounds.w && mouse_y < bounds.y + bounds.h;
    if(in_bounds)
    {
        list->target_scroll += g_mouseWheel * 45;
        if(list->target_scroll >= 0)
            list->target_scroll = 0;

        if(list->target_scroll < low_bound)
            list->target_scroll = low_bound;
    }

    list->scroll = lerp(list->scroll, list->target_scroll, 0.30);

    MutexLock(g_config.map_list_lock);
    for(int i = 0; i < MAP_COUNT; i++)
    {
        SDL_Rect src = { 0, 480, 123, 80 };
        SDL_Rect dst = { (8 + i % 3 * 143) * g_config.gui.interface_scale, (64 + i / 3 * 100 + list->scroll) * g_config.gui.interface_scale,
            123 * g_config.gui.interface_scale, 80 * g_config.gui.interface_scale };

        src.x = i * src.w;

        if(in_bounds && mouse_x >= dst.x && mouse_y >= dst.y && mouse_x < dst.x + dst.w && mouse_y < dst.y + dst.h)
        {
            dst.y -= g_config.gui.interface_scale;

            if (mouse_down)
            {
                g_config.map_selection.map_list[i] = !g_config.map_selection.map_list[i];
                list->cb(component);
            }

            if(flags & SDL_BUTTON(1))
                dst.y += g_config.gui.interface_scale;
        }

        const Uint8 color = g_config.map_selection.map_list[i] ? 255 : 96;
        SDL_SetTextureColorMod(g_textureSheet, color, color, color);
        SDL_RenderCopy(renderer, g_textureSheet, &src, &dst);
    }
    MutexUnlock(g_config.map_list_lock);

    SDL_SetTextureColorMod(g_textureSheet, 255, 255, 255);
    return true;
}
