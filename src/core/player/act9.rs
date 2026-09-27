//! Act 9's closing walls as a player meets them (obj_act9_wall Begin Step): a body is
//! kept out of them, and whoever they squeeze against the terrain, or the ceiling
//! reaches, dies and comes back at the arena's middle.

use super::{gm_sign, Character, Player};
use crate::core::resources::names::{sound, sprite};
use crate::core::config::GameplayConfig;
use crate::core::events::SimEvent;
use crate::core::objects::ids::ObjectId;
use crate::core::world::{ObjectVars, World};

/// A body is kept this far from a side wall, and this far below the ceiling, which
/// kills at half that.
const SIDE_GAP: f64 = 8.0;
const CEILING_GAP: f64 = 16.0;
const CEILING_KILLS_WITHIN: f64 = 8.0;

impl Player {
    pub(super) fn meet_act9_walls(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        for wall in world.ids_of(ObjectId::Act9Wall) {
            let (ObjectVars::Act9Wall { nid }, Some(bbox)) = (&world.instances[wall].vars, world.bbox(wall)) else { continue };
            // GameMaker's bbox_right and bbox_bottom are the last pixels inside.
            let (right, bottom) = (bbox.right - 1.0, bbox.bottom - 1.0);
            let squeezed = self.sensor_l.coll && self.sensor_r.coll;
            match nid {
                1 => {
                    if self.x - SIDE_GAP <= right {
                        self.x = right.ceil() + SIDE_GAP;
                    }
                    if squeezed && world.collides_point(wall, self.sensor_l.x, self.sensor_l.y) {
                        self.crushed(world, cfg, gm_sign(world.instances[wall].x - self.x), events);
                    }
                }
                2 => {
                    if self.x + SIDE_GAP >= bbox.left {
                        self.x = bbox.left.ceil() - SIDE_GAP;
                    }
                    if squeezed && world.collides_point(wall, self.sensor_r.x, self.sensor_r.y) {
                        self.crushed(world, cfg, gm_sign(right - self.x), events);
                    }
                }
                _ => {
                    if self.y <= bottom + CEILING_GAP {
                        if !self.is_dead && self.y <= bottom + CEILING_KILLS_WITHIN {
                            self.crushed(world, cfg, gm_sign(right - self.x), events);
                        } else {
                            self.y = bottom + CEILING_GAP;
                        }
                    }
                }
            }
        }
    }

    /// A survivor dies (EXE and demons only move); everyone comes back at the middle.
    fn crushed(&mut self, world: &World, cfg: &GameplayConfig, blood_side: f64, events: &mut Vec<SimEvent>) {
        if !self.is_dead && self.character != Character::Exe && self.revival_times < 2 {
            self.instakill(world, cfg, events);
            events.push(SimEvent::Sound { sound: sound::SND_DEAD, x: self.x, y: self.y });
            events.push(SimEvent::Effect { sprite: sprite::SPR_BLOOD2, x: self.x, y: self.y, xscale: blood_side, image_speed: 1.0, yspd: 0.0 });
        }
        [self.x, self.y] = cfg.levels.act9.crushed_to;
        self.gspd = 0.0;
        self.xspd = 0.0;
        self.yspd = 0.0;
        self.is_hurt = false;
        self.hurttime = 0;
    }
}
