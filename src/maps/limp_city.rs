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
    let packet_type = match packet.packet_type() { Some(found) => found, None => return };
    match packet_type {
        PacketType::CLIENT_LCEYE_REQUEST_ACTIVATE => {
            packet.pos = 2;
            let activating = match packet.read_u8() { Some(value) => value, None => return };
            let eye_id     = match packet.read_u8() { Some(value) => value, None => return };
            let target     = match packet.read_u8() { Some(value) => value, None => return };
            if eye_id >= 2 { return; }
            let peer = match server.find_peer(peer_id) { Some(peer) => peer, None => return };
            if !peer.in_game { return; }

            with_entity_op(server, outbox, |entities, ctx| {
                if let Some(idx) = entities.iter().position(|entity| entity.lceye_nid() == eye_id as i16) {
                    let (already_used, charge) = entities[idx].lceye_get();
                    if activating != 0 {
                        if already_used { return; }
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
