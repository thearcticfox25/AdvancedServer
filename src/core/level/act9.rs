//! Act 9's walls (the server's act9wall): as the round's time runs out the ceiling
//! comes down and the side walls close in, until nothing is left of the arena.

use crate::core::objects::ids::ObjectId;
use crate::core::world::{ObjectVars, World};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Act9Walls {
    /// The round's time left when the walls started, in ticks.
    start: Option<i64>,
}

impl Act9Walls {
    /// How far the walls have closed, 0 to 1.
    pub(super) fn tick(&mut self, world: &World) -> f32 {
        let start = *self.start.get_or_insert(world.timer_ticks);
        if start > 0 {
            ((start - world.timer_ticks) as f64 / start as f64) as f32
        } else {
            0.0
        }
    }
}

/// SERVER_ACT9WALL_STATE: each wall's edge at its share of the way (whole pixels, as sent).
pub(super) fn apply(world: &mut World, closed: f32) {
    let rules = world.config.levels.act9.clone();
    let room_width = world.room_width;
    let walls: Vec<_> = world.ids_of(ObjectId::Act9Wall).collect();
    for wall in walls {
        let ObjectVars::Act9Wall { nid } = world.instances[wall].vars else { continue };
        let ceiling = (rules.ceiling_travel * closed as f64) as u16 as f64;
        let side = (rules.side_travel * closed as f64) as u16 as f64;
        let instance = &mut world.instances[wall];
        let (x, y) = match nid {
            0 => (-WALL_THICKNESS, ceiling - CEILING_THICKNESS),
            1 => (side - WALL_THICKNESS, 0.0),
            _ => (room_width - side, 0.0),
        };
        (instance.x, instance.y) = (x, y);
    }
}

/// The walls are this thick (their scaled sprite in the map).
const WALL_THICKNESS: f64 = 2240.0;
const CEILING_THICKNESS: f64 = 768.0;
