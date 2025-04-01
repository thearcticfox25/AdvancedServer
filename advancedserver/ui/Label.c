#include <ui/Components.h>
#include <UTF8.h>

bool label_update(SDL_Renderer* renderer, struct _Component* component)
{
	Label* label = (Label*)component;
	int x = label->x;
	int y = label->y;
	SDL_Color clr = COLOR_WHITE;

	for (int i = 0; i < utf8_strlen(label->text); i++)
	{
		utf8_char c = utf8_tolower(utf8_get(label->text, i));
		//Source params
		int pos_x = 8;
		int pos_y = 36;
		int width = 8;
		int height = 9;
		switch (c)
		{
		case '\n':
			x = label->x;
			y += height + 2;
			continue;

		case ' ':
			x += width - 2;
			continue;

		case '	':
			x += width + 3;
			continue;

		case '-':
		case '_':
			pos_x = 56;
			width = 6;
			break;

		case ',':
			pos_x = 3;
			width = 3;
			break;

		case '!':
		case '.':
			pos_x = 0;
			width = 3;
			break;

		case '\'':
			pos_x = 42;
			width = 3;
			break;

		case ':':
			pos_x = 6;
			width = 3;
			break;

		case '(':
		case '[':
		case '<':
			pos_x = 19;
			width = 4;
			break;

		case ')':
		case ']':
		case '>':
			pos_x = 23;
			width = 4;
			break;

		case '%':
			pos_x = 33;
			width = 7;
			break;

		case '+':
			pos_x = 13;
			width = 5;
			break;

		case 0x04D9:
			pos_x = 0;
			pos_y = 45;
			height++;
			break;

		case 0x0454:
			pos_x = 23;
			pos_y = 45;
			width++;
			height++;
			break;

		case 0x0457:
			pos_x = 32;
			pos_y = 45;
			width -= 2;
			height++;
			break;

		case '\\':
			clr = COLOR_RED;
			continue;

		case '@':
			clr = COLOR_GRN;
			continue;

		case '&':
			clr = COLOR_PUR;
			continue;

		case '/':
			clr = COLOR_BLU;
			continue;

		case '|':
			clr = COLOR_GRA;
			continue;

		case '`':
			clr = COLOR_YLW;
			continue;

		case '~':
			clr = COLOR_WHITE;
			continue;

		case 0x2116:
			clr = COLOR_ORG;
			continue;

		default:
			if (c >= 'a' && c <= 'z') {
				pos_y = 0;
				pos_x = (c - 97) * width;
				switch (c)
				{
				case 'e':
					width--;
					break;
				case 'f':
					pos_x--;
					width--;
					break;
				case 'g':
					pos_x -= 2;
					break;
				case 'h':
					pos_x -= 2;
					width++;
					break;
				case 'i':
					pos_x--;
					width -= 2;
					break;
				case 'j':
				case 'k':
					pos_x -= 3;
					break;
				case 'l':
					pos_x -= 3;
					width--;
					break;
				case 'm':
					pos_x -= 4;
					width += 2;
					break;
				case 'n':
					pos_x -= 2;
					width++;
					break;
				case 'o':
				case 'p':
				case 'q':
				case 'r':
				case 's':
				case 't':
				case 'u':
				case 'v':
					pos_x--;
					break;
				case 'w':
					pos_x--;
					width += 4;
					break;
				case 'x':
				case 'y':
				case 'z':
					pos_x += 3;
					break;
				}

			}
			else if (c >= 0x03B1 && c <= 0x03C9) {
				pos_x = (c - 0x03B1) * width;
				pos_y = 9;
				switch (c)
				{
				case 0x03B5:
					width--;
					break;
				case 0x03B6:
					pos_x--;
					break;
				case 0x03B7:
					pos_x--;
					width++;
					break;
				case 0x03B9:
					width -= 2;
					break;
				case 0x03BA:
				case 0x03BB:
					pos_x -= 2;
					break;
				case 0x03BC:
					pos_x -= 2;
					width += 2;
					break;
				case 0x03BD:
					width++;
					break;
				case 0x03BE:
				case 0x03BF:
					pos_x++;
					break;
				case 0x03C0:
					pos_x++;
					width++;
					break;
				case 0x03C1:
				case 0x03C2:
					pos_x += 2;
					break;
				case 0x03C3:
				case 0x03C4:
				case 0x03C5:
					pos_x -= 7;
					break;
				case 0x03C6:
					pos_x -= 6;
					width += 3;
					break;
				case 0x03C7:
					pos_x -= 3;
					break;
				case 0x03C8:
					pos_x -= 3;
					width += 2;
					break;
				case 0x03C9:
					pos_x--;
					width += 2;
					break;
				}
			}
			else if (c >= 0x0430 && c <= 0x044F) {
				pos_y = 18;
				height = 11;
				pos_x = (c - 0x0430) * width;
				switch (c)
				{
				case 0x0435: // Це Е
					width--;
					break;
				case 0x0436:
					pos_x--;
					width += 4;
					break;
				case 0x0437:
					pos_x += 3;
					width--;
					break;
				case 0x0438:
					pos_x += 2;
					width++;
					break;
				case 0x0439:
					pos_x += 3;
					width++;
					break;
				case 0x043A:
				case 0x043B:
					pos_x += 4;
					break;
				case 0x043C:
					pos_x += 4;
					width += 2;
					break;
				case 0x043D:
					pos_x--;
					width++;
				case 0x043E:
					pos_x += 7;
					break;
				case 0x043F:
					pos_x += 7;
					width++;
					break;
				case 0x0440:
					pos_x += 7;
					width++;
					break;
				case 0x0441:
					pos_x += 7;
					width++;
					break;
				case 0x0442:
				case 0x0443:
					pos_x += 8;
					break;
				case 0x0444:
					pos_x += 8;
					width += 3;
					break;
				case 0x0445:
					pos_x += 11;
					break;
				case 0x0446:
				case 0x0447:
					pos_x += 11;
					width++;
					break;
				case 0x0448:
					pos_x += 12;
					width += 4;
					break;
				case 0x0449:
					pos_x += 16;
					width += 5;
					break;
				case 0x044A:
					pos_x += 21;
					width++;
					break;
				case 0x044B:
					pos_x += 22;
					width += 3;
					break;
				case 0x044C:
					pos_x += 25;
					break;
				case 0x044D:
					pos_x += 25;
					width++;
					break;
				case 0x044E:
					pos_x += 26;
					width += 4;
					break;
				case 0x044F:
					pos_x += 30;
					break;
				}
			}
			else if (c >= '0' && c <= '9') {
				pos_x = (c - '0') * width;
				pos_y = 27;
				switch (c)
				{
				case '1':
					width -= 2;
					break;
				case '2':
				case '3':
					pos_x -= 2;
					break;
				case '4':
					pos_x -= 2;
					width++;
					break;
				case '5':
					pos_x -= 1;
					break;
				case '6':
				case '7':
				case '8':
				case '9':
					pos_x--;
					break;
				}
			}
		}
		SDL_SetTextureColorMod(g_defFontSheet, clr.r, clr.g, clr.b);
		SDL_Rect src = (SDL_Rect){ pos_x, pos_y, width, height };
		SDL_Rect dst = (SDL_Rect){ x * g_config.gui.interface_scale, y * g_config.gui.interface_scale,
		width * g_config.gui.interface_scale, height * g_config.gui.interface_scale };
		SDL_RenderCopy(renderer, g_defFontSheet, &src, &dst);
		x += width;
	}

	SDL_SetTextureColorMod(g_defFontSheet, 255, 255, 255);
	return true;
}
