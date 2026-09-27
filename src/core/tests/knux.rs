//! Behavior checks of Knuckles' abilities in a real room.

use super::common::{greenhill_with, hold};
use std::sync::Arc;
use crate::core::resources::names::sprite;
use crate::core::config::GameplayConfig;
use crate::core::events::SimEvent;
use crate::core::player::{Buttons, Character};

#[test]
fn knuckles_uses_his_own_sprites_and_jump() {
    let mut game = greenhill_with(&[Character::Knux]);
    assert_eq!(game.players[0].sprite_index, sprite::SPR_KNUX_IDLE);
    hold(&mut game, 0, 120);
    assert!(game.players[0].is_grounded);
    hold(&mut game, Buttons::A, 1);
    assert_eq!(game.players[0].yspd, -6.5 + 0.21875, "Knuckles jumps with 6.5, gravity applies in the same tick");
}

#[test]
fn second_jump_press_glides_at_five() {
    let mut game = greenhill_with(&[Character::Knux]);
    hold(&mut game, 0, 120);
    hold(&mut game, Buttons::A, 1);
    hold(&mut game, 0, 5);
    hold(&mut game, Buttons::A, 1);
    let knuckles = &game.players[0];
    assert!(knuckles.is_gliding, "{knuckles:?}");
    assert_eq!(knuckles.xspd.abs(), 5.0, "glide starts at 5 px per tick in the facing direction");
    assert_eq!(knuckles.glide_timer, 15 * 60, "glide recharge starts");
}

#[test]
fn b_on_the_ground_punches_with_a_short_dash() {
    let mut game = greenhill_with(&[Character::Knux]);
    hold(&mut game, 0, 120);
    let events = hold(&mut game, Buttons::B | Buttons::RIGHT, 1);
    let knuckles = &game.players[0];
    assert!(knuckles.is_attacking, "{knuckles:?}");
    assert_eq!(knuckles.attack_timer, 20 * 60 - 1, "recharge starts and already counted this tick");
    assert!(events.iter().any(|event| matches!(event, SimEvent::Sound { .. })));
}

#[test]
fn jump_force_comes_from_the_gameplay_config() {
    let mut game = greenhill_with(&[Character::Knux]);
    let mut config = GameplayConfig::default();
    config.knuckles.movement.jump_force_per_tick = 9.0;
    game.world.config = Arc::new(config);
    hold(&mut game, 0, 120);
    hold(&mut game, Buttons::A, 1);
    assert_eq!(game.players[0].yspd, -9.0 + 0.21875, "an edited config changes the jump without rebuilding");
}

#[test]
fn default_config_files_round_trip_through_disk() {
    let folder = std::env::temp_dir().join(format!("core_config_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    GameplayConfig::default().write_missing_files(&folder).unwrap();
    assert!(folder.join("Knuckles.toml").exists());
    assert_eq!(GameplayConfig::load(&folder).unwrap(), GameplayConfig::default());
    std::fs::remove_dir_all(&folder).unwrap();
}
