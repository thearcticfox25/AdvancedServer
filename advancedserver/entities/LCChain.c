#include <entities/LCChain.h>

bool lcchain_tick(Server* server, Entity* entity)
{
	LCChain* chain = (LCChain*)entity;

	switch (chain->state)
	{
		case LCC_NONE:
            if (chain->timer >= g_config.gameplay.entities_misc.map_specific.limb_city.chain.delay * TICKSPERSEC)
			{
				Packet pack;
				PacketCreate(&pack, SERVER_LCCHAIN_STATE);
				PacketWrite(&pack, packet_write8, 0);
				server_broadcast(server, &pack, true);

				chain->timer = 0;
                chain->state = LCC_PREPARE;
			}
			break;

		case LCC_PREPARE:
		{
            if (chain->timer >= g_config.gameplay.entities_misc.map_specific.limb_city.chain.warning * TICKSPERSEC)
			{
				Packet pack;
				PacketCreate(&pack, SERVER_LCCHAIN_STATE);
				PacketWrite(&pack, packet_write8, 1);
				server_broadcast(server, &pack, true);

				chain->timer = 0;
                chain->state = LCC_ACTIVATE;
			}
			break;
		}

		case LCC_ACTIVATE:
		{
            if (chain->timer >= g_config.gameplay.entities_misc.map_specific.limb_city.chain.shocking_time * TICKSPERSEC)
			{
				Packet pack;
				PacketCreate(&pack, SERVER_LCCHAIN_STATE);
				PacketWrite(&pack, packet_write8, 2);
				server_broadcast(server, &pack, true);

				chain->timer = 0;
                chain->state = LCC_NONE;
			}
			break;
		}
	}

	chain->timer += server->delta;
	return true;
}
