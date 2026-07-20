use crate::config::cfg;
use crate::entities::nap_ice::NapIce;
use crate::entities::nap_snowball::NapSnowball;
use crate::packet::{Packet, PacketType};
use crate::server::{BigRingState, OutboxMsg, Server};
use crate::maps::{map_time_ex, map_ring};
use crate::states::game::{game_spawn, with_entity_op};

pub fn nap_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();
    map_time_ex(server, 155, 20);
    map_ring(server, 5);
    server.game.bring_state = BigRingState::None;

    for iid in 0..10u8 {
        game_spawn(server, outbox, NapIce::new(iid));
    }

    if cfg.states.gameplay.entities_misc.map_specific.nasty_paradise.snowballs.enabled {
        let mut sb0 = NapSnowball::new(0, 10, 1);
        for i in 0..4usize {
            sb0.p_move[5 + i] = 0.05 + 0.05 * (i as f32 / 4.0);
            sb0.p_anim[5 + i] = 0.35 + 0.25 * (i as f32 / 4.0);
        }
        game_spawn(server, outbox, sb0);

        let mut sb1 = NapSnowball::new(1, 8, -1);
        for i in 0..5usize {
            sb1.p_move[2 + i] = 0.05 + 0.05 * (i as f32 / 5.0);
            sb1.p_anim[2 + i] = 0.35 + 0.25 * (i as f32 / 5.0);
        }
        game_spawn(server, outbox, sb1);

        let mut sb2 = NapSnowball::new(2, 11, 1);
        for i in 0..5usize {
            sb2.p_move[5 + i] = 0.05 + 0.05 * (i as f32 / 5.0);
            sb2.p_anim[5 + i] = 0.35 + 0.25 * (i as f32 / 5.0);
        }
        game_spawn(server, outbox, sb2);

        let mut sb3 = NapSnowball::new(3, 9, 1);
        for i in 0..2usize {
            sb3.p_move[6 + i] = 0.05 + 0.05 * (i as f32 / 2.0);
            sb3.p_anim[6 + i] = 0.35 + 0.25 * (i as f32 / 2.0);
        }
        game_spawn(server, outbox, sb3);

        let sb4 = NapSnowball::new(4, 5, -1);
        game_spawn(server, outbox, sb4);
    }
}

pub fn nap_tcpmsg(peer_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let ptype = match packet.packet_type() { Some(t) => t, None => return };
    match ptype {
        PacketType::CLIENT_NAPICE_ACTIVATE => {
            let peer = match server.find_peer(peer_id) { Some(p) => p, None => return };
            if !peer.in_game { return; }
            packet.pos = 2;
            let iid = match packet.read_u8() { Some(v) => v, None => return };
            if iid >= 10 { return; }
            with_entity_op(server, outbox, |entities, ctx| {
                if let Some(idx) = entities.iter().position(|e| e.nap_iid() == iid as i16) {
                    entities[idx].nap_activate(ctx);
                }
            });
        }
        _ => {}
    }
}
