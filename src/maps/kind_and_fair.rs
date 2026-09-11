use crate::entities::kaf_speedbox::KafSpeedbox;
use crate::packet::{Packet, PacketType};
use crate::server::{BigRingState, OutboxMsg, Server};
use crate::maps::{map_time_ex, map_ring};
use crate::states::game::{game_spawn, with_entity_op};

pub fn kaf_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    map_time_ex(server, 180, 20);
    map_ring(server, 5);
    server.game.bring_state = BigRingState::None;
    for box_id in 0..11u8 {
        game_spawn(server, outbox, KafSpeedbox::new(box_id));
    }
}

pub fn kaf_tcpmsg(peer_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let packet_type = match packet.packet_type() { Some(found) => found, None => return };
    match packet_type {
        PacketType::CLIENT_KAFMONITOR_ACTIVATE => {
            let peer = match server.find_peer(peer_id) { Some(peer) => peer, None => return };
            if !peer.in_game { return; }
            packet.pos = 2;
            let box_id  = match packet.read_u8() { Some(value) => value, None => return };
            let by_projectile = match packet.read_u8() { Some(value) => value, None => return };
            if box_id >= 11 { return; }
            let is_proj = by_projectile != 0;
            with_entity_op(server, outbox, |entities, ctx| {
                if let Some(idx) = entities.iter().position(|entity| entity.kaf_nid() == box_id as i16) {
                    entities[idx].kaf_activate(ctx, peer_id, is_proj);
                }
            });
        }
        _ => {}
    }
}
