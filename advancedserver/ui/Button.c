#include <ui/Components.h>

bool button_update(SDL_Renderer* renderer, struct _Component* component)
{
    Button* button = (Button*)component;
	SDL_Rect src = { button->x, button->y, button->w, button->h };
	SDL_Rect dst = { button->d_x * g_config.gui.interface_scale, button->d_y * g_config.gui.interface_scale, button->d_w * g_config.gui.interface_scale, button->d_h * g_config.gui.interface_scale };

    Label title = { button->d_x + button->l_x, button->d_y + button->l_y, 0, 0, label_update, button->text };

    int mouse_x, mouse_y;
    float scale_x, scale_y;
    Uint32 flags = SDL_GetMouseState(&mouse_x, &mouse_y);
    SDL_RenderGetScale(renderer, &scale_x, &scale_y);

    mouse_x /= scale_x;
    mouse_y /= scale_y;

    if(mouse_x >= dst.x && mouse_y >= dst.y && mouse_x < dst.x + dst.w && mouse_y < dst.y + dst.h)
    {
        SDL_SetTextureColorMod(g_textureSheet, 36, 99, 255);
        if(flags & SDL_BUTTON(1))
        {
            if(!button->clicked && button->cb)
            {
                button->clicked = true;

                if (!button->cb(component))
                    return false;
            }
            dst.y += g_config.gui.interface_scale;
        }
        else
            button->clicked = false;
    }
    else
        button->clicked = false;

	SDL_RenderCopy(renderer, g_textureSheet, &src, &dst);
    title.update(renderer, (Component*)&title);
    SDL_SetTextureColorMod(g_textureSheet, 255, 255, 255);
    return true;
}
