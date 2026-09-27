//! obj_amy + scr_amy_special: hammer attack and the hammer (big) jump.

use super::animation::{BALANCING, FALL, HURT, IDLE, JUMP, LOOKDOWN, LOOKUP, WALK};
use super::{Buttons, Player};
use crate::core::resources::names::{sound, sprite};
use crate::core::resources::sprites::Sprites;
use crate::core::config::{self, Amy, GameplayConfig};
use crate::core::events::{SimEvent, EFFECT_SPEED};

// Amy's own animation states (obj_amy Step_2).
pub const AMY_ATTACK: usize = BALANCING + 1;
pub const AMY_HJUMP: usize = BALANCING + 2;
pub const AMY_ZIPLINE: usize = BALANCING + 4;

impl Player {
    pub(super) fn amy_special(&mut self, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let amy = &cfg.amy;
        if let Some(delay) = self.ability_delay_ticks(cfg) {
            self.attack_timer = self.attack_timer.max(delay);
            self.hjump_timer = self.hjump_timer.max(delay);
            self.is_attacking = false;
        }
        self.try_big_jump(amy, events);
        self.try_hammer(amy, events);
        if self.is_attacking {
            self.gspd = 0.0;
        }
        if self.state == AMY_HJUMP && self.is_grounded {
            self.state = IDLE;
            self.is_attacking = false;
        }
        // The big jump only recharges on the ground.
        if self.hjump_timer > 0 && self.is_grounded {
            self.hjump_timer -= 1;
        }
        if self.attack_timer > 0 {
            self.attack_timer -= 1;
        }
    }

    /// Down + jump: a much higher jump that also hits on its first frames.
    fn try_big_jump(&mut self, amy: &Amy, events: &mut Vec<SimEvent>) {
        if !(self.controls_enabled && self.is_looking_down && self.pressed(Buttons::A) && self.hjump_timer <= 0) {
            return;
        }
        self.is_grounded = false;
        self.is_spinning = false;
        self.is_jumping = true;
        self.just_jumped = true;
        self.xspd -= amy.big_jump_force_per_tick * self.angle.sin();
        self.yspd = -amy.big_jump_force_per_tick * self.angle.cos();
        self.is_hj = true;
        self.hjump_timer = config::ticks(amy.big_jump_recharge_seconds);
        events.push(SimEvent::Sound { sound: sound::SND_JUMP, x: self.x, y: self.y });
    }

    fn try_hammer(&mut self, amy: &Amy, events: &mut Vec<SimEvent>) {
        if !(self.controls_enabled && self.pressed(Buttons::B) && self.attack_timer <= 0) {
            return;
        }
        // Raw keyboard_check in the original.
        if self.buttons.held(Buttons::LEFT) {
            self.image_xscale = -1.0;
        } else if self.buttons.held(Buttons::RIGHT) {
            self.image_xscale = 1.0;
        }
        let heart = if self.is_demonized() { sprite::SPR_EROSEHEART } else { sprite::SPR_ROSEHEART };
        for index in 0..amy.hearts {
            // The original adds irandom_range(-3, 3) to y; that is presentation only,
            // so the client adds the jitter and the simulation stays deterministic.
            events.push(SimEvent::Effect {
                sprite: heart,
                x: self.x + amy.heart_start_x + index as f64 * amy.heart_spacing * self.image_xscale,
                y: self.y + amy.heart_start_y,
                xscale: 1.0,
                image_speed: EFFECT_SPEED,
                yspd: -1.0,
            });
        }
        self.image_index = 0.0;
        self.attack_timer = self.amy_attack_recharge(amy);
        self.is_attacking = true;
        events.push(SimEvent::Sound { sound: sound::SND_DASH, x: self.x, y: self.y });
    }

    fn amy_attack_recharge(&self, amy: &Amy) -> i32 {
        let seconds = if self.is_demonized() { amy.demonized_attack_recharge_seconds } else { amy.attack_recharge_seconds };
        config::ticks(seconds)
    }

    /// Spring resets from scr_collision_objects_after.
    pub(super) fn amy_spring_reset(&mut self, amy: &Amy) {
        if self.is_attacking {
            self.attack_timer = self.amy_attack_recharge(amy);
            self.is_attacking = false;
        }
        // Overwritten by the shared reset right after, exactly as in the original.
        self.is_jumping = true;
    }

    /// obj_amy Step_2 checks before the shared ones.
    pub(super) fn amy_state(&mut self) -> Option<usize> {
        if self.is_hj {
            return Some(AMY_HJUMP);
        }
        if self.is_attacking {
            return Some(AMY_ATTACK);
        }
        if self.is_zipline {
            if !self.is_grounded {
                return Some(AMY_ZIPLINE);
            }
            self.is_zipline = false;
        }
        if self.is_hurt {
            return Some(HURT);
        }
        None
    }

    /// obj_amy Draw_76 for states that differ from the shared ones.
    pub(super) fn amy_animate_state(&mut self, sprites: &Sprites, amy: &Amy) -> bool {
        let animation = &amy.animation;
        let last_frame = sprites.get(self.sprite_index).frame_count as f64 - 1.0;
        match self.state {
            WALK => self.image_speed = animation.walk_base_speed + self.xspd.abs() / self.max_h_speed,
            FALL => self.image_index = 0.0,
            // The jump pose plays once; after it Amy counts as falling, not jumping.
            JUMP => {
                self.image_speed = animation.jump_speed;
                if self.image_index >= last_frame {
                    self.is_jumping = false;
                }
            }
            LOOKUP | LOOKDOWN => self.animate_look(animation.look_speed),
            AMY_ATTACK => {
                self.image_speed = animation.attack_speed;
                if self.image_index >= last_frame {
                    self.is_attacking = false;
                }
            }
            AMY_HJUMP => {
                self.image_speed = animation.big_jump_speed;
                if self.image_index >= last_frame {
                    self.is_hj = false;
                }
            }
            _ => return false,
        }
        true
    }

    /// obj_amy Other_7: idle loops over its last frames.
    pub(super) fn amy_animation_end(&mut self, amy: &Amy, frame_count: f64) {
        if self.state == IDLE {
            self.image_index = frame_count - amy.animation.idle_loop_last_frames;
        }
    }
}
