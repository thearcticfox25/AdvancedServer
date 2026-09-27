//! Limp City's electric chains and eyes.

use super::common::{hold_each, started_room};
use crate::core::config::{ticks, GameplayConfig};
use crate::core::game::Game;
use crate::core::level::Level;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Buttons, Character, Player};
use crate::core::rooms::ids::RoomId;
use crate::core::world::ObjectVars;

fn limp_city() -> Game {
    let mut game = Game::new(started_room(RoomId::Limpcity));
    game.level = Some(Level::new(&game.world, 5));
    game
}

#[test]
fn the_chains_shock_after_their_warning() {
    let mut game = limp_city();
    let rules = game.world.config.levels.limp_city.clone();
    let chain = game.world.ids_of(ObjectId::LimpcityEchain1).next().unwrap();
    let active = |game: &Game| matches!(game.world.instances[chain].vars, ObjectVars::ElectricChain { state: 2 });
    hold_each(&mut game, &[], ticks(rules.chain_rest_seconds + rules.chain_warning_seconds) as usize);
    assert!(!active(&game), "still warning");
    hold_each(&mut game, &[], 3);
    assert!(active(&game), "shocking");
    let bbox = game.world.bbox(chain).unwrap();
    game.players.push(Player::new(Character::Knux, (bbox.left + bbox.right) / 2.0, (bbox.top + bbox.bottom) / 2.0, &GameplayConfig::default()));
    hold_each(&mut game, &[0], 1);
    assert!(game.players[0].hp < 100);
}

#[test]
fn looking_down_at_an_eye_shows_another_until_the_charge_runs_out() {
    let mut game = limp_city();
    let rules = game.world.config.levels.limp_city.clone();
    let eye = game.world.ids_of(ObjectId::LimpcityEyea).next().unwrap();
    let ObjectVars::Eye { nid, targets: Some((below, _)), .. } = game.world.instances[eye].vars else { panic!("an eyeA") };
    let (x, y) = (game.world.instances[eye].x, game.world.instances[eye].y);
    game.players.push(Player::new(Character::Tails, x, y, &GameplayConfig::default()));
    let watched = |game: &Game| game.world.ids_of(ObjectId::LimpcityEyeb).find(|&id| matches!(game.world.instances[id].vars, ObjectVars::Eye { nid, used: true, .. } if nid == below));
    for _ in 0..30 {
        let (tx, ty) = (game.world.instances[eye].x, game.world.instances[eye].y);
        (game.players[0].x, game.players[0].y, game.players[0].yspd) = (tx, ty, 0.0);
        game.players[0].is_looking_down = true;
        hold_each(&mut game, &[Buttons::DOWN], 1);
        if game.players[0].watching_eye.is_some() {
            break;
        }
    }
    assert_eq!(game.players[0].watching_eye, Some(nid), "looking through it");
    game.players[0].is_looking_down = true;
    hold_each(&mut game, &[Buttons::DOWN], 1);
    assert!(watched(&game).is_some(), "the other eye shows it is watched");

    // Staying on: the charge lasts a few seconds.
    let mut lasted = 0;
    while game.players[0].watching_eye.is_some() && lasted < ticks(10.0) {
        let (tx, ty) = (game.world.instances[eye].x, game.world.instances[eye].y);
        (game.players[0].x, game.players[0].y, game.players[0].yspd) = (tx, ty, 0.0);
        game.players[0].is_looking_down = true;
        hold_each(&mut game, &[Buttons::DOWN], 1);
        lasted += 1;
    }
    // 100 charge, a use cost a second, and it stops below the minimum.
    let seconds = (100 - rules.eye_min_charge) / rules.eye_use_cost;
    assert!(lasted >= ticks(seconds as f64 - 1.0) && lasted <= ticks(seconds as f64 + 1.0), "ran dry after {lasted} ticks");
}

