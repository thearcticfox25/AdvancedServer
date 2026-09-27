//! Weed Zone's lanterns and conveyors.

use super::common::{hold_each, started_room};
use crate::core::config::{ticks, GameplayConfig};
use crate::core::game::Game;
use crate::core::level::Level;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Character, Player};
use crate::core::rooms::ids::RoomId;
use crate::core::world::ObjectVars;

fn weed_zone() -> Game {
    let mut game = Game::new(started_room(RoomId::Weedzone));
    game.level = Some(Level::new(&game.world, 2));
    game
}

#[test]
fn one_lantern_is_lit_after_a_dark_while_then_none() {
    let mut game = weed_zone();
    let rules = game.world.config.levels.weed_zone.clone();
    let lit = |game: &Game| game.world.ids_of(ObjectId::WeedLantern).filter(|&id| matches!(game.world.instances[id].vars, ObjectVars::Lantern { lit: true, .. })).count();
    hold_each(&mut game, &[], ticks(rules.lantern_dark_seconds) as usize - 1);
    assert_eq!(lit(&game), 0, "dark at first");
    hold_each(&mut game, &[], ticks(rules.lantern_dark_extra_seconds as f64) as usize + 2);
    assert_eq!(lit(&game), 1, "then one is lit");
    hold_each(&mut game, &[], ticks(rules.lantern_lit_seconds + rules.lantern_lit_extra_seconds as f64) as usize + 2);
    assert_eq!(lit(&game), 0, "and goes out");
}

#[test]
fn a_conveyor_carries_the_player_left() {
    let mut game = weed_zone();
    let conveyor = game.world.ids_of(ObjectId::WeedConveyor).next().unwrap();
    let bbox = game.world.bbox(conveyor).unwrap();
    game.players.push(Player::new(Character::Knux, bbox.right - 8.0, bbox.top - 30.0, &GameplayConfig::default()));
    let mut ticks_waited = 0;
    while !game.players[0].is_grounded && ticks_waited < 60 {
        hold_each(&mut game, &[0], 1);
        ticks_waited += 1;
    }
    assert!(game.players[0].is_grounded, "landed on it: {:?}", game.players[0]);
    let before = game.players[0].x;
    hold_each(&mut game, &[0], 5);
    assert!(before - game.players[0].x >= 15.0, "carried left: {before} to {}", game.players[0].x);
}
