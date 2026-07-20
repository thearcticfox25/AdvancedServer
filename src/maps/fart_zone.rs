use crate::entities::black_ring::BlackRing;
use crate::entities::dummy::Dummy;
use crate::entities::spike_controller::SpikeController;
use crate::entities::dt_ball::DtBall;
use crate::packet::{Packet, PacketType};
use crate::server::{BigRingState, OutboxMsg, Server};
use crate::maps::{map_time_ex, map_ring};
use crate::states::game::{game_spawn, with_entity_op};

pub const MAP_BRING: f32 = 32767.0;

pub fn ft_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    map_time_ex(server, 256, 20);
    map_ring(server, 1);
    server.game.bring_state = BigRingState::None;

    for _ in 0..3 {
        game_spawn(server, outbox, BlackRing::new(MAP_BRING, MAP_BRING));
    }
    game_spawn(server, outbox, Dummy::new());
    game_spawn(server, outbox, SpikeController::new());
    game_spawn(server, outbox, DtBall::new());
}

pub fn ft_tcpmsg(peer_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let ptype = match packet.packet_type() { Some(t) => t, None => return };
    match ptype {
        PacketType::CLIENT_FART_PUSH => {
            let peer = match server.find_peer(peer_id) { Some(p) => p, None => return };
            if !peer.in_game { return; }
            packet.pos = 2;
            let spd = match packet.read_i8() { Some(v) => v, None => return };
            with_entity_op(server, outbox, |entities, _ctx| {
                if let Some(dum) = entities.iter_mut().find(|e| e.tag() == "dummy") {
                    dum.dummy_push(spd);
                }
            });
        }
        _ => {}
    }
}
