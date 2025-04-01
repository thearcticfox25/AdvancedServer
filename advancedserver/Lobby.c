#include <Log.h>
#include <Server.h>
#include <States.h>
#include <Config.h>
#include <Moderation.h>
#include <Colors.h>
#include <CMath.h>
#include <stdbool.h>
#include <time.h>

#define NO_COUNTDOWN (5 + 1)
#define COUNTDOWN g_config.lobby_misc.lobby_start_timer

bool lobby_send_countdown(Server* server)
{
	Packet pack;

	PacketCreate(&pack, SERVER_LOBBY_COUNTDOWN);
	PacketWrite(&pack, packet_write8, server->lobby.countdown_sec < NO_COUNTDOWN);
	PacketWrite(&pack, packet_write8, server->lobby.countdown_sec);
	
	server_broadcast(server, &pack, true);
	return true;
}

void lobby_check_vote(Server* server)
{
	if (vote_check(&server->lobby.vote))
	{
		switch (server->lobby.vote.type)
		{
			case VOTE_KICK:
			{
				char buffer[256];
                snprintf(buffer, 256, "vote kick " CLRCODE_GRN "succeeded~ (" CLRCODE_GRN "%d ~out of %d~)", server->lobby.vote.votecnt, server->lobby.vote.votetotal);
				server_broadcast_msg(server, 0, buffer);
				
				timeout_set(server->lobby.kick_target.nickname.value, server->lobby.kick_target.udid.value, server->lobby.kick_target.ip.value, time(NULL) + 60);
				server_disconnect_id(server, server->lobby.kick_target.id, DR_KICKEDBYHOST, NULL);
				break;
			}

			case VOTE_MAP:
			{
				char buffer[256];
                snprintf(buffer, 256, "map vote " CLRCODE_GRN "succeeded~ (" CLRCODE_GRN "%d ~out of %d~)", server->lobby.vote.votecnt, server->lobby.vote.votetotal);
				server_broadcast_msg(server, 0, buffer);

				server->lobby.prac_countdown = 2 * TICKSPERSEC;
				break;
			}
		}
	}
	else
	{
        switch (server->lobby.vote.type)
		{
			case VOTE_KICK:
			{
				char buffer[256];
                snprintf(buffer, 256, "vote kick " CLRCODE_RED "failed~ (" CLRCODE_RED "%d ~out of %d~)", server->lobby.vote.votecnt, server->lobby.vote.votetotal);
				server_broadcast_msg(server, 0, buffer);
				break;
			}

			case VOTE_MAP:
			{
				char buffer[256];
                snprintf(buffer, 256, "map vote " CLRCODE_RED "failed~ (" CLRCODE_RED "%d ~out of %d~)", server->lobby.vote.votecnt, server->lobby.vote.votetotal);
				server_broadcast_msg(server, 0, buffer);
				break;
			}
		}
	}

	server->lobby.vote.ongoing = 0;
}

bool lobby_check_countdown(Server* server)
{
	if (!server) // stop warnings
		return false;

    uint8_t count = 0;
	for (size_t i = 0; i < server->peers.capacity; i++)
	{
		PeerData* peer = (PeerData*)server->peers.ptr[i];
		if (!peer)
			continue;

		if (peer->ready)
			count++;
	}

    if (server->peers.noitems * (g_config.lobby_misc.lobby_ready_required_percentage / 100.0) <= count && server->peers.noitems > 1 - g_config.gameplay.banana.singleplayer)
	{
		server->lobby.countdown = TICKSPERSEC;
		server->lobby.countdown_sec = COUNTDOWN;
		RAssert(lobby_send_countdown(server));
	}
	else if (server->lobby.countdown_sec != NO_COUNTDOWN)
	{
		server->lobby.countdown = TICKSPERSEC;
		server->lobby.countdown_sec = NO_COUNTDOWN;
		RAssert(lobby_send_countdown(server));
	}

	return true;
}

bool lobby_state_handle(PeerData* v, Packet* packet)
{
	// sub-state machine
	switch (v->server->state)
	{
	case ST_LOBBY:
	case ST_GAME:
	case ST_RESULTS:
		break;
	case ST_MAPVOTE:
		return mapvote_state_handle(v, packet);
	case ST_CHARSELECT:
		return charselect_state_handle(v, packet);
	}

	// Read header
	PacketRead(passtrough, packet, packet_read8, uint8_t);
	PacketRead(type, packet, packet_read8, uint8_t);

	bool res = true;
	Packet pack;
	switch (type)
	{
        default:
			RAssert(server_msg_handle(v->server, type, v, packet));
			break;

		case CLIENT_LOBBY_PLAYERS_REQUEST:
		{
			for (size_t i = 0; i < v->server->peers.capacity; i++)
			{
				PeerData* peer = (PeerData*)v->server->peers.ptr[i];
				if (!peer)
					continue;

				if (peer->id == v->id)
					continue;

				packet_new(&pack, SERVER_LOBBY_PLAYER);
				packet_write16(&pack, peer->id);
                packet_write8(&pack, peer->ready);
                if(g_config.lobby_misc.anonymous_mode){
                    packet_writestr(&pack, string_new("anonymous"));
                    packet_write8(&pack, 0);
                    packet_write8(&pack, -1);
                } else {
                    packet_writestr(&pack, peer->nickname);
                    packet_write8(&pack, peer->lobby_icon);
                    packet_write8(&pack, peer->pet);
                }

				if (!packet_send(v->peer, &pack, true))
				{
					res = false;
					break;
				}
			}

			PacketCreate(&pack, SERVER_LOBBY_CORRECT);
			RAssert(packet_send(v->peer, &pack, true));

            char msg[100];
            if(g_config.networking.server_count >= 2){
				server_send_msg(v->server, v->peer, UPPER_BRACKET);
				snprintf(msg, 100, "hosted by " CLRCODE_PUR  "%s" CLRCODE_RST, g_config.lobby_misc.hosts_name);
				server_send_msg(v->server, v->peer, msg);
				snprintf(msg, 100, "server " CLRCODE_RED "%d" CLRCODE_RST " of " CLRCODE_BLU "%d" CLRCODE_RST, v->server->id + 1, g_config.networking.server_count);
				server_send_msg(v->server, v->peer, msg);
				server_send_msg(v->server, v->peer, LOWER_BRACKET);
            }
			server_send_msg(v->server, v->peer, CLRCODE_GRA "type .help for command list~");

            if(g_config.lobby_misc.server_location[0] != '\0' && g_config.pairing.ping_limit != UINT16_MAX){
                snprintf(msg, 100, "%s, required ping: %d or less", g_config.lobby_misc.server_location, g_config.pairing.ping_limit);
                server_send_msg(v->server, v->peer, msg);
            }

            if(g_config.lobby_misc.message_of_the_day[0] != '\0')
                server_send_msg(v->server, v->peer, g_config.lobby_misc.message_of_the_day);

            if(g_config.lobby_misc.authoritarian_mode)
                server_send_msg(v->server, v->peer, CLRCODE_RED "authoritarian mode is on");

            switch(v->op){
                case 1:
                    server_send_msg(v->server, v->peer, CLRCODE_GRN "you're an operator on this server" CLRCODE_RST);
                    break;
                case 2:
                    server_send_msg(v->server, v->peer, CLRCODE_GRN "you're a moderator on this server" CLRCODE_RST);
                    break;
                case 3:
                    server_send_msg(v->server, v->peer, CLRCODE_GRN "you've got root perms on this server" CLRCODE_RST);
                    break;
            }
		}

		case CLIENT_CHAT_MESSAGE:
		{
			PacketRead(pid, packet, packet_read16, uint16_t);
			PacketRead(msg, packet, packet_readstr, String);
			AssertOrDisconnect(v->server, string_length(&msg) <= 80);
			v->timeout = 0;

			if(strstr(msg.value, "i want big burgr"))
			{
                if(v->op >= 1)
				{
					v->exe_chance = 101;

					Packet pack;
					PacketCreate(&pack, SERVER_LOBBY_EXE_CHANCE);
					PacketWrite(&pack, packet_write8, v->exe_chance);
					RAssert(packet_send(v->peer, &pack, true));
				}

				RAssert(server_send_msg(v->server, v->peer, CLRCODE_GRA "you were sent to " CLRCODE_RED "brazil" CLRCODE_RST));
				return true;
			}

            if(strstr(msg.value, "52") && strcmp(g_config.lobby_misc.server_location, "Saint Petersburg") == 0)
            {
                RAssert(server_send_msg(v->server, v->peer, CLRCODE_ORG "да здравствует " CLRCODE_GRN "санкт-петербург " CLRCODE_RED "и этот город наш" CLRCODE_RST));
            }

			bool ignore = true;
			unsigned long hash = server_cmd_parse(&msg);

			switch (hash)
			{
				default:
				{
					if(!server_cmd_handle(v->server, hash, v, &msg))
						ignore = false;
					break;
				}

                case CMD_CHANCE:
                {
                    if (v->op < 1)
                    {
                        RAssert(server_send_msg(v->server, v->peer, CLRCODE_RED "your permission level is too low"));
                        break;
                    }

                    int chance;
                    if (sscanf(msg.value, ":chance %d", &chance) < 0)
                    {
                        RAssert(server_send_msg(v->server, v->peer, CLRCODE_RED "example:~ :chance 74"));
                        break;
                    }

                    if (chance < 0 || chance > 100)
                    {
                        RAssert(server_send_msg(v->server, v->peer, "chance should be between 0 and 100"));
                        break;
                    }

                    v->exe_chance = chance;

                    Packet pack;
                    PacketCreate(&pack, SERVER_LOBBY_EXE_CHANCE);
                    PacketWrite(&pack, packet_write8, v->exe_chance);
                    RAssert(packet_send(v->peer, &pack, true));

                    RAssert(server_send_msg(v->server, v->peer, CLRCODE_GRN "cheat code activated." CLRCODE_RST));
                    break;
                }

				/* Force a map */
				case CMD_MAP:
				{
                    if (v->op < 1)
					{
                        RAssert(server_send_msg(v->server, v->peer, CLRCODE_RED "your permission level is too low"));
						break;
					}

					int ind;
					if (sscanf(msg.value, ".map %d", &ind) <= 0)
					{
						RAssert(server_send_msg(v->server, v->peer, CLRCODE_RED "example:~ .map 1"));
						break;
					}
					
					ind--;
                    if (ind < 0 || ind >= MAP_COUNT)
					{
						char msg[128];
                        snprintf(msg, 128, CLRCODE_RED "map should be between 1 and %d", MAP_COUNT);
						RAssert(server_send_msg(v->server, v->peer, msg));
						break;
					}

                    v->server->last_map = ind;
					return charselect_init(ind, v->server) || lobby_init(v->server);
				}

				/* YES on votes */
				case CMD_YES:
				case CMD_Y:
				{
                    if(g_config.lobby_misc.authoritarian_mode || g_config.lobby_misc.anonymous_mode)
                        break;

					if (!v->server->lobby.vote.ongoing)
						break;

					if (!v->can_vote)
					{
						server_send_msg(v->server, v->peer, CLRCODE_RED "you can't participate in this vote.");
						break;
					}

					if (v->server->lobby.vote.type == VOTE_KICK && v->server->lobby.kick_target.id == v->id)
					{
						server_send_msg(v->server, v->peer, CLRCODE_RED "why are you kicking yourself ???");
						break;
					}

					switch (vote_add(&v->server->lobby.vote, v->id))
					{
						case 1:
						{
							char buffer[512];
							snprintf(buffer, 512, "%s~ " CLRCODE_GRN "voted~.", v->nickname.value);
							server_broadcast_msg(v->server, 0, buffer);
							break;
						}

						case 0:
						{
							server_send_msg(v->server, v->peer, CLRCODE_RED "you have already voted.");
							break;
						}

						case -1:
						{
							lobby_check_vote(v->server);
							break;
						}
					}
					break;
				}

				case CMD_VM:
				case CMD_VP:
				{
                    if(g_config.lobby_misc.authoritarian_mode || g_config.lobby_misc.anonymous_mode)
                        break;

					if (v->server->lobby.vote.ongoing)
					{
						if (!v->can_vote)
						{
							server_send_msg(v->server, v->peer, CLRCODE_RED "you can't participate in this vote.");
							break;
						}

						if (pid == v->server->lobby.kick_target.id)
						{
							switch (vote_add(&v->server->lobby.vote, v->id))
							{
								case VOTE_SUCCESS:
								{
									char buffer[512];
									snprintf(buffer, 512, "%s~ " CLRCODE_GRN "voted~.", v->nickname.value);
									server_broadcast_msg(v->server, 0, buffer);
									break;
								}

								case VOTE_ALREADYVOTED:
								{
									server_send_msg(v->server, v->peer, CLRCODE_RED "you have already voted.");
									break;
								}

								case VOTE_FULL:
								{
									lobby_check_vote(v->server);
									break;
								}
							}
						}
						else
							server_send_msg(v->server, v->peer, CLRCODE_RED "another vote is already in progress.");

						break;
					}

					if (v->vote_cooldown > 0)
					{
						char buffer[128];
						snprintf(buffer, 128, CLRCODE_RED "you cannot start another vote for %d s", (int)(v->vote_cooldown / TICKSPERSEC));
						server_send_msg(v->server, v->peer, buffer);
						break;
					}

					if (!vote_init(v->server, &v->server->lobby.vote, VOTE_MAP, 0))
					{
						server_send_msg(v->server, v->peer, CLRCODE_RED "not enough participants.");
						break;
					}

					if (hash == CMD_VM) {
						int ind;
						if (sscanf(msg.value, ":vm %d", &ind) <= 0)
						{
							RAssert(server_send_msg(v->server, v->peer, CLRCODE_RED "example:~ :vm 1"));
							break;
						}

						ind--;
						if (ind < 0 || ind >= MAP_COUNT)
						{
							char msg[128];
							snprintf(msg, 128, CLRCODE_RED "map should be between 1 and %d", MAP_COUNT);
							RAssert(server_send_msg(v->server, v->peer, msg));
							break;
						}

						v->server->lobby.voting_map = ind;

					} else if (hash == CMD_VP) {
						v->server->lobby.voting_map = 20;
					}

					char buffer[356];
					snprintf(buffer, 356, "%s~ " CLRCODE_YLW "started map vote for %s" CLRCODE_RST ".", v->nickname.value, g_mapList[v->server->lobby.voting_map].name);

					server_broadcast_msg(v->server, 0, "-----------------------");
					server_broadcast_msg(v->server, 0, buffer);
					server_broadcast_msg(v->server, 0, "type " CLRCODE_GRN ".yes~ or ignore");
					snprintf(buffer, 356, "results will be summarized in " CLRCODE_GRA "%d~ sec", g_config.lobby_misc.votekick_cooldown);
					server_broadcast_msg(v->server, 0, buffer);
					server_broadcast_msg(v->server, 0, "-----------------------");

					vote_add(&v->server->lobby.vote, v->id);
                    v->vote_cooldown = g_config.lobby_misc.votekick_cooldown * TICKSPERSEC;
					break;
				}

				case CMD_VK:
				{
                    if(g_config.lobby_misc.authoritarian_mode || g_config.lobby_misc.anonymous_mode)
                        break;

					if (v->server->lobby.vote.ongoing)
					{
						server_send_msg(v->server, v->peer, CLRCODE_RED "another vote is already in progress.");
						break;
					}

					if (v->vote_cooldown > 0)
					{
						char buffer[128];
						snprintf(buffer, 128, CLRCODE_RED "you cannot start another vote for %ds", (int)(v->vote_cooldown / TICKSPERSEC));
						server_send_msg(v->server, v->peer, buffer);
						break;
					}

					// we check if number of players in game is at least two
					int ingame = server_ingame(v->server);
					if (ingame > 2)
					{
						PacketCreate(&pack, SERVER_LOBBY_CHOOSEVOTEKICK);
						RAssert(packet_send(v->peer, &pack, true));
					}
					else
						server_send_msg(v->server, v->peer, CLRCODE_RED "not enough participants.");
					break;
				}

				case CMD_START:
				{
					if (v->op < 2)
					{
						RAssert(server_send_msg(v->server, v->peer, CLRCODE_RED "your permission level is too low"));
						break;
					}

					return mapvote_init(v->server);
					break;
				}
			}

            Info("%s " LOG_RST "(id %d): %s", v->nickname.value, v->id, msg.value);
            if(!ignore && g_config.lobby_misc.apply_textchat_fixes)
                server_broadcast_msg(v->server, v->id, msg.value);
			break;
		}

		case CLIENT_LOBBY_READY_STATE:
		{
			PacketRead(state, packet, packet_read8, uint8_t);
			v->ready = state;
		
			PacketCreate(&pack, SERVER_LOBBY_READY_STATE);
			PacketWrite(&pack, packet_write16, v->id);
			PacketWrite(&pack, packet_write8, state);
			server_broadcast(v->server, &pack, true);

			RAssert(lobby_check_countdown(v->server));
			break;
		}

		case CLIENT_LOBBY_CHOOSEVOTEKICK:
		{
			PacketRead(pid, packet, packet_read16, uint16_t);

			if (v->server->lobby.vote.ongoing)
			{
				if (!v->can_vote)
				{
					server_send_msg(v->server, v->peer, CLRCODE_RED "you can't participate in this vote.");
					break;
				}

				if (pid == v->server->lobby.kick_target.id)
				{
					switch (vote_add(&v->server->lobby.vote, v->id))
					{
						case VOTE_SUCCESS:
						{
							char buffer[512];
							snprintf(buffer, 512, "%s~ " CLRCODE_GRN "voted~.", v->nickname.value);
							server_broadcast_msg(v->server, 0, buffer);
							break;
						}

						case VOTE_ALREADYVOTED:
						{
							server_send_msg(v->server, v->peer, CLRCODE_RED "you have already voted.");
							break;
						}

						case VOTE_FULL:
						{
							lobby_check_vote(v->server);
							break;
						}
					}
				}
				else
					server_send_msg(v->server, v->peer, CLRCODE_RED "another vote is already in progress.");

				break;
			}
			else
			{
				bool found = false;
				for (size_t i = 0; i < v->server->peers.capacity; i++)
				{
					PeerData* peer = (PeerData*)v->server->peers.ptr[i];
					if (!peer)
						continue;

					if (peer->id == pid)
					{
						if (peer->op)
						{
							char buffer[128];
							snprintf(buffer, 128, CLRCODE_RED "you're permissionless");
							server_send_msg(v->server, v->peer, buffer);
							return true;
						}

						memcpy(&v->server->lobby.kick_target, peer, sizeof(PeerData));
						found = true;
						break;
					}
				}

				if (found)
				{
					if (v->vote_cooldown > 0)
					{
						char buffer[128];
						snprintf(buffer, 128, CLRCODE_RED "you cannot start another vote for %ds", (int)(v->vote_cooldown / TICKSPERSEC));
						server_send_msg(v->server, v->peer, buffer);
						break;
					}

					if(!vote_init(v->server, &v->server->lobby.vote, VOTE_KICK, pid))
					{
						server_send_msg(v->server, v->peer, CLRCODE_RED "not enough participants.");
						break;
					}

					char buffer[555];
					snprintf(buffer, 555, "%s~ " CLRCODE_RED "started kick vote for " CLRCODE_RST "%s" CLRCODE_RST, v->nickname.value, v->server->lobby.kick_target.nickname.value);

					server_broadcast_msg(v->server, 0, "-----------------------");
					server_broadcast_msg(v->server, 0, buffer);
					server_broadcast_msg(v->server, 0, "type " CLRCODE_GRN ".yes~ or ignore");
					snprintf(buffer, 356, "results will be summarized in " CLRCODE_GRA "%d~ sec", g_config.lobby_misc.votekick_cooldown);
					server_broadcast_msg(v->server, 0, buffer);
					server_broadcast_msg(v->server, 0, "-----------------------");

					vote_add(&v->server->lobby.vote, v->id);
                    v->vote_cooldown = g_config.lobby_misc.votekick_cooldown * TICKSPERSEC;
				}
				else
                    server_send_msg(v->server, v->peer, CLRCODE_RED "specified player not found.");
			}

			break;
		}

		
	}

	return res;
}

bool lobby_state_tick(Server* server)
{
	uint8_t should = 0;

	// sub-state machine
	switch (server->state)
	{
		case ST_LOBBY:
		{
			for (size_t i = 0; i < server->peers.capacity; i++)
			{
				PeerData* peer = (PeerData*)server->peers.ptr[i];
				if (!peer)
					continue;

				if (peer->vote_cooldown > 0)
					peer->vote_cooldown -= server->delta;

				if (!peer->ready)
				{
					peer->timeout += server->delta;
					if ((int)peer->timeout % 60 == 0)
						Debug("tick for %s: %f", peer->nickname.value, peer->timeout/60.0f);

                    if (peer->timeout >= g_config.lobby_misc.lobby_timeout_timer * TICKSPERSEC)
						server_disconnect(server, peer->peer, DR_AFKTIMEOUT, NULL);
				}
				else
					peer->timeout = 0;
			}
			break;
		}
		case ST_MAPVOTE:
			return mapvote_state_tick(server);

		case ST_CHARSELECT:
			return charselect_state_tick(server);

		case ST_GAME:
		case ST_RESULTS:
			break;
	}

	// Small delay before entering practice level
	if (server->lobby.prac_countdown > 0)
	{
		server->lobby.prac_countdown -= server->delta;
		if (server->lobby.prac_countdown <= 0)
			return charselect_init(server->lobby.voting_map, server) || lobby_init(server);
	}

	// If vote is going on, call tick function
	if (server->lobby.vote.ongoing && !vote_tick(server, &server->lobby.vote))
		lobby_check_vote(server);

	// Do countdown every second
    if (server->lobby.countdown_sec <= COUNTDOWN && !g_config.lobby_misc.authoritarian_mode)
	{
		if (server->lobby.countdown <= 0)
		{
			server->lobby.countdown += TICKSPERSEC;

            if (--server->lobby.countdown_sec == 0){
                if(g_config.lobby_misc.kick_unready_before_starting)
                    for(size_t i = 0; i < server->peers.capacity; i++){
                        PeerData* peer = (PeerData*)server->peers.ptr[i];
                        if (!peer)
                            continue;

                        if(peer->ready)
                            continue;

                        server_disconnect(server, peer->peer, DR_AFKTIMEOUT, NULL);
                    }
                return mapvote_init(server) || lobby_init(server);
            }
			RAssert(lobby_send_countdown(server));
		}

		server->lobby.countdown -= server->delta;
	}

	return true;
}

bool lobby_init(Server* server)
{
	srand((unsigned int)time(NULL));

	Debug("Attepting to enter ST_LOBBY...");
	RAssert(server);

	for (size_t i = 0; i < server->peers.capacity; i++)
	{
		PeerData* v = (PeerData*)server->peers.ptr[i];
		if (!v)
			continue;
		
		v->ready = false;
		v->voted = false;
		v->timeout = false;

		if (!v->in_game)
		{
			v->in_game = true;

			Packet pack;
			PacketCreate(&pack, SERVER_IDENTITY_RESPONSE);
			PacketWrite(&pack, packet_write8, 1);
			PacketWrite(&pack, packet_write16, v->id);
			
			if (!packet_send(v->peer, &pack, true))
			{
				server_disconnect_id(server, v->id, DR_SERVERTIMEOUT, NULL);
				continue;
			}
		}
		else
		{
			if (v->id != server->game.exe)
				v->exe_chance += 2 + rand() % 5;
			else
				v->exe_chance = 1 + rand() % 1;

			Packet pack;
			PacketCreate(&pack, SERVER_LOBBY_EXE_CHANCE);
			PacketWrite(&pack, packet_write8, v->exe_chance);
			packet_send(v->peer, &pack, true);
		}
	}

	server->state = ST_LOBBY;
	server->lobby.countdown = TICKSPERSEC;
	server->lobby.countdown_sec = NO_COUNTDOWN;
	server->lobby.prac_countdown = 0;
	memset(&server->lobby.vote, 0, sizeof(Vote));

	Packet pack;
	PacketCreate(&pack, SERVER_GAME_BACK_TO_LOBBY);
	server_broadcast(server, &pack, true);

	Info(LOG_YLW "Server is now in " LOG_PUR "Lobby");
	return true;
}

bool lobby_state_join(PeerData* v)
{
	if (!v->in_game)
		return true;

	v->timeout = 0;
	switch (v->server->state)
	{
	case ST_LOBBY:
		break;
	case ST_GAME:
	case ST_RESULTS:
		break;
	case ST_MAPVOTE:
		return mapvote_state_join(v);
	case ST_CHARSELECT:
		return charselect_state_join(v);
	}

	RAssert(lobby_check_countdown(v->server));
	return true;
}

bool lobby_state_left(PeerData* v)
{
	if (!v->in_game)
		return true;

	switch (v->server->state)
	{
	case ST_LOBBY:
	case ST_GAME:
	case ST_RESULTS:
		break;
	case ST_MAPVOTE:
		return mapvote_state_left(v);
	case ST_CHARSELECT:
		return charselect_state_left(v);
	}

	RAssert(lobby_check_countdown(v->server));
	return true;
}
