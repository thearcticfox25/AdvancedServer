//! Priceless Freedom's lifts.

use super::common::{hold_each, started_room};
use crate::core::config::GameplayConfig;
use crate::core::game::Game;
use crate::core::level::Level;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Character, Player};
use crate::core::rooms::ids::RoomId;
use crate::core::world::ObjectVars;

#[test]
fn a_lift_carries_its_rider_to_the_top_lets_it_off_and_comes_back() {
    let world = started_room(RoomId::Pricelessfreedom);
    let mut game = Game::new(world);
    game.level = Some(Level::new(&game.world, 4));
    let lift = game.world.ids_of(ObjectId::PfLift).next().unwrap();
    let ObjectVars::Lift { start_y, top_y, .. } = game.world.instances[lift].vars else { panic!("lifts know their top") };
    let (x, y) = (game.world.instances[lift].x, start_y - 4.0);
    game.players.push(Player::new(Character::Tails, x, y, &GameplayConfig::default()));

    // A lift fades in before it takes anyone.
    hold_each(&mut game, &[0], 70);
    assert_eq!(game.players[0].riding_lift, Some(lift), "touching the faded in lift boards it: {:?}", game.players[0]);
    assert!(!game.players[0].controls_enabled);

    let mut ticks = 0;
    while game.players[0].riding_lift.is_some() && ticks < 1200 {
        hold_each(&mut game, &[0], 1);
        ticks += 1;
    }
    let tails = &game.players[0];
    assert!(tails.riding_lift.is_none() && tails.controls_enabled, "let off at the top: {tails:?}");
    assert!(tails.y < top_y + 20.0, "at the top {top_y}: {}", tails.y);

    hold_each(&mut game, &[0], 100);
    assert_eq!(game.world.instances[lift].y, start_y, "back at its start after resting");
}
