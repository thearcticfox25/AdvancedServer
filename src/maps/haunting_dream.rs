use crate::entities::hd_door::HdDoor;
use crate::packet::{Packet, PacketType};
use crate::server::{BigRingState, OutboxMsg, Server};
use crate::maps::{map_time_ex, map_ring};
use crate::states::game::{game_spawn, with_entity_op};

pub fn hd_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    map_time_ex(server, 205, 20);
    map_ring(server, 5);
    server.game.bring_state = BigRingState::None;
    game_spawn(server, outbox, HdDoor::new());
}

pub fn hd_tcpmsg(peer_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let ptype = match packet.packet_type() { Some(t) => t, None => return };
    match ptype {
        PacketType::CLIENT_HDDOOR_TOGGLE => {
            let peer = match server.find_peer(peer_id) { Some(p) => p, None => return };
            if !peer.in_game { return; }
            with_entity_op(server, outbox, |entities, ctx| {
                if let Some(door) = entities.iter_mut().find(|e| e.tag() == "hddoor") {
                    door.hd_toggle(ctx);
                }
            });
        }
        _ => {}
    }
}
