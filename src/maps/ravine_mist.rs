use crate::config::cfg;
use crate::server::{BigRingState, OutboxMsg, Server};
use crate::states::game::game_bigring;

pub fn rmz_tick(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();
    if cfg.states.gameplay.banana.disable_timer { return; }
    if server.game.time == 0.0 { return; }

    let ring_time = cfg.states.gameplay.ring_appearance_timer as u16;
    let escape_time = cfg.states.gameplay.escape_time as u16;

    if server.game.time_sec <= ring_time && server.game.bring_state < BigRingState::Deactivated {
        game_bigring(server, BigRingState::Deactivated, outbox);
    }

    if server.game.time_sec <= escape_time && server.game.bring_state < BigRingState::Activated {
        let shard_cfg = &cfg.states.gameplay.entities_misc.map_specific.ravine_mist.shards;
        if server.game.shards_found >= shard_cfg.required_for_exit {
            game_bigring(server, BigRingState::Activated, outbox);
        }
    }
}
