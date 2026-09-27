//! Act 9's bodies (obj_act9_bodies): each character's body floats in the middle, and
//! fades in once nobody plays that character or its player is dead for good.

use crate::core::player::{Character, Player};
use crate::core::objects::ids::ObjectId;
use crate::core::world::{InstanceId, World};
use macroquad::rand::gen_range;
use crate::core::config::step;

/// y = sY + sin(current_time / 550 + sS) * 4, sS = random_range(0, 25).
const BOB_MILLISECONDS_PER_RADIAN: f64 = 550.0;
const BOB: f64 = 4.0;
const PHASE_RANGE: f64 = 25.0;
const FADE_IN: f64 = 0.016 * 0.25;

pub struct Act9Bodies {
    /// Each body, where the map put it, and its phase.
    bodies: Vec<(InstanceId, f64, f64)>,
}

impl Act9Bodies {
    pub fn open(world: &mut World) -> Act9Bodies {
        let ids: Vec<InstanceId> = world.ids_of(ObjectId::Act9Bodies).collect();
        let bodies: Vec<(InstanceId, f64, f64)> = ids.into_iter().map(|id| (id, world.instances[id].y, gen_range(0.0, PHASE_RANGE))).collect();
        for &(id, _, _) in &bodies {
            world.instances[id].image_alpha = 0.0;
        }
        Act9Bodies { bodies }
    }

    /// `players`: everyone in the round this client knows of, itself included.
    pub fn step<'a>(&self, world: &mut World, players: impl Iterator<Item = &'a Player> + Clone, current_time_ms: f64) {
        for &(id, start_y, phase) in &self.bodies {
            let instance = &mut world.instances[id];
            instance.y = start_y + (current_time_ms / BOB_MILLISECONDS_PER_RADIAN + phase).sin() * BOB;
            // image_index is the character's number less one.
            let still_played = players.clone().any(|player| {
                player.character != Character::Exe && player.character as i32 - 1 == instance.image_index as i32 && !(player.revival_times == 1 && player.hp <= 0)
            });
            if !still_played && instance.image_alpha < 1.0 {
                instance.image_alpha += FADE_IN * step();
            }
        }
    }
}
