use crate::entities::you_cant_run::YouCantRun;
use crate::entities::spike_controller::SpikeController;
use crate::server::{BigRingState, OutboxMsg, Server};
use crate::maps::{map_time_ex, map_ring};
use crate::states::game::game_spawn;

pub fn ycr_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    map_time_ex(server, 180, 20);
    map_ring(server, 5);
    server.game.bring_state = BigRingState::None;
    game_spawn(server, outbox, SpikeController::new());
    game_spawn(server, outbox, YouCantRun::new());
}
