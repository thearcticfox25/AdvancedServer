use crate::entities::vv_lava::VvLava;
use crate::entities::vv_vase::VvVase;
use crate::packet::{Packet, PacketType};
use crate::server::{BigRingState, OutboxMsg, Server};
use crate::maps::{map_time_ex, map_ring};
use crate::states::game::{game_spawn, game_despawn};

pub fn vv_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    map_time_ex(server, 180, 20);
    map_ring(server, 5);
    server.game.bring_state = BigRingState::None;

    for vase_id in 0..14u8 {
        game_spawn(server, outbox, VvVase::new(vase_id));
    }

    game_spawn(server, outbox, VvLava::new(0, 736.0,  130.0));
    game_spawn(server, outbox, VvLava::new(1, 1388.0, 130.0));
    game_spawn(server, outbox, VvLava::new(2, 1524.0, 130.0));
    game_spawn(server, outbox, VvLava::new(3, 1084.0, 130.0));
}

pub fn vv_tcpmsg(peer_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let packet_type = match packet.packet_type() { Some(found) => found, None => return };
    match packet_type {
        PacketType::CLIENT_VVVASE_BREAK => {
            let peer = match server.find_peer(peer_id) { Some(peer) => peer, None => return };
            if !peer.in_game { return; }
            packet.pos = 2;
            let vase_id = match packet.read_u8() { Some(value) => value, None => return };

            let found = server.game.entities.iter()
                .find(|entity| entity.vv_vid() == vase_id as i16)
                .map(|entity| (entity.id(), entity.vv_vtype()));

            if let Some((ent_id, vtype)) = found {
                if let Some(peer) = server.find_peer_mut(peer_id) {
                    let rings_gained = (vtype + 1) as i16;
                    peer.plr.rings += rings_gained;
                    peer.plr.stats.rings += rings_gained as u16;
                    peer.plr.last_rings = Some(std::time::Instant::now());
                }
                let has_rings = server.find_peer(peer_id).map(|peer| peer.plr.rings > 0).unwrap_or(false);

                let mut pkt = Packet::new(PacketType::SERVER_VVVASE_STATE);
                let _ = pkt.write_u8(vase_id);
                let _ = pkt.write_u8(vtype);
                let _ = pkt.write_u16(peer_id);
                let _ = pkt.write_u8(has_rings as u8);
                outbox.push(crate::server::OutboxMsg::Broadcast(pkt.data().to_vec(), true));

                game_despawn(server, outbox, ent_id);
            }
        }
        _ => {}
    }
}
