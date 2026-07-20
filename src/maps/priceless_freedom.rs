use crate::config::cfg;
use crate::entities::pf_lift::PfLift;
use crate::entities::black_ring::BlackRing;
use crate::packet::{Packet, PacketType};
use crate::server::{BigRingState, OutboxMsg, Server};
use crate::maps::{map_time_ex, map_ring};
use crate::states::game::{game_spawn, with_entity_op};

pub const MAP_BRING: f32 = 32767.0;

pub fn pf_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();
    map_time_ex(server, 155, 10);
    map_ring(server, 5);
    server.game.bring_state = BigRingState::None;

    if cfg.states.gameplay.entities_misc.map_specific.priceless_freedom.black_rings.enabled {
        for _ in 0..29 {
            game_spawn(server, outbox, BlackRing::new(MAP_BRING, MAP_BRING));
        }
    }

    game_spawn(server, outbox, PfLift::new(0, 1669.0, 1016.0));
    game_spawn(server, outbox, PfLift::new(1, 1069.0, 704.0));
    game_spawn(server, outbox, PfLift::new(2, 829.0,  400.0));
    game_spawn(server, outbox, PfLift::new(3, 1070.0, 544.0));
}

pub fn pf_tcpmsg(peer_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let ptype = match packet.packet_type() { Some(t) => t, None => return };
    match ptype {
        PacketType::CLIENT_PFLIT_ACTIVATE => {
            let peer = match server.find_peer(peer_id) { Some(p) => p, None => return };
            if !peer.in_game { return; }
            packet.pos = 2;
            let lid = match packet.read_u8() { Some(v) => v, None => return };
            with_entity_op(server, outbox, |entities, ctx| {
                if let Some(idx) = entities.iter().position(|e| e.pf_lid() == lid as i16) {
                    entities[idx].pf_activate(ctx, peer_id);
                }
            });
        }
        _ => {}
    }
}
