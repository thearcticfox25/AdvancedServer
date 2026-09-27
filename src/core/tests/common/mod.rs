//! Setup shared by the behavior tests: a real converted room with players in it.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use crate::core::resources::sprites::Sprites;
use crate::core::config::GameplayConfig;
use crate::core::events::SimEvent;
use crate::core::game::Game;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Buttons, Character, Player};
use crate::core::rooms::ids::RoomId;
use crate::core::world::World;

pub fn resources() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("Resources")
}

/// A started round in `room` with the default gameplay config and three minutes on the clock.
pub fn started_room(room: RoomId) -> World {
    let sprites = Arc::new(Sprites::index(&resources().join("Textures")).unwrap());
    let mut world = World::empty(sprites, Arc::new(GameplayConfig::default()));
    world.load_room(&resources().join("Maps"), room).unwrap();
    world.round_started = true;
    world.timer_ticks = 3 * 60 * 60;
    world
}

/// Green Hill with the given characters standing on its first spawn point.
pub fn greenhill_with(characters: &[Character]) -> Game {
    let world = started_room(RoomId::Greenhill);
    let spawn = world.ids_of(ObjectId::Spawnpoint).next().expect("greenhill has a spawn point");
    let (x, y) = (world.instances[spawn].x, world.instances[spawn].y - 18.0);
    let mut game = Game::new(world);
    for &character in characters {
        game.players.push(Player::new(character, x, y, &GameplayConfig::default()));
    }
    game
}

/// Green Hill with Sonic.EXE standing on its first spawn point.
pub fn greenhill_with_exe(exe: crate::core::player::ExeCharacter) -> Game {
    let mut game = greenhill_with(&[]);
    let spawn = game.world.ids_of(ObjectId::Spawnpoint).next().unwrap();
    let (x, y) = (game.world.instances[spawn].x, game.world.instances[spawn].y - 18.0);
    game.players.push(Player::new_exe(exe, x, y, &GameplayConfig::default()));
    game
}

/// Holds the same buttons for every player for `ticks` ticks; returns every event.
pub fn hold(game: &mut Game, buttons: u16, ticks: usize) -> Vec<SimEvent> {
    let pressed = vec![Buttons(buttons); game.players.len()];
    (0..ticks).flat_map(|_| game.tick(&pressed)).collect()
}

/// Holds different buttons per player.
pub fn hold_each(game: &mut Game, buttons: &[u16], ticks: usize) -> Vec<SimEvent> {
    let pressed: Vec<Buttons> = buttons.iter().map(|&b| Buttons(b)).collect();
    (0..ticks).flat_map(|_| game.tick(&pressed)).collect()
}

/// A flat piece of dry floor: the same first floor pixel in every column from
/// x - 48 to x + 48 (room for the bottom sensors and for Exetior's waves at 25 px),
/// above Green Hill's water, which slows players down.
pub fn flat_floor_spot(world: &World) -> (f64, f64) {
    let first_floor = |x: f64| (0..world.room_height as i64).find(|&y| world.position_meeting(x, y as f64, ObjectId::FloorParent));
    (200..4000)
        .step_by(8)
        .map(|x| x as f64)
        .find_map(|x| {
            let heights: Vec<Option<i64>> = (-48..=48).map(|dx| first_floor(x + dx as f64)).collect();
            match heights[0] {
                Some(height) if heights.iter().all(|h| *h == Some(height)) && !world.position_meeting(x, height as f64 - 1.0, ObjectId::GhzWater) => {
                    Some((x, height as f64 - 18.0 - 40.0))
                }
                _ => None,
            }
        })
        .expect("the room has flat floor somewhere")
}
