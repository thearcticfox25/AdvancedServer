//! obj_cream + scr_cream_special: short flight, speed dash, spawning rings.

use super::animation::{BALANCING, HURT, IDLE, LOOKDOWN, LOOKUP, WALK};
use super::{Buttons, Player};
use crate::core::resources::names::sound;
use crate::core::resources::sprites::Sprites;
use crate::core::config::{self, Cream, GameplayConfig};
use crate::core::events::SimEvent;
use crate::core::objects::ids::ObjectId;
use crate::core::world::World;

// Cream's own animation states (obj_cream Step_2).
pub const CREAM_FLY: usize = BALANCING + 1;
pub const CREAM_SPAWN: usize = BALANCING + 2;
pub const CREAM_ZIPLINE: usize = BALANCING + 3;

impl Player {
    pub(super) fn cream_special(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let cream = &cfg.cream;
        if let Some(delay) = self.ability_delay_ticks(cfg) {
            self.fly_timer = self.fly_timer.max(delay);
            self.dash_timer = self.dash_timer.max(delay);
            self.rings_timer = self.rings_timer.max(delay);
            self.rings_spawn = 0;
        }
        self.try_cream_fly(cream);
        self.try_dash(cream, events);
        self.is_colliding = self.rings_spot_blocked(world, cream) || (self.revival_times >= 2 && self.near_red_ring(world, cream));
        self.try_start_ring_spawn(cream);
        self.spawn_rings(world, cream, events);
        self.keep_cream_flying(cream);
        self.update_dash_speed(cream);
        if !self.is_flying && self.fly_timer > 0 {
            self.fly_timer -= 1;
        }
        if self.rings_timer > 0 {
            self.rings_timer -= 1;
        }
        if self.dash_timer > 0 {
            self.dash_timer -= 1;
        }
    }

    /// Jump again while still rising from a jump: a slow glide down.
    fn try_cream_fly(&mut self, cream: &Cream) {
        if !(self.controls_enabled && self.is_jumping && !self.just_jumped && self.pressed(Buttons::A) && self.fly_timer <= 0) {
            return;
        }
        self.fly_timeout = 0;
        self.is_flying = true;
        self.is_jumping = false;
        self.is_spinning = false;
        self.fly_timer = config::ticks(cream.fly_recharge_seconds);
    }

    fn try_dash(&mut self, cream: &Cream, events: &mut Vec<SimEvent>) {
        let ready = self.dash_timer <= 0 && self.xspd.abs() > 0.0 && self.rings_spawn <= 0;
        if !(self.controls_enabled && self.pressed(Buttons::B) && ready) {
            return;
        }
        self.dashing = config::ticks(cream.dash_seconds);
        self.effect_time = config::ticks(cream.dash_seconds);
        self.dash_timer = config::ticks(cream.dash_recharge_seconds);
        self.max_h_speed = cream.dash_max_speed_per_tick;
        events.push(SimEvent::Sound { sound: sound::SND_CREAMDASH, x: self.x, y: self.y });
    }

    /// Where the rings appear: a little in front of Cream, at chest height.
    fn rings_spot(&self, cream: &Cream) -> (f64, f64) {
        let flip_shift = if self.image_xscale < 0.0 { cream.rings_spot_flip_shift } else { 0.0 };
        (self.x + cream.rings_spot_x + flip_shift, self.y + cream.rings_spot_y)
    }

    /// Rings cannot be placed into terrain or on a ramp helper.
    fn rings_spot_blocked(&self, world: &World, cream: &Cream) -> bool {
        let (x, y) = self.rings_spot(cream);
        self.meeting_at(world, self.x, self.y, ObjectId::Angleanuller)
            || world.collision_circle_precise(x, y, cream.rings_spot_radius, ObjectId::FloorParent)
    }

    /// scr_cream_special: distance_to_object(obj_redring) < 130 for a demonized Cream.
    fn near_red_ring(&self, world: &World, cream: &Cream) -> bool {
        let body = crate::core::collision::sprite_bbox(world.sprites.get(self.sprite_index), self.x, self.y, self.image_xscale, 1.0, 0.0);
        let ring_sprite = world.sprites.get(crate::core::resources::names::sprite::SPR_REDRING);
        world.rings.iter().filter(|ring| ring.red).any(|ring| {
            let ring_box = crate::core::collision::sprite_bbox(ring_sprite, ring.x, ring.y, 1.0, 1.0, 0.0);
            body.distance(&ring_box) < cream.demonized_red_ring_distance
        })
    }

    fn try_start_ring_spawn(&mut self, cream: &Cream) {
        if !(self.controls_enabled && self.pressed(Buttons::C) && self.rings_timer <= 0 && self.is_grounded && !self.is_colliding) {
            return;
        }
        self.rings_timer = self.cream_rings_recharge(cream);
        self.rings_spawn = crate::core::config::original_ticks(cream.rings_spawn_ticks);
    }

    fn cream_rings_recharge(&self, cream: &Cream) -> i32 {
        let seconds = if self.is_demonized() { cream.demonized_rings_recharge_seconds } else { cream.rings_recharge_seconds };
        config::ticks(seconds)
    }

    /// Standing still while the rings appear; leaving the ground cancels (and still costs the recharge).
    fn spawn_rings(&mut self, world: &World, cream: &Cream, events: &mut Vec<SimEvent>) {
        if self.rings_spawn > 1 {
            if !self.is_grounded || !self.controls_enabled {
                self.rings_timer = self.cream_rings_recharge(cream);
                self.rings_spawn = 0;
            }
            self.gspd = 0.0;
            self.xspd = 0.0;
            self.yspd = 0.0;
            self.rings_spawn -= 1;
        } else if self.rings_spawn == 1 {
            let (x, y) = self.rings_spot(cream);
            if !world.collision_circle_precise(x, y, cream.rings_spot_radius, ObjectId::FloorParent) {
                events.push(SimEvent::Sound { sound: sound::SND_CREAMRING, x: self.x, y: self.y });
                events.push(SimEvent::SpawnCreamRings { x, y, demonized: self.is_demonized() });
            }
            self.rings_spawn = 0;
        }
    }

    fn keep_cream_flying(&mut self, cream: &Cream) {
        if !self.is_flying {
            return;
        }
        self.yspd = cream.fly_fall_per_tick;
        self.fly_timeout += 1;
        if self.fly_timeout > config::ticks(cream.max_fly_seconds) || self.is_grounded {
            self.is_flying = false;
        }
    }

    /// While dashing Cream accelerates hard. Otherwise her speeds are reset every tick,
    /// with her own 30 % slowdown: this overrides the 40 % of scr_player_slow one
    /// tick later, exactly as in the original.
    fn update_dash_speed(&mut self, cream: &Cream) {
        if self.dashing > 0 {
            self.is_jumping = false;
            self.acc = cream.dash_acceleration_per_tick;
            self.dashing -= 1;
            return;
        }
        let keep = if self.is_slow { (100.0 - cream.slow_percent) / 100.0 } else { 1.0 };
        self.max_h_speed = cream.movement.max_speed_per_tick * keep;
        self.acc = cream.movement.acceleration_per_tick * keep;
    }

    /// Resets shared by death, stun and hurt in scr_move_basic / obj_cream Step_0.
    pub(super) fn cream_stop_abilities(&mut self) {
        self.dashing = 0;
        self.rings_spawn = 0;
    }

    /// Spring resets from scr_collision_objects_after.
    pub(super) fn cream_spring_reset(&mut self, cream: &Cream) {
        if self.is_flying {
            self.fly_timer = config::ticks(cream.fly_recharge_seconds);
            self.is_flying = false;
        }
    }

    /// obj_cream Step_2 checks before the shared ones.
    pub(super) fn cream_state(&mut self) -> Option<usize> {
        if self.rings_spawn > 0 {
            return Some(CREAM_SPAWN);
        }
        if self.is_hurt {
            return Some(HURT);
        }
        if self.is_flying {
            return Some(CREAM_FLY);
        }
        if self.is_zipline {
            if !self.is_grounded {
                return Some(CREAM_ZIPLINE);
            }
            self.is_zipline = false;
        }
        None
    }

    /// obj_cream Draw_76 for states that differ from the shared ones.
    pub(super) fn cream_animate_state(&mut self, sprites: &Sprites, cream: &Cream) -> bool {
        let animation = &cream.animation;
        match self.state {
            WALK => self.image_speed = animation.walk_base_speed + self.xspd.abs() / self.max_h_speed,
            LOOKUP | LOOKDOWN => self.animate_look(animation.look_speed),
            CREAM_FLY => self.image_speed = animation.fly_speed,
            CREAM_SPAWN => {
                self.image_speed = animation.spawn_speed;
                let last_frame = sprites.get(self.sprite_index).frame_count as f64 - 1.0;
                if self.image_index >= last_frame {
                    self.image_index = last_frame;
                }
            }
            _ => return false,
        }
        true
    }

    /// obj_cream Other_7: idle loops over its last frames.
    pub(super) fn cream_animation_end(&mut self, cream: &Cream, frame_count: f64) {
        if self.state == IDLE {
            self.image_index = frame_count - cream.animation.idle_loop_last_frames;
        }
    }
}
