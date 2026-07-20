use crate::entities::lc_eye::LcEye;
use crate::entities::lc_chain::LcChain;
use crate::packet::{Packet, PacketType};
use crate::server::{BigRingState, OutboxMsg, Server};
use crate::maps::{map_time_ex, map_ring};
use crate::states::game::{game_spawn, with_entity_op};

pub fn lc_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    map_time_ex(server, 155, 20);
    map_ring(server, 5);
    server.game.bring_state = BigRingState::None;
    game_spawn(server, outbox, LcEye::new(0));
    game_spawn(server, outbox, LcEye::new(1));
    game_spawn(server, outbox, LcChain::new());
}

pub fn lc_tcpmsg(peer_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let ptype = match packet.packet_type() { Some(t) => t, None => return };
    match ptype {
        PacketType::CLIENT_LCEYE_REQUEST_ACTIVATE => {
            packet.pos = 2;
            let val    = match packet.read_u8() { Some(v) => v, None => return };
            let nid    = match packet.read_u8() { Some(v) => v, None => return };
            let target = match packet.read_u8() { Some(v) => v, None => return };
            if nid >= 2 { return; }
            let peer = match server.find_peer(peer_id) { Some(p) => p, None => return };
            if !peer.in_game { return; }

            with_entity_op(server, outbox, |entities, ctx| {
                if let Some(idx) = entities.iter().position(|e| e.lceye_nid() == nid as i16) {
                    let (used, charge) = entities[idx].lceye_get();
                    if val != 0 {
                        if used { return; }
                        if charge < 20 { return; }
                        entities[idx].lceye_set_used(true, peer_id, target, ctx);
                    } else {
                        entities[idx].lceye_set_used(false, 0, 0, ctx);
                    }
                }
            });
        }
        _ => {}
    }
}
