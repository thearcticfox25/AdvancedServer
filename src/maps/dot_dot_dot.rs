use crate::entities::spike_controller::SpikeController;
use crate::server::{BigRingState, OutboxMsg, Server};
use crate::maps::{map_time_ex, map_ring};
use crate::states::game::game_spawn;

pub fn dot_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    map_time_ex(server, 205, 20);
    map_ring(server, 5);
    server.game.bring_state = BigRingState::None;
    game_spawn(server, outbox, SpikeController::new());
}
