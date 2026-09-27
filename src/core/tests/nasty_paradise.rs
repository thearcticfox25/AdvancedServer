//! Nasty Paradise's ice blocks and snowballs.

use super::common::{hold_each, started_room};
use crate::core::config::{ticks, GameplayConfig};
use crate::core::game::Game;
use crate::core::level::Level;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Character, Player};
use crate::core::rooms::ids::RoomId;
use crate::core::world::ObjectVars;

fn nasty_paradise() -> Game {
    let mut game = Game::new(started_room(RoomId::Nastyparadise));
    game.level = Some(Level::new(&game.world, 3));
    game
}

#[test]
fn an_ice_block_landed_on_throws_the_player_up_breaks_and_grows_back() {
    let mut game = nasty_paradise();
    let block = game.world.ids_of(ObjectId::NapIceblock).find(|&id| matches!(game.world.instances[id].vars, ObjectVars::IceBlock { nid: 3 })).unwrap();
    let top = game.world.bbox(block).unwrap().top;
    let x = game.world.instances[block].x + 20.0;
    game.players.push(Player::new(Character::Tails, x, top - 40.0, &GameplayConfig::default()));

    let mut thrown = false;
    for _ in 0..60 {
        hold_each(&mut game, &[0], 1);
        thrown |= game.players[0].yspd < 0.0;
    }
    assert!(thrown, "landing on the block throws the player up: {:?}", game.players[0]);
    assert!(!game.world.instances[block].visible, "and breaks it");
    assert!(game.world.bbox(block).is_none_or(|bbox| bbox.bottom <= bbox.top), "a broken block has no mask");

    // Standing where it grows back would break it again.
    game.players.clear();
    let regeneration = ticks(game.world.config.levels.nasty_paradise.ice_regeneration_seconds) as usize;
    hold_each(&mut game, &[], regeneration);
    assert!(game.world.instances[block].visible, "grown back");
}

#[test]
fn snowballs_roll_down_their_paths_then_break() {
    let mut game = nasty_paradise();
    let rules = game.world.config.levels.nasty_paradise.clone();
    let balls: Vec<_> = game.world.ids_of(ObjectId::NapSnowball).collect();
    assert_eq!(balls.len(), 5, "one snowball a path");
    let first = balls.iter().copied().find(|&id| matches!(game.world.instances[id].vars, ObjectVars::Snowball { nid: 0 })).unwrap();
    let start = (game.world.instances[first].x, game.world.instances[first].y);

    hold_each(&mut game, &[], ticks(rules.snowball_every_seconds) as usize + 2);
    assert!(game.world.instances[first].visible, "rolling after its wait");
    assert_eq!(game.world.instances[first].image_xscale, 1.0, "path 0 goes right");
    let mut rolled = 0;
    while game.world.instances[first].visible && rolled < 3000 {
        hold_each(&mut game, &[], 1);
        rolled += 1;
    }
    assert!(rolled < 3000, "breaks at the end of its path");
    let end = (game.world.instances[first].x, game.world.instances[first].y);
    assert!(end.0 > start.0 + 700.0 && end.1 > start.1, "went down its path: {start:?} to {end:?}");
}

#[test]
fn a_rolling_snowball_hurts_and_slows() {
    let mut game = nasty_paradise();
    let rules = game.world.config.levels.nasty_paradise.clone();
    hold_each(&mut game, &[], ticks(rules.snowball_every_seconds) as usize + 120);
    let ball = game.world.ids_of(ObjectId::NapSnowball).find(|&id| matches!(game.world.instances[id].vars, ObjectVars::Snowball { nid: 0 })).unwrap();
    let instance = &game.world.instances[ball];
    assert!(instance.visible && instance.image_index >= 8.0, "rolling by now: {instance:?}");
    game.players.push(Player::new(Character::Knux, instance.x, instance.y - 40.0, &GameplayConfig::default()));
    hold_each(&mut game, &[0], 1);
    let knux = &game.players[0];
    assert!(knux.hp < 100 && knux.is_slow, "hurt and slowed: {knux:?}");
}
