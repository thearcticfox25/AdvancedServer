//! Fart Zone's training dummy and the statues' curse.

use super::common::{hold_each, started_room};
use crate::core::config::{ticks, GameplayConfig};
use crate::core::events::SimEvent;
use crate::core::game::Game;
use crate::core::level::Level;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Character, Player};
use crate::core::rooms::ids::RoomId;

fn fart_zone() -> Game {
    let mut game = Game::new(started_room(RoomId::Fartzone));
    game.level = Some(Level::new(&game.world, 2));
    game
}

#[test]
fn an_attack_pushes_the_dummy_which_slides_to_a_stop() {
    let mut game = fart_zone();
    hold_each(&mut game, &[], 1);
    let dummy = game.world.ids_of(ObjectId::FartDummy).next().unwrap();
    let (x, y) = (game.world.instances[dummy].x, game.world.instances[dummy].y);
    let mut knux = Player::new(Character::Knux, x - 10.0, y - 20.0, &GameplayConfig::default());
    knux.is_attacking = true;
    knux.image_xscale = 1.0;
    game.players.push(knux);
    let events = hold_each(&mut game, &[0], 1);
    assert!(events.iter().any(|event| matches!(event, SimEvent::DummyHit { worth: 3, everyone: false, .. })), "{events:?}");
    game.players.clear();
    hold_each(&mut game, &[], 60);
    let moved = game.world.instances[dummy].x - x;
    assert!(moved > 10.0, "slid right: {moved}");
    let stopped_at = game.world.instances[dummy].x;
    hold_each(&mut game, &[], 10);
    assert_eq!(game.world.instances[dummy].x, stopped_at, "and stopped");
}

#[test]
fn the_statues_curse_passes_by_touch_and_ends_the_game_of_whoever_keeps_it() {
    let mut game = fart_zone();
    let rules = game.world.config.levels.fart_zone.clone();
    let statue = game.world.ids_of(ObjectId::FartMermer).next().unwrap();
    let (x, y) = (game.world.instances[statue].x, game.world.instances[statue].y);
    game.players.push(Player::new(Character::Tails, x, y, &GameplayConfig::default()));
    game.players.push(Player::new(Character::Amy, 100.0, 100.0, &GameplayConfig::default()));
    hold_each(&mut game, &[0, 0], 1);
    assert!(game.players[0].potato_ticks > 0, "cursed by the statue");
    // Amy comes close once it can pass.
    hold_each(&mut game, &[0, 0], ticks(rules.potato_passes_after_seconds) as usize);
    game.players[1].x = game.players[0].x;
    game.players[1].y = game.players[0].y;
    hold_each(&mut game, &[0, 0], 1);
    assert_eq!(game.players[0].potato_ticks, 0, "passed on");
    assert!(game.players[1].potato_ticks > 0);
    game.players[0].x = 100.0;
    let events = hold_each(&mut game, &[0, 0], ticks(rules.potato_seconds) as usize + 1);
    assert!(events.contains(&SimEvent::PotatoBoom), "Amy kept it too long");
}
