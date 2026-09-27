//! Act 9's closing walls.

use super::common::{hold_each, started_room};
use crate::core::config::GameplayConfig;
use crate::core::game::Game;
use crate::core::level::Level;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Character, Player};
use crate::core::rooms::ids::RoomId;
use crate::core::world::ObjectVars;

#[test]
fn the_walls_close_with_the_clock_and_crush_who_they_reach() {
    let mut game = Game::new(started_room(RoomId::Act9));
    game.level = Some(Level::new(&game.world, 1));
    let rules = game.world.config.levels.act9.clone();
    let start = game.world.timer_ticks;
    hold_each(&mut game, &[], 1);
    // Half the time has gone.
    game.world.timer_ticks = start / 2;
    hold_each(&mut game, &[], 1);
    let wall = |game: &Game, wanted: u8| game.world.ids_of(ObjectId::Act9Wall).find(|&id| game.world.instances[id].vars == ObjectVars::Act9Wall { nid: wanted }).unwrap();
    let ceiling = game.world.bbox(wall(&game, 0)).unwrap();
    let left = game.world.bbox(wall(&game, 1)).unwrap();
    assert!((ceiling.bottom - rules.ceiling_travel / 2.0).abs() < 2.0, "the ceiling half way down: {ceiling:?}");
    assert!((left.right - rules.side_travel / 2.0).abs() < 2.0, "the left wall half way in: {left:?}");

    game.players.push(Player::new(Character::Amy, 1000.0, ceiling.bottom + 4.0, &GameplayConfig::default()));
    hold_each(&mut game, &[0], 1);
    let amy = &game.players[0];
    assert!(amy.is_dead, "the ceiling crushed her");
    assert!((amy.x - rules.crushed_to[0]).abs() < 8.0, "and she lies at the middle: {}", amy.x);
}
