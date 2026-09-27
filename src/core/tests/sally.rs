//! Behavior checks of Sally's abilities in a real room.

use super::common::{greenhill_with, hold};
use crate::core::player::{Buttons, Character};

#[test]
fn shield_absorbs_one_hit() {
    let mut game = greenhill_with(&[Character::Sally]);
    hold(&mut game, 0, 120);
    hold(&mut game, Buttons::C, 1);
    assert!(game.players[0].shield_timer > 0, "C raises the shield");

    let cfg = game.world.config.clone();
    let world = &game.world;
    let mut events = Vec::new();
    let hit = crate::core::player::hurt::Hit::damage(&cfg, cfg.contact.damage);
    game.players[0].hurt(world, &cfg, hit, &mut events);
    let sally = &game.players[0];
    assert_eq!(sally.hp, cfg.hurt.max_hp, "the shield takes the hit");
    assert_eq!(sally.shield_timer, 0, "and breaks");
    assert_eq!(sally.hurttime, 60, "one second of invincibility");
}

#[test]
fn down_while_running_slides() {
    let mut game = greenhill_with(&[Character::Sally]);
    hold(&mut game, 0, 120);
    hold(&mut game, Buttons::RIGHT, 60);
    let speed_before = game.players[0].xspd;
    assert!(speed_before >= 2.5, "running fast enough to slide: {speed_before}");
    hold(&mut game, Buttons::RIGHT | Buttons::DOWN, 1);
    assert!(game.players[0].is_sliding, "{:?}", game.players[0]);
}
