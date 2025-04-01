#include <ui/Components.h>
#include <ui/Presets.h>

bool mappreset_update(SDL_Renderer* renderer, struct _Component* component)
{
    MapListPreset* list = (MapListPreset*)component;
    SDL_Rect src = { 647, 302, 128, 40 };
    SDL_Rect dst = { list->x * g_config.gui.interface_scale, list->y * g_config.gui.interface_scale, list->w * g_config.gui.interface_scale, list->h * g_config.gui.interface_scale };

    Label title = { list->x + 32, list->y + 8, 0, 0, label_update, "map list"};

    int mouse_x, mouse_y;
    float scale_x, scale_y;
    Uint32 flags = SDL_GetMouseState(&mouse_x, &mouse_y);
    SDL_RenderGetScale(renderer, &scale_x, &scale_y);

    bool mouse_down = false;
    if (flags & SDL_BUTTON(1))
    {
        if (!list->clicked)
        {
            mouse_down = true;
            list->clicked = true;
        }
    }
    else
        list->clicked = false;

    mouse_x /= scale_x;
    mouse_y /= scale_y;

    if (mouse_x >= dst.x && mouse_y >= dst.y && mouse_x < dst.x + dst.w && mouse_y < dst.y + dst.h)
    {
        SDL_SetTextureColorMod(g_textureSheet, 36, 99, 255);

        SDL_Rect arrow_src = { 1177, 0, 116, 32 };
        SDL_Rect arrow_dst = { dst.x, dst.y, 116 * g_config.gui.interface_scale, 32 * g_config.gui.interface_scale };

        if (mouse_down)
        {
            arrow_dst.y += g_config.gui.interface_scale;

            MutexLock(g_config.map_list_lock);
            {
                if (mouse_x < dst.x + dst.w / 2)
                {
                    if (--list->preset < 0)
                        list->preset = PRESET_COUNT - 2;
                }
                else
                {
                    if (++list->preset >= PRESET_COUNT - 1)
                        list->preset = 0;
                }

                memcpy(g_config.map_selection.map_list, g_defaultPresets[list->preset].values, sizeof(g_config.map_selection.map_list));
            }
            MutexUnlock(g_config.map_list_lock);
        }

        SDL_RenderCopy(renderer, g_textureSheet, &src, &dst);
        SDL_RenderCopy(renderer, g_textureSheet, &arrow_src, &arrow_dst);
    }
    else
        SDL_RenderCopy(renderer, g_textureSheet, &src, &dst);

    title.update(renderer, (Component*)&title);

    list->label.x = dst.x / g_config.gui.interface_scale + 8;
    list->label.y = dst.y / g_config.gui.interface_scale + 25;
    list->label.text = g_defaultPresets[list->preset].name;
    label_update(renderer, (Component*)&list->label);
    SDL_SetTextureColorMod(g_textureSheet, 255, 255, 255);
    return true;
}