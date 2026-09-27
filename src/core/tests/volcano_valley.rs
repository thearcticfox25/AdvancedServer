//! Volcano Valley's lava columns and vases.

use super::common::{hold_each, started_room};
use crate::core::config::GameplayConfig;
use crate::core::game::Game;
use crate::core::level::Level;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Character, Player};
use crate::core::rooms::ids::RoomId;
use crate::core::world::ObjectVars;

fn volcano_valley() -> Game {
    let mut game = Game::new(started_room(RoomId::Volcanovalley));
    game.level = Some(Level::new(&game.world, 5));
    game
}

#[test]
fn falling_onto_a_vase_breaks_it_for_its_rings_once() {
    let mut game = volcano_valley();
    let vase = game.world.ids_of(ObjectId::VvVase).next().unwrap();
    let (x, y) = (game.world.instances[vase].x, game.world.instances[vase].y);
    game.players.push(Player::new(Character::Tails, x, y - 60.0, &GameplayConfig::default()));
    hold_each(&mut game, &[0], 60);
    let rings = game.players[0].rings;
    assert!((1..=4).contains(&rings), "the vase's rings: {rings}");
    assert!(!game.world.instances[vase].visible, "broken");
    hold_each(&mut game, &[0], 60);
    assert_eq!(game.players[0].rings, rings, "a broken vase gives nothing more");
}

#[test]
fn a_rising_lava_column_burns() {
    let mut game = volcano_valley();
    let column = game.world.ids_of(ObjectId::VvLavacolumn).next().unwrap();
    let ObjectVars::LavaColumn { start_y, .. } = game.world.instances[column].vars else { panic!("columns know their place") };
    let x = game.world.instances[column].x;
    // Resting, then up high within half a minute.
    let mut rose = false;
    for _ in 0..30 * 60 {
        hold_each(&mut game, &[], 1);
        if game.world.instances[column].y < start_y - 100.0 {
            rose = true;
            break;
        }
    }
    assert!(rose, "the column rose");
    let top = game.world.bbox(column).unwrap().top;
    game.players.push(Player::new(Character::Knux, x, top + 30.0, &GameplayConfig::default()));
    hold_each(&mut game, &[0], 1);
    assert!(game.players[0].hp < 100, "burnt: {:?}", game.players[0]);
}
