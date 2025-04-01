#include "Server.h"
#include <entities/DTBall.h>

bool dtball_tick(Server* server, Entity* entity)
{
	DTBall* ball = (DTBall*)entity;

	if (ball->side)
	{
		ball->state += g_config.gameplay.entities_misc.map_specific.dark_tower.balls.shift_per_tick * server->delta;

		if (ball->state >= 1)
			ball->side = 0;
	}
	else
	{
		ball->state -= g_config.gameplay.entities_misc.map_specific.dark_tower.balls.shift_per_tick * server->delta;

		if (ball->state <= -1)
			ball->side = 1;
	}

	Packet pack;
	PacketCreate(&pack, SERVER_DTBALL_STATE);
	PacketWrite(&pack, packet_writefloat, (float)ball->state);
	server_broadcast(server, &pack, false);

	return true;
}
