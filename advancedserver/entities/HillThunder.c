#include <entities/HillThunder.h>
#include <CMath.h>

bool thunder_tick(Server* server, Entity* entity)
{
	Thunder* th = (Thunder*)entity;

	if (th->timer <= 2 * TICKSPERSEC && !th->flag)
	{
		Packet pack;
		PacketCreate(&pack, SERVER_GHZTHUNDER_STATE);
		PacketWrite(&pack, packet_write8, 0);
		server_broadcast(server, &pack, true);
		th->flag = true;
	}
	else if (th->timer <= 0)
	{
		Packet pack;
		PacketCreate(&pack, SERVER_GHZTHUNDER_STATE);
		PacketWrite(&pack, packet_write8, 1);
		server_broadcast(server, &pack, true);

        th->timer = (g_config.gameplay.entities_misc.map_specific.hills.thunder.timer
			+ (!g_config.gameplay.entities_misc.map_specific.hills.thunder.timer_offset
			? 0 : (rand() % g_config.gameplay.entities_misc.map_specific.hills.thunder.timer_offset))) * TICKSPERSEC;
		th->flag = false;
		return true;
	}

	th->timer -= server->delta;
	return true;
}
