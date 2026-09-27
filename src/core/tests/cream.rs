//! Behavior checks of Cream's abilities in a real room.

use super::common::{greenhill_with, hold};
use crate::core::events::SimEvent;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Buttons, Character};
use crate::core::world::World;

/// Flat floor with nothing inside Cream's 32 px rings circle (spot 12 px behind, 16 px up).
/// Flat alone is not enough: a step right next to Cream blocks the rings, as in the original.
fn open_rings_spot(world: &World) -> (f64, f64) {
    let first_floor = |x: f64| (0..world.room_height as i64).find(|&y| world.position_meeting(x, y as f64, ObjectId::FloorParent));
    (200..4000)
        .step_by(8)
        .map(|x| x as f64)
        .find_map(|x| {
            let floor = first_floor(x)? as f64;
            let flat = (-8..=8).all(|dx| first_floor(x + dx as f64) == Some(floor as i64));
            let player_y = floor - 18.0;
            let circle_clear = !world.collision_circle_precise(x - 12.0, player_y - 16.0, 32.0, ObjectId::FloorParent);
            (flat && circle_clear).then_some((x, player_y - 40.0))
        })
        .expect("greenhill has an open spot for rings")
}

#[test]
fn jumping_again_while_rising_starts_the_glide() {
    let mut game = greenhill_with(&[Character::Cream]);
    hold(&mut game, 0, 120);
    hold(&mut game, Buttons::A, 1);
    hold(&mut game, 0, 2);
    hold(&mut game, Buttons::A, 1);
    let cream = &game.players[0];
    assert!(cream.is_flying, "{cream:?}");
    assert_eq!(cream.fly_timer, 15 * 60, "flight recharge starts and holds while flying");
}

#[test]
fn dash_raises_top_speed_then_resets_it() {
    let mut game = greenhill_with(&[Character::Cream]);
    hold(&mut game, 0, 120);
    hold(&mut game, Buttons::RIGHT, 20);
    hold(&mut game, Buttons::RIGHT | Buttons::B, 1);
    assert_eq!(game.players[0].max_h_speed, 13.0, "dash top speed");
    assert_eq!(game.players[0].acc, 0.8, "dash acceleration");
    hold(&mut game, Buttons::RIGHT, 40);
    assert_eq!(game.players[0].max_h_speed, 11.0, "back to normal after the dash");
}

#[test]
fn c_on_open_floor_spawns_rings_after_one_second() {
    let mut game = greenhill_with(&[Character::Cream]);
    let (x, y) = open_rings_spot(&game.world);
    game.players[0].x = x;
    game.players[0].y = y;
    hold(&mut game, 0, 120);
    assert!(!game.players[0].is_colliding, "open floor should allow rings: {:?}", game.players[0]);
    hold(&mut game, Buttons::C, 1);
    let events = hold(&mut game, 0, 70);
    assert!(events.iter().any(|event| matches!(event, SimEvent::SpawnCreamRings { demonized: false, .. })), "{events:?}");
}


#[test]
fn demonized_cream_places_no_rings_near_a_red_ring() {
    let mut game = greenhill_with(&[Character::Cream]);
    let (x, y) = open_rings_spot(&game.world);
    game.players[0].x = x;
    game.players[0].y = y;
    game.players[0].revival_times = 2;
    hold(&mut game, 0, 120);
    assert!(!game.players[0].is_colliding, "no red ring yet: {:?}", game.players[0]);
    let feet = game.players[0].y;
    game.world.rings.push(crate::core::rings::MapRing::new(x + 100.0, feet, true));
    hold(&mut game, 0, 1);
    assert!(game.players[0].is_colliding, "a red ring 100 px away blocks the rings");
}
