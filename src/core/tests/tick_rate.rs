//! The server owner sets how many times a second the round is simulated, and the game
//! runs at the same speed whatever they set: a tick is worth proportionally less
//! movement, so the same seconds cover the same ground.

use super::common::{greenhill_with, hold};
use crate::core::config::{set_tick_rate_of_this_thread, ticks, ORIGINAL_TICKS_PER_SECOND};
use crate::core::player::{Buttons, Character};

/// Where Tails is at every quarter second of a run held on `buttons`, with the round
/// simulated `rate` times a second.
fn run_at(rate: f64, seconds: f64, buttons: u16) -> Vec<(f64, f64)> {
    set_tick_rate_of_this_thread(rate);
    let mut game = greenhill_with(&[Character::Tails]);
    let mut path = Vec::new();
    for _ in 0..(seconds / SAMPLE_SECONDS) as usize {
        hold(&mut game, buttons, ticks(SAMPLE_SECONDS) as usize);
        path.push((game.players[0].x, game.players[0].y));
    }
    path
}

const SAMPLE_SECONDS: f64 = 0.25;

#[test]
fn a_higher_tick_rate_takes_the_same_path_in_the_same_seconds() {
    let (seconds, buttons) = (3.0, Buttons::RIGHT | Buttons::A);
    let original = run_at(ORIGINAL_TICKS_PER_SECOND, seconds, buttons);
    let faster = run_at(72.0, seconds, buttons);
    let again = run_at(ORIGINAL_TICKS_PER_SECOND, seconds, buttons);
    set_tick_rate_of_this_thread(ORIGINAL_TICKS_PER_SECOND);

    assert_eq!(original, again, "the same rate must give the very same run");
    let start = original[0].0;
    assert!(original.last().unwrap().0 > start + 300.0, "the run has to cover ground to mean anything: {original:?}");
    // Not the very same numbers: the sensors work in whole pixels, so the smaller step
    // rounds differently. The run above stays inside a pixel and a half of the 60 Hz one,
    // which no player can tell apart, and it must not drift further as the run goes on.
    for (at, (slow, fast)) in original.iter().zip(&faster).enumerate() {
        let (dx, dy) = ((fast.0 - slow.0).abs(), (fast.1 - slow.1).abs());
        let second = at as f64 * SAMPLE_SECONDS + SAMPLE_SECONDS;
        assert!(dx < 4.0 && dy < 4.0, "after {second} s: 72 Hz at {fast:?}, 60 Hz at {slow:?}");
    }
}

#[test]
fn counts_of_the_originals_steps_last_as_long_at_any_rate() {
    use crate::core::config::{eased_share, original_ticks};
    set_tick_rate_of_this_thread(72.0);
    // One second of 60 Hz steps is one second of 72 Hz steps.
    assert_eq!(original_ticks(60), 72);
    assert_eq!(original_ticks(6), 7);
    // Half the way a 60 Hz step: after a second the same part of the way is left.
    let left_after_a_second = (1.0 - eased_share(0.5)).powi(72);
    assert!((left_after_a_second - 0.5f64.powi(60)).abs() < 1e-12);
    set_tick_rate_of_this_thread(ORIGINAL_TICKS_PER_SECOND);
    assert_eq!((original_ticks(60), eased_share(0.5)), (60, 0.5), "at 60 Hz nothing changes");
}
