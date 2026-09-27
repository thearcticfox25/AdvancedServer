//! What Haunting Dream's doors and crystals do to a player: obj_hd_door pushes a body
//! out of its way and crushes whoever is under it while it closes, obj_hd_crystal
//! opens or closes them when looked at.

use super::{Buttons, Character, Player};
use crate::core::collision::sprite_bbox;
use crate::core::config::GameplayConfig;
use crate::core::events::SimEvent;
use crate::core::objects::ids::ObjectId;
use crate::core::world::{InstanceId, ObjectVars, World};

/// obj_hd_door2 checks a box this big around the player instead of its sensors.
const BODY_HALF_WIDTH: f64 = 8.0;
const BODY_HALF_HEIGHT: f64 = 18.0;
/// obj_hd_door2 leaves this much of its edge out at the top and the bottom.
const DOOR2_EDGE_MARGIN: f64 = 2.0;
/// Enough to leave any door; the original loops until the body is out.
const MAX_PUSH: usize = 256;

impl Player {
    /// obj_hd_door Step: a dead body inside a door is pushed out to the right.
    pub(super) fn push_body_out_of_doors(&mut self, world: &World) {
        if !self.is_dead {
            return;
        }
        for door in world.ids_of(ObjectId::HdDoor) {
            let Some(bbox) = world.bbox(door) else { continue };
            for _ in 0..MAX_PUSH {
                let body = sprite_bbox(world.sprites.get(self.sprite_index), self.x, self.y, self.image_xscale, 1.0, 0.0);
                if !bbox.overlaps(&body) {
                    break;
                }
                self.x += 1.0;
            }
        }
    }

    /// obj_hd_door and obj_hd_door2 End Step: the edge of a door closing on a survivor
    /// kills it; EXE and demons are put on the nearest death teleport point instead.
    pub(super) fn crush_under_doors(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let doors: Vec<InstanceId> = world.ids_of(ObjectId::HdDoor).chain(world.ids_of(ObjectId::HdDoor2)).collect();
        for door in doors {
            let (ObjectVars::Door { closing: true, .. }, Some(bbox)) = (&world.instances[door].vars, world.bbox(door)) else { continue };
            let crushed = if world.instances[door].object == ObjectId::HdDoor {
                // point_in_rectangle(i, y + sprite_height + 1, sensorTL, sensorBR) for each column.
                let below = bbox.bottom + 1.0;
                let (left, right) = (self.sensor_tl.x, self.sensor_br.x);
                let (top, bottom) = (self.sensor_tl.y, self.sensor_br.y);
                below >= top && below <= bottom && bbox.left.max(left) <= (bbox.right - 1.0).min(right)
            } else {
                let beside = bbox.right + 1.0;
                let (top, bottom) = (bbox.top + DOOR2_EDGE_MARGIN, bbox.bottom - DOOR2_EDGE_MARGIN);
                let body_top = self.y - BODY_HALF_HEIGHT;
                let body_bottom = self.y + BODY_HALF_HEIGHT;
                beside >= self.x - BODY_HALF_WIDTH && beside <= self.x + BODY_HALF_WIDTH && top.max(body_top) < bottom.min(body_bottom + 1.0)
            };
            if !crushed {
                continue;
            }
            if self.character == Character::Exe || self.is_demonized() {
                if let Some(point) = world.instance_nearest(self.x, self.y, ObjectId::DeathtpPoint) {
                    self.x = world.instances[point].x;
                    self.y = world.instances[point].y;
                }
            } else {
                self.instakill(world, cfg, events);
            }
        }
    }

    /// obj_hd_crystal Draw: looking up or down at a crystal and pressing up or down again
    /// asks for the doors (the crystals decide whether they are resting).
    pub(super) fn use_crystals(&self, world: &World, events: &mut Vec<SimEvent>) {
        if !(self.is_looking_down || self.is_looking_up) || !(self.pressed_raw(Buttons::DOWN) || self.pressed_raw(Buttons::UP)) {
            return;
        }
        if self.meeting_at(world, self.x, self.y, ObjectId::HdCrystal) {
            events.push(SimEvent::CrystalUsed);
        }
    }
}
