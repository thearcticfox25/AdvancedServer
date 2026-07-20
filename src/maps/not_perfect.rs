use crate::entities::not_perfect::NotPerfect;
use crate::server::{OutboxMsg, Server};
use crate::maps::map_init;
use crate::states::game::game_spawn;

pub fn np_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    map_init(server, outbox);
    let ctrl = NotPerfect::new();
    game_spawn(server, outbox, ctrl);
}
