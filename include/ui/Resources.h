#ifndef RESOURCES_H
#define RESOURCES_H
#include <Server.h>
#include <SDL.h>
#include <stdbool.h>

extern SDL_Texture* g_textureSheet;
extern SDL_Texture* g_defFontSheet;
bool resources_load(SDL_Renderer* renderer);

#endif
