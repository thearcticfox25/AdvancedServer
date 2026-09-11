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

    for ice_id in 0..10u8 {
        game_spawn(server, outbox, NapIce::new(ice_id));
    }

    if cfg.states.gameplay.entities_misc.map_specific.nasty_paradise.snowballs.enabled {
        let mut snowball_0 = NapSnowball::new(0, 10, 1);
        for step in 0..4usize {
            snowball_0.p_move[5 + step] = 0.05 + 0.05 * (step as f32 / 4.0);
            snowball_0.p_anim[5 + step] = 0.35 + 0.25 * (step as f32 / 4.0);
        }
        game_spawn(server, outbox, snowball_0);

        let mut snowball_1 = NapSnowball::new(1, 8, -1);
        for step in 0..5usize {
            snowball_1.p_move[2 + step] = 0.05 + 0.05 * (step as f32 / 5.0);
            snowball_1.p_anim[2 + step] = 0.35 + 0.25 * (step as f32 / 5.0);
        }
        game_spawn(server, outbox, snowball_1);

        let mut snowball_2 = NapSnowball::new(2, 11, 1);
        for step in 0..5usize {
            snowball_2.p_move[5 + step] = 0.05 + 0.05 * (step as f32 / 5.0);
            snowball_2.p_anim[5 + step] = 0.35 + 0.25 * (step as f32 / 5.0);
        }
        game_spawn(server, outbox, snowball_2);

        let mut snowball_3 = NapSnowball::new(3, 9, 1);
        for step in 0..2usize {
            snowball_3.p_move[6 + step] = 0.05 + 0.05 * (step as f32 / 2.0);
            snowball_3.p_anim[6 + step] = 0.35 + 0.25 * (step as f32 / 2.0);
        }
        game_spawn(server, outbox, snowball_3);

        let snowball_4 = NapSnowball::new(4, 5, -1);
        game_spawn(server, outbox, snowball_4);
    }
}

pub fn nap_tcpmsg(peer_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let packet_type = match packet.packet_type() { Some(found) => found, None => return };
    match packet_type {
        PacketType::CLIENT_NAPICE_ACTIVATE => {
            let peer = match server.find_peer(peer_id) { Some(peer) => peer, None => return };
            if !peer.in_game { return; }
            packet.pos = 2;
            let ice_id = match packet.read_u8() { Some(value) => value, None => return };
            if ice_id >= 10 { return; }
            with_entity_op(server, outbox, |entities, ctx| {
                if let Some(idx) = entities.iter().position(|entity| entity.nap_iid() == ice_id as i16) {
                    entities[idx].nap_activate(ctx);
                }
            });
        }
        _ => {}
    }
}
