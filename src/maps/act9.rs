use crate::entities::act9_wall::Act9Wall;
use crate::server::{BigRingState, OutboxMsg, Server};
use crate::maps::{map_time_ex, map_ring};
use crate::states::game::game_spawn;

pub fn act9_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    map_time_ex(server, 130, 10);
    map_ring(server, 3);
    server.game.bring_state = BigRingState::None;

    let cfg = crate::config::cfg();
    if cfg.states.gameplay.entities_misc.map_specific.act9.walls.enabled {
        game_spawn(server, outbox, Act9Wall::new(0, 0.0, 1025.0));
        game_spawn(server, outbox, Act9Wall::new(1, 1663.0, 0.0));
        game_spawn(server, outbox, Act9Wall::new(2, 1663.0, 0.0));
    }
}
