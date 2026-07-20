use crate::entities::tc_acid::TcAcid;
use crate::server::{OutboxMsg, Server};
use crate::maps::map_init;
use crate::states::game::game_spawn;

pub fn tc_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    map_init(server, outbox);
    let acid = TcAcid::new();
    game_spawn(server, outbox, acid);
}
