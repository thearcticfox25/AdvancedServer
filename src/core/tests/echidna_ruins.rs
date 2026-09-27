//! Echidna Ruins' crystals, lava platform and floating platforms.

use super::common::{hold_each, started_room};
use crate::core::config::{ticks, GameplayConfig};
use crate::core::game::Game;
use crate::core::level::Level;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Buttons, Character, Player};
use crate::core::rooms::ids::RoomId;
use crate::core::world::ObjectVars;

fn echidna_ruins() -> Game {
    let mut game = Game::new(started_room(RoomId::Marijuna));
    game.level = Some(Level::new(&game.world, 3));
    game
}

#[test]
fn a_lit_crystal_reverses_the_controls_for_a_while() {
    let mut game = echidna_ruins();
    let rules = game.world.config.levels.echidna_ruins.clone();
    hold_each(&mut game, &[], ticks(rules.crystal_seconds + 1.5) as usize);
    let crystal = game.world.ids_of(ObjectId::MarijunaCrystal).next().unwrap();
    assert!(matches!(game.world.instances[crystal].vars, ObjectVars::MjCrystal { lit: true, glow } if glow >= 1.0), "lit and glowing");
    let (x, y) = (game.world.instances[crystal].x, game.world.instances[crystal].y);
    let mut sally = Player::new(Character::Sally, x, y, &GameplayConfig::default());
    sally.yspd = 0.0;
    game.players.push(sally);
    hold_each(&mut game, &[0], 1);
    assert!(game.players[0].controls_reversed > 0, "reversed");
    // Away from the crystal, which would reverse them again.
    game.players[0].x = 100.0;
    game.players[0].xspd = 0.0;
    hold_each(&mut game, &[Buttons::RIGHT], 10);
    assert!(game.players[0].xspd < 0.0, "right goes left now: {}", game.players[0].xspd);
    hold_each(&mut game, &[0], ticks(rules.reversed_seconds) as usize);
    game.players[0].xspd = 0.0;
    hold_each(&mut game, &[Buttons::RIGHT], 10);
    assert!(game.players[0].xspd > 0.0, "and right again: {}", game.players[0].xspd);
}

#[test]
fn the_lava_platform_rises() {
    let mut game = echidna_ruins();
    let lava = game.world.ids_of(ObjectId::MarijunaLavaplatform).next().unwrap();
    let rules = game.world.config.levels.echidna_ruins.lava.clone();
    let mut highest = f64::MAX;
    for _ in 0..ticks(20.0) {
        hold_each(&mut game, &[], 1);
        highest = highest.min(game.world.instances[lava].y);
    }
    assert!(highest <= 1624.0 - rules.rise, "the lava rose: {highest}");
}
