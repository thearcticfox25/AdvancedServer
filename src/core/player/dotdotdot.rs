//! DotDotDot's ladders as a player climbs them: obj_dotdotdot_i gives back the normal
//! speed every Begin Step, and a ladder at a side sensor caps it (obj_dotdotdot_shitladder).

use super::animation::WALK;
use super::{movement_config, Character, Player};
use crate::core::config::GameplayConfig;
use crate::core::objects::ids::ObjectId;
use crate::core::rooms::ids::RoomId;
use crate::core::world::World;

impl Player {
    pub(super) fn climb_ladders(&mut self, world: &World, cfg: &GameplayConfig) {
        if world.room != Some(RoomId::Dotdotdot) {
            return;
        }
        // Every EXE gets the original EXE's numbers here.
        let movement = if self.character == Character::Exe { &cfg.exe.movement } else { movement_config(self.character, cfg) };
        if !self.is_slow {
            self.max_h_speed = movement.max_speed_per_tick;
            self.acc = movement.acceleration_per_tick;
        }
        let ladders: Vec<_> = world.ids_of(ObjectId::DotdotdotShitladder).collect();
        self.on_ladder = world.position_meeting_any(self.sensor_l.x, self.sensor_l.y, &ladders) || world.position_meeting_any(self.sensor_r.x, self.sensor_r.y, &ladders);
        if self.on_ladder && !self.is_slow {
            let cap = cfg.levels.dotdotdot.ladder_max_speed;
            if self.xspd.abs() > cap {
                self.xspd = cap * self.xspd.signum();
                self.gspd = self.xspd;
            }
            self.max_h_speed = cap;
        }
    }

    /// obj_dotdotdot_shitladder Draw Begin: walking up a ladder is slower to look at.
    pub(super) fn slow_ladder_walk(&mut self, cfg: &GameplayConfig) {
        if self.on_ladder && self.state == WALK {
            self.image_speed /= cfg.levels.dotdotdot.ladder_walk_slowdown;
        }
    }
}
