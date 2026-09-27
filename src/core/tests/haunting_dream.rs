//! Haunting Dream's crystals and doors.

use super::common::{hold_each, started_room};
use crate::core::config::GameplayConfig;
use crate::core::game::Game;
use crate::core::level::Level;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Buttons, Character, Player};
use crate::core::rooms::ids::RoomId;
use crate::core::world::ObjectVars;

fn haunting_dream() -> Game {
    let world = started_room(RoomId::Haundream);
    let mut game = Game::new(world);
    game.level = Some(Level::new(&game.world, 9));
    game
}

#[test]
fn looking_up_at_a_crystal_opens_the_doors_a_pixel_a_tick() {
    let mut game = haunting_dream();
    let crystal = game.world.ids_of(ObjectId::HdCrystal).next().unwrap();
    let (x, y) = (game.world.instances[crystal].x + 12.0, game.world.instances[crystal].y + 12.0);
    game.players.push(Player::new(Character::Tails, x, y, &GameplayConfig::default()));
    hold_each(&mut game, &[0], 60);
    let tails = &game.players[0];
    assert!(tails.is_grounded && tails.meeting_at(&game.world, tails.x, tails.y, ObjectId::HdCrystal), "standing at the crystal: {tails:?}");

    hold_each(&mut game, &[Buttons::UP], 2);
    let door = game.world.ids_of(ObjectId::HdDoor).next().unwrap();
    let ObjectVars::Door { closed_y, .. } = game.world.instances[door].vars else { panic!() };
    hold_each(&mut game, &[0], 30);
    assert!(matches!(game.world.instances[door].vars, ObjectVars::Door { open: true, .. }));
    let slid = closed_y - game.world.instances[door].y;
    assert!((29.0..=31.0).contains(&slid), "a pixel a tick: {slid}");
}

#[test]
fn a_closing_door_crushes_a_survivor_under_it() {
    let mut game = haunting_dream();
    let door = game.world.ids_of(ObjectId::HdDoor).next().unwrap();
    game.level.as_mut().unwrap().toggle_doors(&game.world.config.clone());
    // Open, and the crystals rest for ten seconds.
    hold_each(&mut game, &[], 600);
    game.level.as_mut().unwrap().toggle_doors(&game.world.config.clone());
    let bbox = game.world.bbox(door).unwrap();
    let mut tails = Player::new(Character::Tails, (bbox.left + bbox.right) / 2.0, bbox.bottom + 20.0, &GameplayConfig::default());
    tails.is_grounded = true;
    game.players.push(tails);
    let mut died = false;
    for _ in 0..90 {
        let events = hold_each(&mut game, &[0], 1);
        died |= events.iter().any(|event| matches!(event, crate::core::events::SimEvent::PlayerDied { .. }));
    }
    assert!(died, "the door came down on Tails: {:?}", game.players[0]);
}
