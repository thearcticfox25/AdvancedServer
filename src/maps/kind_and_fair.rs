use crate::entities::kaf_speedbox::KafSpeedbox;
use crate::packet::{Packet, PacketType};
use crate::server::{BigRingState, OutboxMsg, Server};
use crate::maps::{map_time_ex, map_ring};
use crate::states::game::{game_spawn, with_entity_op};

pub fn kaf_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    map_time_ex(server, 180, 20);
    map_ring(server, 5);
    server.game.bring_state = BigRingState::None;
    for nid in 0..11u8 {
        game_spawn(server, outbox, KafSpeedbox::new(nid));
    }
}

pub fn kaf_tcpmsg(peer_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let ptype = match packet.packet_type() { Some(t) => t, None => return };
    match ptype {
        PacketType::CLIENT_KAFMONITOR_ACTIVATE => {
            let peer = match server.find_peer(peer_id) { Some(p) => p, None => return };
            if !peer.in_game { return; }
            packet.pos = 2;
            let nid  = match packet.read_u8() { Some(v) => v, None => return };
            let proj = match packet.read_u8() { Some(v) => v, None => return };
            if nid >= 11 { return; }
            let is_proj = proj != 0;
            with_entity_op(server, outbox, |entities, ctx| {
                if let Some(idx) = entities.iter().position(|e| e.kaf_nid() == nid as i16) {
                    entities[idx].kaf_activate(ctx, peer_id, is_proj);
                }
            });
        }
        _ => {}
    }
}
