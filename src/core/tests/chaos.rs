//! Behavior checks of Chaos in a real room.

use super::common::{flat_floor_spot, greenhill_with_exe, hold};
use crate::core::player::{Buttons, ExeCharacter};

#[test]
fn liquid_form_speeds_up_and_can_be_cancelled() {
    let mut game = greenhill_with_exe(ExeCharacter::Chaos);
    hold(&mut game, 0, 120);
    hold(&mut game, Buttons::C, 1);
    let chaos = &game.players[0];
    assert_eq!(chaos.slime_timer, 15 * 60 - 1, "15 seconds of liquid form");
    assert!((chaos.acc - 0.056875 * 1.5).abs() < 1e-12);
    assert!((chaos.max_h_speed - 12.0 * 1.2).abs() < 1e-12);
    assert!(!chaos.can_spin, "no spinning while liquid");

    hold(&mut game, 0, 5);
    hold(&mut game, Buttons::C, 1);
    assert!(game.players[0].slime_timer > 0, "cannot cancel in the first 10 ticks");
    hold(&mut game, 0, 10);
    hold(&mut game, Buttons::C, 1);
    let chaos = &game.players[0];
    assert!(chaos.slime_timer <= 0, "a later C press cancels");
    assert!((chaos.acc - 0.056875).abs() < 1e-12, "speeds back to normal");
}

#[test]
fn air_dash_follows_the_arrows_for_fifteen_ticks() {
    let mut game = greenhill_with_exe(ExeCharacter::Chaos);
    hold(&mut game, 0, 120);
    hold(&mut game, Buttons::A, 1);
    hold(&mut game, 0, 2);
    hold(&mut game, Buttons::B | Buttons::RIGHT | Buttons::UP, 1);
    let chaos = &game.players[0];
    assert!(chaos.is_attacking);
    assert_eq!((chaos.dash_dir_x, chaos.dash_dir_y), (1.0, -1.0));
    hold(&mut game, 0, 5);
    assert!(game.players[0].is_attacking, "still dashing");
    hold(&mut game, 0, 15);
    assert!(!game.players[0].is_attacking, "the air dash lasts 15 ticks");
}

#[test]
fn dashing_down_onto_the_floor_sticks_and_dashes_off() {
    let mut game = greenhill_with_exe(ExeCharacter::Chaos);
    let (x, y) = flat_floor_spot(&game.world);
    game.players[0].x = x;
    game.players[0].y = y;
    hold(&mut game, 0, 120);
    hold(&mut game, Buttons::A, 1);
    hold(&mut game, 0, 10);
    hold(&mut game, Buttons::B | Buttons::DOWN, 1);
    let mut stuck = false;
    for _ in 0..24 {
        hold(&mut game, 0, 1);
        if game.players[0].stuck_timer > 0 {
            stuck = true;
            break;
        }
    }
    assert!(stuck, "a downward dash landing sticks Chaos to the floor: {:?}", game.players[0]);
    hold(&mut game, Buttons::B | Buttons::RIGHT, 1);
    let chaos = &game.players[0];
    assert_eq!(chaos.stuck_timer, 0, "B plus the arrow away from the surface leaves it");
    assert!(chaos.stuck_dash_timer > 0, "with a short burst");
}

#[test]
fn air_dash_into_a_stickable_wall_sticks_to_it() {
    use crate::core::objects::ids::ObjectId;
    let mut game = greenhill_with_exe(ExeCharacter::Chaos);
    let world = &game.world;
    // A tall stickable wall with at least 60 px of free air to its right.
    let spot = world.ids_of(ObjectId::SolidBlock).find_map(|id| {
        let wall = world.bbox(id)?;
        let tall = wall.bottom - wall.top >= 64.0;
        let (x, y) = (wall.right + 30.0, wall.top + 32.0);
        let free = (0..60).all(|dx| (-24..24).all(|dy| !world.position_meeting(wall.right + 1.0 + dx as f64, y + dy as f64, ObjectId::FloorParent)));
        (world.instances[id].can_stuck && tall && free).then_some((x, y))
    });
    let (x, y) = spot.expect("greenhill has a free-standing stickable wall");
    game.players[0].x = x;
    game.players[0].y = y;

    hold(&mut game, Buttons::B | Buttons::LEFT, 1);
    let mut stuck = false;
    for _ in 0..10 {
        hold(&mut game, Buttons::LEFT, 1);
        if game.players[0].stuck_timer > 0 {
            stuck = true;
            break;
        }
    }
    let chaos = &game.players[0];
    assert!(stuck, "dashing into the wall sticks Chaos to it: {chaos:?}");
    assert_eq!(chaos.stuck_dir, 1.0, "facing away from the wall on the left");
    // Only the arrow pointing away from the surface is remembered (up, on a wall).
    assert!(!chaos.up_pressed, "up was not held on impact");

    // The tick after impact still finishes the dash (the wall cut its timer), which
    // returns from the ability before the stuck logic; a B press there is lost, as in
    // the original. So wait two ticks, well inside the escape window.
    hold(&mut game, 0, 2);
    hold(&mut game, Buttons::B | Buttons::UP, 1);
    let chaos = &game.players[0];
    assert_eq!(chaos.stuck_timer, 0, "B plus up jumps off the wall");
    assert!(chaos.is_attacking && chaos.stuck_dash_timer > 0, "as an upward dash: {chaos:?}");
    assert_eq!((chaos.dash_dir_x, chaos.dash_dir_y), (0.0, -1.4));
}
