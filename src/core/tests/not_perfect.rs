//! Not Perfect's stage switches.

use super::common::{hold_each, started_room};
use crate::core::config::{ticks, GameplayConfig};
use crate::core::events::SimEvent;
use crate::core::game::Game;
use crate::core::level::Level;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Character, Player};
use crate::core::rooms::ids::RoomId;
use crate::core::world::ObjectVars;

#[test]
fn everyone_moves_to_the_next_stage_after_the_warning() {
    let mut game = Game::new(started_room(RoomId::Notperfect));
    game.level = Some(Level::new(&game.world, 1));
    let rules = game.world.config.levels.not_perfect.clone();
    let spawn = game.world.ids_of(ObjectId::Spawnpoint).next().unwrap();
    let (x, y) = (game.world.instances[spawn].x, game.world.instances[spawn].y);
    game.players.push(Player::new(Character::Tails, x, y, &GameplayConfig::default()));
    // Keep the round clock far from the chase.
    let warning = ticks(rules.warning_after_seconds) as usize + 1;
    hold_each(&mut game, &[0], warning);
    let controller = game.world.ids_of(ObjectId::NpController).next().unwrap();
    assert!(matches!(game.world.instances[controller].vars, ObjectVars::NotPerfectController { stage: 0, warning: true, .. }), "warned");
    let before = game.players[0].x;
    let mut switched = false;
    for _ in 0..ticks(rules.switch_after_seconds) + 1 {
        switched |= hold_each(&mut game, &[0], 1).iter().any(|event| matches!(event, SimEvent::StageSwitched { .. }));
    }
    assert!(switched);
    assert!(matches!(game.world.instances[controller].vars, ObjectVars::NotPerfectController { stage: 1, warning: false, .. }));
    let moved = game.players[0].x - before;
    assert!((moved - rules.stage_offset[0]).abs() < 40.0, "moved one stage right: {moved}");
}

#[test]
fn a_player_outside_the_stage_is_put_back() {
    let mut game = Game::new(started_room(RoomId::Notperfect));
    game.level = Some(Level::new(&game.world, 1));
    let area = game.world.ids_of(ObjectId::NpTeleporn).find(|&id| matches!(game.world.instances[id].vars, ObjectVars::StageArea { stage: 0, .. })).unwrap();
    let ObjectVars::StageArea { back_x, .. } = game.world.instances[area].vars else { panic!() };
    game.players.push(Player::new(Character::Tails, 4000.0, 600.0, &GameplayConfig::default()));
    hold_each(&mut game, &[0], 1);
    assert!((game.players[0].x - back_x).abs() < 10.0, "back on stage 0: {}", game.players[0].x);
}
