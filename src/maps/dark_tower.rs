use crate::entities::dt_ball::DtBall;
use crate::entities::dt_stalactits::DtStalactits;
use crate::entities::dt_tails_doll::DtTailsDoll;
use crate::packet::{Packet, PacketType};
use crate::server::{BigRingState, OutboxMsg, Server};
use crate::maps::{map_time_ex, map_ring};
use crate::states::game::{game_spawn, with_entity_op};

const STALACTITE_POSITIONS: [(u8, u16, u16); 14] = [
    (0,  1744, 224),
    (1,  1840, 224),
    (2,  1936, 224),
    (3,  2032, 224),
    (4,  2128, 224),
    (5,  1824, 784),
    (6,  1920, 784),
    (7,  2016, 784),
    (8,  2112, 784),
    (9,  2208, 784),
    (10, 2464, 1384),
    (11, 2592, 1384),
    (12, 3032, 64),
    (13, 3088, 64),
];

pub fn dt_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    map_time_ex(server, 205, 20);
    map_ring(server, 5);
    server.game.bring_state = BigRingState::None;

    game_spawn(server, outbox, DtBall::new());
    game_spawn(server, outbox, DtTailsDoll::new());

    for &(sid, x, y) in &STALACTITE_POSITIONS {
        game_spawn(server, outbox, DtStalactits::new(sid, x, y));
    }
}

pub fn dt_tcpmsg(peer_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let ptype = match packet.packet_type() { Some(t) => t, None => return };
    match ptype {
        PacketType::CLIENT_DTASS_ACTIVATE => {
            let peer = match server.find_peer(peer_id) { Some(p) => p, None => return };
            if !peer.in_game { return; }
            packet.pos = 2;
            let sid = match packet.read_u8() { Some(v) => v, None => return };
            if sid >= 14 { return; }
            with_entity_op(server, outbox, |entities, ctx| {
                if let Some(idx) = entities.iter().position(|e| e.dt_sid() == sid as i16) {
                    entities[idx].dt_activate(ctx);
                }
            });
        }
        _ => {}
    }
}
