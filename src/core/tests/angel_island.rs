//! Angel Island's ziplines.

use super::common::started_room;
use crate::core::resources::names::sound;
use crate::core::config::GameplayConfig;
use crate::core::events::SimEvent;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Buttons, Character, Player};
use crate::core::rooms::ids::RoomId;
use crate::core::world::{ObjectVars, World};

fn run(player: &mut Player, world: &World, buttons: u16, ticks: usize) -> Vec<SimEvent> {
    let mut events = Vec::new();
    for _ in 0..ticks {
        player.tick(world, Buttons(buttons), &mut events);
    }
    events
}

#[test]
fn a_zipline_carries_its_rider_to_the_end_and_lets_it_fly_on() {
    let world = started_room(RoomId::Angelisland);
    // The line of gid 1 runs to the right on one height.
    let (zipline, start, end) = world
        .ids_of(ObjectId::AizZipline)
        .find_map(|id| match world.instances[id].vars {
            ObjectVars::Zipline { start, end } if world.instances[end].x > world.instances[start].x + 300.0 && world.instances[end].y == world.instances[start].y => Some((id, start, end)),
            _ => None,
        })
        .expect("angel island has a level zipline to the right");
    let handle = &world.instances[zipline];
    let mut tails = Player::new(Character::Tails, handle.x + 12.0, handle.y + 20.0, &GameplayConfig::default());
    tails.is_grounded = false;

    let events = run(&mut tails, &world, 0, 1);
    assert!(events.iter().any(|event| matches!(event, SimEvent::Sound { sound: sound::SND_SNAP, .. })), "grabbing snaps: {tails:?}");
    assert!(tails.is_zipline && !tails.controls_enabled);

    let start_x = world.instances[start].x;
    run(&mut tails, &world, Buttons::RIGHT, 60);
    assert!(tails.x > start_x + 12.0 && tails.is_zipline, "the rider moves along the line: {}", tails.x);
    let pressed_jump_ride = tails.x;

    let mut ticks = 0;
    while tails.is_zipline && ticks < 600 {
        run(&mut tails, &world, 0, 1);
        ticks += 1;
    }
    assert!(tails.controls_enabled && tails.xspd > 0.0, "at the end the rider flies on: {tails:?}");
    assert!(tails.x > pressed_jump_ride && tails.x >= world.instances[end].x, "it got to the end: {}", tails.x);
    assert!(tails.ziplines.iter().any(|handle| handle.zipline == zipline), "the handle falls away");
}

#[test]
fn jump_lets_go_and_the_held_key_counts_as_up() {
    let world = started_room(RoomId::Angelisland);
    let zipline = world.ids_of(ObjectId::AizZipline).find(|&id| matches!(world.instances[id].vars, ObjectVars::Zipline { .. })).unwrap();
    let handle = &world.instances[zipline];
    let mut tails = Player::new(Character::Tails, handle.x + 12.0, handle.y + 20.0, &GameplayConfig::default());
    tails.is_grounded = false;
    run(&mut tails, &world, 0, 1);
    assert!(tails.is_zipline);
    // Too early to let go: the first tenth of the line.
    run(&mut tails, &world, Buttons::A, 1);
    assert!(tails.is_zipline, "jump does nothing before a tenth of the line");
    run(&mut tails, &world, 0, 120);
    run(&mut tails, &world, Buttons::A, 1);
    assert!(!tails.is_zipline && tails.ignore_jump_until_released, "jump lets go: {tails:?}");
    run(&mut tails, &world, Buttons::A, 5);
    assert!(tails.ignore_jump_until_released, "the key that let go counts as up while it stays down");
    run(&mut tails, &world, 0, 1);
    assert!(!tails.ignore_jump_until_released);
}
