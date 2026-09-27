//! obj_knux + scr_knux_special: punch dash, gliding and sticking to walls.

use super::animation::{BALANCING, HURT, IDLE, LOOKDOWN, LOOKUP, WALK};
use super::{Buttons, Player};
use crate::core::resources::names::{sound, sprite};
use crate::core::resources::sprites::Sprites;
use crate::core::config::{self, step, GameplayConfig, Knuckles};
use crate::core::events::SimEvent;
use crate::core::rooms::ids::RoomId;
use crate::core::world::World;

// Knuckles' own animation states (obj_knux Step_2).
pub const KNUX_ATTACK: usize = BALANCING + 1;
pub const KNUX_AIR_ATTACK: usize = BALANCING + 2;
pub const KNUX_GLIDE: usize = BALANCING + 3;
pub const KNUX_ZIPLINE: usize = BALANCING + 4;
pub const KNUX_STUCK: usize = BALANCING + 5;

impl Player {
    pub(super) fn knux_special(&mut self, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let knuckles = &cfg.knuckles;
        if let Some(delay) = self.ability_delay_ticks(cfg) {
            self.attack_timer = self.attack_timer.max(delay);
            self.glide_timer = self.glide_timer.max(delay);
            self.is_attacking = false;
        }
        self.try_start_glide(knuckles);
        self.try_punch(knuckles, events);
        if self.is_attacking {
            self.punch_dash(knuckles);
        }
        if self.is_grounded || self.is_hurt {
            self.is_gliding = false;
            self.unstick();
        }
        if self.is_gliding {
            self.glide(knuckles);
        } else {
            self.unstick();
        }
        if self.attack_timer > 0 {
            self.attack_timer -= 1;
        }
        if self.glide_timer > 0 && !self.is_gliding {
            self.glide_timer -= 1;
        }
    }

    fn try_start_glide(&mut self, knuckles: &Knuckles) {
        if !(self.controls_enabled && self.pressed(Buttons::A) && !self.is_grounded && !self.is_gliding && self.glide_timer <= 0) {
            return;
        }
        self.is_attacking = false;
        self.is_jumping = false;
        self.is_spinning = false;
        self.is_gliding = true;
        self.glide_timer = config::ticks(knuckles.glide_recharge_seconds);
        self.glide_xspd = self.image_xscale * knuckles.glide_speed_per_tick;
    }

    fn try_punch(&mut self, knuckles: &Knuckles, events: &mut Vec<SimEvent>) {
        if !(self.controls_enabled && self.pressed(Buttons::B) && self.attack_timer <= 0) {
            return;
        }
        // Raw keyboard_check in the original.
        if self.buttons.held(Buttons::LEFT) {
            self.image_xscale = -1.0;
        } else if self.buttons.held(Buttons::RIGHT) {
            self.image_xscale = 1.0;
        }
        self.image_index = 0.0;
        self.attack_timer = self.knux_attack_recharge(knuckles);
        self.is_attacking = true;
        events.push(SimEvent::Sound { sound: sound::SND_DASH, x: self.x, y: self.y });
    }

    fn knux_attack_recharge(&self, knuckles: &Knuckles) -> i32 {
        let seconds = if self.is_demonized() { knuckles.demonized_attack_recharge_seconds } else { knuckles.attack_recharge_seconds };
        config::ticks(seconds)
    }

    fn punch_dash(&mut self, knuckles: &Knuckles) {
        if self.is_gliding {
            self.is_gliding = false;
            self.unstick();
        }
        self.gspd = 0.0;
        let dash = if self.is_grounded { knuckles.punch_dash_ground_per_tick } else { knuckles.punch_dash_air_per_tick };
        self.xspd = self.image_xscale * dash;
    }

    /// Letting go of a wall gives movement back.
    pub(super) fn unstick(&mut self) {
        if self.is_stuck {
            self.can_move = true;
            self.is_stuck = false;
        }
    }

    fn glide(&mut self, knuckles: &Knuckles) {
        self.is_attacking = false;
        self.glide_timeout += 1;
        if self.glide_timeout >= config::ticks(knuckles.max_glide_seconds) {
            self.glide_timeout = 0;
            self.is_gliding = false;
            self.unstick();
        }

        if self.controls_enabled && self.buttons.held(Buttons::A) {
            if !self.is_stuck {
                self.steer_glide(knuckles);
            }
        } else {
            self.is_gliding = false;
            self.unstick();
        }

        self.image_xscale = 1.0;
        self.yspd = knuckles.glide_fall_per_tick;
        self.xspd = self.glide_xspd;
    }

    /// Turning while gliding; the frame shows the turn (0 = full right ... 4 = full left).
    fn steer_glide(&mut self, knuckles: &Knuckles) {
        let top_speed = knuckles.glide_speed_per_tick;
        let turn = self.acc * knuckles.glide_turn_multiplier * step();
        if self.held(Buttons::LEFT) {
            if self.glide_xspd > -top_speed {
                self.glide_xspd -= turn;
            }
        } else if self.held(Buttons::RIGHT) && self.glide_xspd < top_speed {
            self.glide_xspd += turn;
        }
        // The sprite has five turn frames; these speed bands pick one (as in the original).
        match self.glide_xspd.floor() as i32 {
            2..=5 => self.image_index = 0.0,
            1 => self.image_index = 1.0,
            0 => self.image_index = 2.0,
            -1 => self.image_index = 3.0,
            -5..=-2 => self.image_index = 4.0,
            _ => {}
        }
    }

    /// scr_collision_check: a glide that hits a wall that allows it sticks Knuckles to it.
    pub(super) fn knux_touch_wall(&mut self, world: &World, wall: Option<usize>, wall_is_left: bool, events: &mut Vec<SimEvent>) {
        if !(self.is_gliding && self.glide_timer > 0) {
            return;
        }
        let wall_allows_sticking = wall.is_some_and(|id| world.instances[id].can_stuck);
        if wall_allows_sticking && !self.is_stuck && !self.is_hurt {
            events.push(SimEvent::Sound { sound: sound::SND_SNAP, x: self.x, y: self.y });
            self.is_stuck = true;
        }
        if self.is_stuck && !self.is_hurt {
            self.image_xscale = if wall_is_left { -1.0 } else { 1.0 };
            self.yspd = 0.0;
        }
    }

    /// Spring resets from scr_collision_objects_after.
    pub(super) fn knux_spring_reset(&mut self, knuckles: &Knuckles) {
        if self.is_gliding {
            self.glide_timer = config::ticks(knuckles.glide_recharge_seconds);
            self.is_gliding = false;
            self.unstick();
        }
        if self.is_attacking {
            self.attack_timer = self.knux_attack_recharge(knuckles);
            self.is_attacking = false;
        }
    }

    /// obj_knux Step_2 checks before the shared ones.
    pub(super) fn knux_state(&mut self) -> Option<usize> {
        if self.is_attacking {
            // The original names are swapped: grounded uses KNUX_AIR_ATTACK. Kept as is.
            return Some(if self.is_grounded { KNUX_AIR_ATTACK } else { KNUX_ATTACK });
        }
        if self.is_zipline {
            if !self.is_grounded {
                return Some(KNUX_ZIPLINE);
            }
            self.is_zipline = false;
        }
        if self.is_hurt {
            return Some(HURT);
        }
        if self.is_stuck {
            return Some(KNUX_STUCK);
        }
        if self.is_gliding {
            return Some(KNUX_GLIDE);
        }
        None
    }

    /// obj_knux Draw_76 for states that differ from the shared ones.
    pub(super) fn knux_animate_state(&mut self, sprites: &Sprites, knuckles: &Knuckles, room: Option<RoomId>) -> bool {
        if !self.is_demonized() && self.state == IDLE && room == Some(RoomId::Marijuna) {
            self.sprite_index = sprite::SPR_KNUX_MARIJUNA;
        }
        let animation = &knuckles.animation;
        let last_frame = sprites.get(self.sprite_index).frame_count as f64 - 1.0;
        match self.state {
            WALK => self.image_speed = animation.walk_base_speed + self.xspd.abs() / self.max_h_speed,
            LOOKUP | LOOKDOWN => self.animate_look(animation.look_speed),
            KNUX_ATTACK => {
                self.image_speed = animation.ground_punch_speed;
                if self.image_index >= last_frame {
                    self.is_attacking = false;
                }
            }
            KNUX_AIR_ATTACK => {
                self.image_speed = animation.air_punch_speed;
                if self.image_index >= last_frame {
                    self.is_attacking = false;
                    self.is_grounded = false;
                }
            }
            KNUX_GLIDE => {
                self.image_speed = 0.0;
                self.image_xscale = 1.0;
            }
            _ => return false,
        }
        true
    }

    /// obj_knux Other_7: idle and balancing loop over their last frames.
    pub(super) fn knux_animation_end(&mut self, knuckles: &Knuckles, frame_count: f64) {
        if self.state == IDLE || self.state == BALANCING {
            self.image_index = frame_count - knuckles.animation.idle_loop_last_frames;
        }
    }
}
