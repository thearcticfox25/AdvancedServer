//! Behavior checks of Amy's abilities in a real room.

use super::common::{greenhill_with, hold};
use crate::core::events::SimEvent;
use crate::core::player::{Buttons, Character};

#[test]
fn down_and_jump_does_the_hammer_jump() {
    let mut game = greenhill_with(&[Character::Amy]);
    hold(&mut game, 0, 120);
    assert!(game.players[0].is_grounded);
    hold(&mut game, Buttons::DOWN, 2);
    hold(&mut game, Buttons::DOWN | Buttons::A, 1);
    let amy = &game.players[0];
    assert!(amy.is_hj, "{amy:?}");
    assert_eq!(amy.yspd, -10.0 + 0.21875, "hammer jump force 10, gravity in the same tick");
    assert_eq!(amy.hjump_timer, 9 * 60, "recharge starts but only counts down on the ground");
}

#[test]
fn hammer_attack_lasts_its_animation() {
    let mut game = greenhill_with(&[Character::Amy]);
    hold(&mut game, 0, 120);
    let events = hold(&mut game, Buttons::B, 1);
    assert!(game.players[0].is_attacking);
    let hearts = events.iter().filter(|event| matches!(event, SimEvent::Effect { .. })).count();
    assert_eq!(hearts, 4, "four hearts fly out of the hammer");
    let mut ticks = 0;
    while game.players[0].is_attacking && ticks < 600 {
        hold(&mut game, 0, 1);
        ticks += 1;
    }
    assert!(!game.players[0].is_attacking, "the attack ends with its animation");
    assert!(ticks > 10, "and not instantly: {ticks} ticks");
}
