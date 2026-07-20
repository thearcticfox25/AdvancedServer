use crate::server::{OutboxMsg, Server};
use crate::maps::map_init;

pub fn hs2_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    map_init(server, outbox);
}
