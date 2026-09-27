//! What Nasty Paradise does to a player: its ice blocks break under feet, attacks and a
//! gliding Knuckles (scr_collision_objects_before, obj_nap_iceblock Step), and its
//! snowballs roll over whoever is in their way (obj_nap_snowball Step).

use super::hurt::Hit;
use super::knux::KNUX_GLIDE;
use super::{Character, Player};
use crate::core::collision::sprite_bbox;
use crate::core::config::{ticks, GameplayConfig};
use crate::core::events::SimEvent;
use crate::core::objects::ids::ObjectId;
use crate::core::world::{ObjectVars, World};

/// The feet are this far below the position.
const FEET_BELOW: f64 = 19.0;
/// The bottom sensors look this far under themselves, plus the fall speed.
const SENSOR_LOOKS_DOWN: f64 = 2.0;
/// spr_nap_snowball frames from here on roll; the ones before only start moving.
const FIRST_ROLLING_FRAME: f64 = 8.0;

impl Player {
    /// scr_collision_objects_before, before the terrain: standing on an ice block throws
    /// the player up and breaks it, and so does an attack into one. A broken block has an
    /// empty mask, so nothing meets it.
    pub(super) fn break_ice(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        if self.is_dead {
            return;
        }
        let body = sprite_bbox(world.sprites.get(self.sprite_index), self.x, self.y, self.image_xscale, 1.0, 0.0);
        for block in world.ids_of(ObjectId::NapIceblock) {
            let (ObjectVars::IceBlock { nid }, Some(bbox)) = (&world.instances[block].vars, world.bbox(block)) else { continue };
            let yspd = self.yspd;
            let looked_at = |sensor: super::Sensor| world.collides_point(block, sensor.x, sensor.y + SENSOR_LOOKS_DOWN + yspd);
            if self.y + FEET_BELOW > bbox.top && (looked_at(self.sensor_bl) || looked_at(self.sensor_br)) {
                self.yspd = cfg.levels.nasty_paradise.ice_bounce_speed;
                self.is_jumping = true;
                self.is_grounded = false;
                events.push(SimEvent::IceBlockHit { nid: *nid });
            }
            if self.is_attacking && bbox.overlaps(&body) {
                events.push(SimEvent::IceBlockHit { nid: *nid });
            }
        }
    }

    /// obj_nap_iceblock Step: Knuckles glides through the blocks.
    pub(super) fn glide_through_ice(&self, world: &World, events: &mut Vec<SimEvent>) {
        if self.character != Character::Knux || self.state != KNUX_GLIDE {
            return;
        }
        let body = sprite_bbox(world.sprites.get(self.sprite_index), self.x, self.y, self.image_xscale, 1.0, 0.0);
        for block in world.ids_of(ObjectId::NapIceblock).filter(|&block| world.instances[block].visible) {
            if let (ObjectVars::IceBlock { nid }, Some(bbox)) = (&world.instances[block].vars, world.bbox(block)) {
                if bbox.overlaps(&body) {
                    events.push(SimEvent::IceBlockHit { nid: *nid });
                }
            }
        }
    }

    /// obj_nap_snowball Step: a rolling snowball hurts and slows whoever it touches.
    pub(super) fn roll_over_by_snowballs(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let body = sprite_bbox(world.sprites.get(self.sprite_index), self.x, self.y, self.image_xscale, 1.0, 0.0);
        let rolled_over = world.ids_of(ObjectId::NapSnowball).any(|ball| {
            let instance = &world.instances[ball];
            instance.visible && instance.image_index >= FIRST_ROLLING_FRAME && world.bbox(ball).is_some_and(|bbox| bbox.overlaps(&body))
        });
        if !rolled_over {
            return;
        }
        let rules = &cfg.levels.nasty_paradise;
        // isSlow and alarm[4] only: the speed stays, and is set back to normal when it rings.
        self.is_slow = true;
        self.slow_ticks = ticks(rules.snowball_slow_seconds);
        let hit = Hit { xpw: -self.image_xscale * rules.snowball_knockback_x, ..Hit::damage(cfg, rules.snowball_damage) };
        self.hurt(world, cfg, hit, events);
    }
}

