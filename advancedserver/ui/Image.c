#include <ui/Components.h>

bool image_update(SDL_Renderer* renderer, struct _Component* component)
{
	Image* image = (Image*)component;

	SDL_Rect src = { image->x, image->y, image->w, image->h };
	SDL_Rect dst = { image->d_x * g_config.gui.interface_scale, image->d_y * g_config.gui.interface_scale, image->d_w * g_config.gui.interface_scale, image->d_h * g_config.gui.interface_scale };
	SDL_RenderCopy(renderer, g_textureSheet, &src, &dst);

	return true;
}
