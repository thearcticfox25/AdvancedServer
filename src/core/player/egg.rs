//! obj_egg + scr_egg_special: jetpack double jump, electric shield, tracker mines.

use super::animation::{BALANCING, FALL, HURT, IDLE, JUMP, LOOKDOWN, LOOKUP, WALK};
use super::{Buttons, Player};
use crate::core::resources::names::{sound, sprite};
use crate::core::resources::sprites::Sprites;
use crate::core::config::{self, Eggman, GameplayConfig};
use crate::core::events::{SimEvent, EFFECT_SPEED};
use crate::core::objects::ids::ObjectId;
use crate::core::world::World;

// Eggman's own animation states (obj_egg Step_2).
pub const EGG_DJUMP: usize = BALANCING + 1;
pub const EGG_ZIPLINE: usize = BALANCING + 2;

impl Player {
    pub(super) fn egg_special(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let eggman = &cfg.eggman;
        if let Some(delay) = self.ability_delay_ticks(cfg) {
            self.djump_recharge = self.djump_recharge.max(delay);
            // A shield in use (negative timer) is not touched.
            if self.shield_recharge >= 0 && self.shield_recharge <= delay {
                self.shield_recharge = delay;
            }
            self.tracker_recharge = self.tracker_recharge.max(delay);
            self.is_attacking = false;
        }
        if self.controls_enabled && self.pressed(Buttons::A) && self.is_jumping && self.djump_recharge == 0 {
            self.is_jumping = false;
            self.yspd = eggman.double_jump_speed_per_tick;
            events.push(SimEvent::Sound { sound: sound::SND_EGG_DJUMP, x: self.x, y: self.y });
            // Negative timers count the ability in use; see count_down_egg_timers.
            self.djump_recharge = -1;
        }
        if self.controls_enabled && self.pressed(Buttons::B) && self.shield_recharge == 0 {
            self.is_attacking = true;
            events.push(SimEvent::Sound { sound: sound::SND_EGG_SHIELD, x: self.x, y: self.y });
            self.shield_recharge = -1;
        }
        self.try_place_tracker(world, eggman, cfg.physics.feet_below_position, events);
        self.count_down_egg_timers(eggman);
    }

    /// Down + C while standing on flat floor in front of Eggman drops a tracker.
    /// `feet`: the tracker check point and the tracker itself sit at the player's feet.
    fn try_place_tracker(&mut self, world: &World, eggman: &Eggman, feet: f64, events: &mut Vec<SimEvent>) {
        let ahead = if self.image_xscale > 0.0 { eggman.tracker_floor_check_ahead } else { -eggman.tracker_floor_check_ahead };
        self.is_colliding = self.is_grounded
            && !self.meeting_at(world, self.x, self.y, ObjectId::Angleanuller)
            && world.position_meeting(self.x + ahead, self.y + feet, ObjectId::FloorParent);
        let wants_tracker = self.controls_enabled && self.pressed(Buttons::C) && self.tracker_recharge <= 0;
        if !(wants_tracker && self.is_looking_down && self.is_colliding) {
            return;
        }
        events.push(SimEvent::Sound { sound: sound::SND_EGG_TRACKER, x: self.x, y: self.y });
        events.push(SimEvent::SpawnEggTracker { x: self.x, y: self.y + feet });
        self.tracker_recharge = config::ticks(eggman.tracker_recharge_seconds);
    }

    fn count_down_egg_timers(&mut self, eggman: &Eggman) {
        if self.shield_recharge < 0 {
            self.shield_recharge -= 1;
            if self.shield_recharge <= -config::ticks(eggman.shield_duration_seconds) {
                self.is_attacking = false;
                self.shield_recharge = self.egg_shield_recharge(eggman);
            }
        }
        if self.djump_recharge < 0 {
            self.djump_recharge -= 1;
            if self.djump_recharge <= -config::ticks(eggman.jetpack_pose_seconds) {
                self.djump_recharge = config::ticks(eggman.double_jump_recharge_seconds);
            }
        }
        if self.djump_recharge > 0 {
            self.djump_recharge -= 1;
        }
        if self.shield_recharge > 0 {
            self.shield_recharge -= 1;
        }
        if self.tracker_recharge > 0 {
            self.tracker_recharge -= 1;
        }
    }

    fn egg_shield_recharge(&self, eggman: &Eggman) -> i32 {
        let seconds = if self.is_demonized() { eggman.demonized_shield_recharge_seconds } else { eggman.shield_recharge_seconds };
        config::ticks(seconds)
    }

    /// Speed monitor boost in scr_move_basic: an active shield ends at once.
    pub(super) fn egg_boost_reset(&mut self, eggman: &Eggman) {
        if self.shield_recharge < 0 {
            self.shield_recharge = -config::ticks(eggman.shield_duration_seconds);
        }
        self.is_attacking = false;
    }

    /// obj_egg Step_2 checks before the shared ones.
    pub(super) fn egg_state(&mut self) -> Option<usize> {
        if self.is_hurt {
            return Some(HURT);
        }
        if self.is_zipline {
            if !self.is_grounded {
                return Some(EGG_ZIPLINE);
            }
            self.is_zipline = false;
        }
        None
    }

    /// obj_egg Step_2 air branch: falling right after a double jump shows the jetpack.
    pub(super) fn egg_air_state(&self, eggman: &Eggman, events: &mut Vec<SimEvent>) -> usize {
        if self.is_jumping {
            return JUMP;
        }
        if self.djump_recharge >= 0 {
            return FALL;
        }
        if self.y.floor() as i64 % eggman.animation.jetpack_puff_every_pixels == 0 {
            events.push(SimEvent::Effect { sprite: sprite::SPR_EGGPACK, x: self.x, y: self.y, xscale: 1.0, image_speed: EFFECT_SPEED, yspd: -1.0 });
        }
        EGG_DJUMP
    }

    /// obj_egg Draw_76 for states that differ from the shared ones.
    pub(super) fn egg_animate_state(&mut self, sprites: &Sprites, eggman: &Eggman) -> bool {
        let animation = &eggman.animation;
        let last_frame = sprites.get(self.sprite_index).frame_count as f64 - 1.0;
        match self.state {
            // Unlike the others Eggman's hurt animation plays.
            HURT => self.image_speed = animation.hurt_speed,
            WALK => self.image_speed = animation.walk_base_speed + self.xspd.abs() / self.max_h_speed,
            FALL => self.image_index = 0.0,
            JUMP => self.image_index = if self.yspd > 0.0 { 1.0 } else { 0.0 },
            LOOKUP | LOOKDOWN => {
                self.image_speed = animation.look_speed;
                if self.image_index >= last_frame {
                    self.image_index = last_frame;
                }
            }
            _ => return false,
        }
        true
    }

    /// obj_egg Other_7: idle loops over its last frames.
    pub(super) fn egg_animation_end(&mut self, eggman: &Eggman, frame_count: f64) {
        if self.state == IDLE {
            self.image_index = frame_count - eggman.animation.idle_loop_last_frames;
        }
    }
}
