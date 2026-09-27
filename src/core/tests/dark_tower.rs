//! Dark Tower's balls, stalactites and Tails Doll.

use super::common::{hold_each, started_room};
use crate::core::config::{ticks, GameplayConfig};
use crate::core::events::SimEvent;
use crate::core::game::Game;
use crate::core::level::Level;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Character, Player};
use crate::core::rooms::ids::RoomId;
use crate::core::world::ObjectVars;

fn dark_tower() -> Game {
    let mut game = Game::new(started_room(RoomId::Dartower));
    game.level = Some(Level::new(&game.world, 7));
    game
}

#[test]
fn a_stalactite_falls_on_whoever_walks_under_it_breaks_and_comes_back() {
    let mut game = dark_tower();
    let stalactite = game.world.ids_of(ObjectId::DarktowerStalactite).next().unwrap();
    let ObjectVars::Stalactite { start_y, .. } = game.world.instances[stalactite].vars else { panic!("stalactites know their place") };
    let x = game.world.instances[stalactite].x + 40.0;
    // Far from the doll's spots; the player hangs in the air below it.
    game.players.push(Player::new(Character::Knux, x, start_y + 120.0, &GameplayConfig::default()));
    let mut hurt = false;
    let mut broke_after = None;
    for tick in 0..120 {
        game.players[0].y = start_y + 120.0;
        game.players[0].yspd = 0.0;
        hold_each(&mut game, &[0], 1);
        hurt |= game.players[0].hp < 100;
        if !game.world.instances[stalactite].visible {
            broke_after = Some(tick);
            break;
        }
    }
    assert!(hurt, "the falling stalactite hurt the player");
    assert!(broke_after.is_some(), "and broke on the floor");
    game.players.clear();
    let rules = game.world.config.levels.dark_tower.clone();
    hold_each(&mut game, &[], ticks(rules.stalactite_back_seconds + rules.stalactite_back_extra_seconds as f64) as usize);
    assert!(game.world.instances[stalactite].visible, "back a while later");
    assert_eq!(game.world.instances[stalactite].y, start_y);
}

#[test]
fn the_tails_doll_notices_chases_and_scares_a_survivor() {
    let mut game = dark_tower();
    let rules = game.world.config.levels.dark_tower.clone();
    game.players.push(Player::new(Character::Tails, 0.0, 0.0, &GameplayConfig::default()));
    hold_each(&mut game, &[0], 1);
    let doll = game.world.ids_of(ObjectId::DarktowerTailsdoll).next().unwrap();
    let (x, y) = (game.world.instances[doll].x, game.world.instances[doll].y);
    assert!(game.world.instances[doll].visible && rules.doll_spots.contains(&[x, y]), "waits at a spot: {x} {y}");

    let mut events = Vec::new();
    for _ in 0..ticks(5.0) {
        // A survivor that stays put in the air near the doll.
        game.players[0].x = x + 100.0;
        game.players[0].y = y;
        game.players[0].yspd = 0.0;
        events.extend(hold_each(&mut game, &[0], 1));
        if events.iter().any(|event| matches!(event, SimEvent::TailsDollCaught { .. })) {
            break;
        }
    }
    let order: Vec<_> = events.iter().filter(|event| event.done_to() == Some(0)).collect();
    assert!(matches!(order[..], [SimEvent::TailsDollNoticed { .. }, SimEvent::TailsDollChases { .. }, SimEvent::TailsDollCaught { .. }]), "{order:?}");
    assert!(game.players[0].is_slow, "slowed by the scare");
}

#[test]
fn the_balls_swing_and_hurt() {
    let mut game = dark_tower();
    let ball = game.world.ids_of(ObjectId::DarktowerBall).next().unwrap();
    hold_each(&mut game, &[], 30);
    let ObjectVars::DarkTowerBall { start_y, .. } = game.world.instances[ball].vars else { panic!() };
    assert!(game.world.instances[ball].y != start_y, "swinging");
    let (x, y) = (game.world.instances[ball].x, game.world.instances[ball].y);
    game.players.push(Player::new(Character::Knux, x, y, &GameplayConfig::default()));
    hold_each(&mut game, &[0], 1);
    assert!(game.players[0].hp < 100);
}

