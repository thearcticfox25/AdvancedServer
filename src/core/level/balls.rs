//! The swinging balls of Dark Tower and Fart Zone (the server's dt_ball): every ball
//! swings up and down its map distance together.

use crate::core::config::{step, DarkTowerRules};
use crate::core::objects::ids::ObjectId;
use crate::core::world::{ObjectVars, World};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BallSwing {
    /// Between -1 and 1.
    swing: f64,
    going_down: bool,
}

impl BallSwing {
    pub(super) fn tick(&mut self, rules: &DarkTowerRules) {
        if self.going_down {
            self.swing += rules.ball_shift_per_tick * step();
            if self.swing >= 1.0 {
                self.going_down = false;
            }
        } else {
            self.swing -= rules.ball_shift_per_tick * step();
            if self.swing <= -1.0 {
                self.going_down = true;
            }
        }
    }

    pub(super) fn swing(&self) -> f32 {
        self.swing as f32
    }
}

/// SERVER_DTBALL_STATE: y = sY + state * dir * dist.
pub(super) fn apply(world: &mut World, swing: f32) {
    let balls: Vec<_> = world.ids_of(ObjectId::DarktowerBall).collect();
    for ball in balls {
        if let ObjectVars::DarkTowerBall { start_y, dir, dist } = world.instances[ball].vars {
            world.instances[ball].y = start_y + swing as f64 * dir * dist;
        }
    }
}
