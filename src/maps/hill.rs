use crate::entities::hill_thunder::HillThunder;
use crate::server::{OutboxMsg, Server};
use crate::maps::map_init;
use crate::states::game::game_spawn;

pub fn hill_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    map_init(server, outbox);
    let thunder = HillThunder::new();
    game_spawn(server, outbox, thunder);
}
