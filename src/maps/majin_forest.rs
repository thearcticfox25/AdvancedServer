use crate::server::{BigRingState, OutboxMsg, Server};
use crate::maps::{map_time_ex, map_ring};

pub fn maj_init(server: &mut Server, _outbox: &mut Vec<OutboxMsg>) {
    map_time_ex(server, 155, 10);
    map_ring(server, 3);
    server.game.bring_state = BigRingState::None;
}
