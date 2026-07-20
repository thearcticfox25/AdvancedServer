use crate::entities::mj_lava::MjLava;
use crate::entities::mj_judger::MjJudger;
use crate::entities::mj_ass::MjAss;
use crate::server::{BigRingState, OutboxMsg, Server};
use crate::maps::{map_time_ex, map_ring};
use crate::states::game::game_spawn;

pub fn mj_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    map_time_ex(server, 205, 20);
    map_ring(server, 5);
    server.game.bring_state = BigRingState::None;
    game_spawn(server, outbox, MjLava::new(1624.0, 512.0));
    game_spawn(server, outbox, MjAss::new());
    game_spawn(server, outbox, MjJudger::new());
}
