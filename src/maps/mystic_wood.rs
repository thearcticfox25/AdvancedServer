use crate::entities::wd_latern::WdLatern;
use crate::server::{OutboxMsg, Server};
use crate::states::game::game_spawn;

pub fn wd_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    crate::maps::map_time_ex(server, 155, 20);
    crate::maps::map_ring(server, 5);
    server.game.bring_state = crate::server::BigRingState::None;
    game_spawn(server, outbox, WdLatern::new());
}
