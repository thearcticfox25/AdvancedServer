//! Behavior checks of Eggman's abilities in a real room.

use super::common::{flat_floor_spot, greenhill_with, hold_each};
use crate::core::events::SimEvent;
use crate::core::player::{Buttons, Character};

#[test]
fn second_jump_press_fires_the_jetpack() {
    let mut game = greenhill_with(&[Character::Eggman]);
    hold_each(&mut game, &[0], 120);
    assert!(game.players[0].is_grounded);
    hold_each(&mut game, &[Buttons::A], 1);
    hold_each(&mut game, &[0], 3);
    hold_each(&mut game, &[Buttons::A], 1);
    let eggman = &game.players[0];
    assert_eq!(eggman.yspd, -8.0 + 0.21875, "double jump sets -8, gravity applies in the same tick");
    assert!(eggman.djump_recharge < 0, "jetpack pose running");
}

#[test]
fn shield_lasts_one_and_a_half_seconds() {
    let mut game = greenhill_with(&[Character::Eggman]);
    hold_each(&mut game, &[0], 120);
    hold_each(&mut game, &[Buttons::B], 1);
    assert!(game.players[0].is_attacking, "B raises the shield");
    // The timer is set to -1 and counted once more in the same tick, so the
    // shield holds 89 ticks including the press, as in the original.
    hold_each(&mut game, &[0], 87);
    assert!(game.players[0].is_attacking, "still up on tick 88");
    hold_each(&mut game, &[0], 1);
    assert!(!game.players[0].is_attacking, "down on tick 89");
    assert!(game.players[0].shield_recharge > 0, "recharge started");
}

#[test]
fn tracker_slows_exe_for_three_seconds() {
    let mut game = greenhill_with(&[Character::Eggman, Character::Exe]);
    let (x, y) = flat_floor_spot(&game.world);
    for player in &mut game.players {
        player.x = x;
        player.y = y;
    }
    hold_each(&mut game, &[0, 0], 120);
    let events = hold_each(&mut game, &[Buttons::DOWN | Buttons::C, 0], 1);
    assert!(
        events.iter().any(|event| matches!(event, SimEvent::SpawnEggTracker { .. })),
        "down + C on flat floor places a tracker: {:?}",
        game.players[0]
    );
    // Trackers run after the players in the same tick, so an EXE standing on the
    // spot is caught in the placement tick already.
    let mut events = events;
    events.extend(hold_each(&mut game, &[0, 0], 2));
    assert!(events.iter().any(|event| matches!(event, SimEvent::TrackerCaught { eggman: 0, victim: 1 })), "{events:?}");
    let exe = &game.players[1];
    assert!(exe.is_slow);
    assert!((exe.acc - 0.056875 * 0.7).abs() < 1e-12, "EXE is slowed by 30 %: {}", exe.acc);

    hold_each(&mut game, &[0, 0], 180);
    assert!(!game.players[1].is_slow, "slowdown ends after 3 seconds");
}


