//! obj_sally + scr_sally_special: air kick, shield, slide.

use super::animation::{BALANCING, FALL, HURT, IDLE, JUMP, LOOKDOWN, LOOKUP, WALK};
use super::{gm_sign, Buttons, Player};
use crate::core::resources::names::{sound, sprite};
use crate::core::resources::sprites::Sprites;
use crate::core::config::{self, step, GameplayConfig, Sally};
use crate::core::events::{SimEvent, EFFECT_SPEED};
use crate::core::world::World;

// Sally's own animation states (obj_sally Step_2).
pub const SALLY_ATTACK: usize = BALANCING + 1;
pub const SALLY_SLIDE: usize = BALANCING + 2;
pub const SALLY_ZIPLINE: usize = BALANCING + 4;
/// Sally's Draw_76 also animates state 16 like an emotion.
const SALLY_EXTRA_EMOTION: usize = BALANCING + 3;

impl Player {
    pub(super) fn sally_special(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let sally = &cfg.sally;
        if let Some(delay) = self.ability_delay_ticks(cfg) {
            self.shield_recharge = self.shield_recharge.max(delay);
            self.attack_timer = self.attack_timer.max(delay);
            self.is_attacking = false;
        }
        self.try_kick(sally, events);
        self.try_shield(sally, events);
        self.try_slide(sally, events);
        if self.shield_recharge > 0 {
            self.shield_recharge -= 1;
        }
        if self.attack_timer > 0 {
            self.attack_timer -= 1;
        }
        self.count_down_shield(sally, events);
        if self.is_sliding {
            self.slide(world, sally, events);
        }
    }

    /// B in the air: a kick.
    fn try_kick(&mut self, sally: &Sally, events: &mut Vec<SimEvent>) {
        if !(self.controls_enabled && self.pressed(Buttons::B) && !self.is_grounded && self.attack_timer <= 0) {
            return;
        }
        events.push(SimEvent::Sound { sound: sound::SND_DASH, x: self.x, y: self.y });
        // Raw keyboard_check in the original.
        if self.buttons.held(Buttons::LEFT) {
            self.image_xscale = -1.0;
        } else if self.buttons.held(Buttons::RIGHT) {
            self.image_xscale = 1.0;
        }
        self.image_index = 0.0;
        self.image_speed = 0.0;
        self.is_jumping = false;
        self.is_attacking = true;
        self.attack_timer = self.sally_attack_recharge(sally);
    }

    fn sally_attack_recharge(&self, sally: &Sally) -> i32 {
        let seconds = if self.is_demonized() { sally.demonized_attack_recharge_seconds } else { sally.attack_recharge_seconds };
        config::ticks(seconds)
    }

    fn sally_shield_recharge(&self, sally: &Sally) -> i32 {
        let seconds = if self.is_demonized() { sally.demonized_shield_recharge_seconds } else { sally.shield_recharge_seconds };
        config::ticks(seconds)
    }

    fn try_shield(&mut self, sally: &Sally, events: &mut Vec<SimEvent>) {
        if !(self.controls_enabled && self.pressed(Buttons::C) && self.shield_recharge <= 0) {
            return;
        }
        events.push(SimEvent::Sound { sound: sound::SND_SALLY_SHIELD, x: self.x, y: self.y });
        self.shield_timer = config::ticks(sally.shield_seconds);
        self.shield_recharge = self.sally_shield_recharge(sally);
    }

    /// Down while running fast: slide with a small speed boost.
    fn try_slide(&mut self, sally: &Sally, events: &mut Vec<SimEvent>) {
        let fast_enough = self.xspd.abs() >= sally.slide_min_speed_per_tick;
        if !(self.controls_enabled && fast_enough && self.pressed(Buttons::DOWN) && self.is_grounded && !self.is_sliding) {
            return;
        }
        events.push(SimEvent::Sound { sound: sound::SND_SALLY_SLIDE, x: self.x, y: self.y });
        self.is_attacking = false;
        self.is_sliding = true;
        self.slide_speed = self.xspd + sally.slide_boost_per_tick * gm_sign(self.xspd);
    }

    /// While the shield is up its recharge stays full, so it only starts counting once the shield is gone.
    fn count_down_shield(&mut self, sally: &Sally, events: &mut Vec<SimEvent>) {
        if self.shield_timer <= 0 {
            return;
        }
        self.shield_timer -= 1;
        if self.shield_timer <= 0 && self.is_demonized() {
            self.show_shield_break(events);
        }
        self.shield_recharge = self.sally_shield_recharge(sally);
    }

    pub fn show_shield_break(&self, events: &mut Vec<SimEvent>) {
        let effect = if self.is_demonized() { sprite::SPR_SHIELDBREAK2 } else { sprite::SPR_SHIELDBREAK };
        events.push(SimEvent::Sound { sound: sound::SND_SALLY_SHIELDBREAK, x: self.x, y: self.y });
        events.push(SimEvent::Effect { sprite: effect, x: self.x, y: self.y, xscale: 1.0, image_speed: 1.0, yspd: 0.0 });
    }

    fn slide(&mut self, world: &World, sally: &Sally, events: &mut Vec<SimEvent>) {
        let margin = sally.slide_room_edge_margin;
        let at_room_edge = self.x <= margin || self.x >= world.room_width - margin;
        if self.shocked_timer > 0 || !self.is_grounded || at_room_edge || self.slide_speed.abs() <= 0.0 {
            self.is_sliding = false;
            return;
        }
        self.slide_speed -= self.slide_speed.abs().min(self.acc * sally.slide_friction_multiplier * step()) * gm_sign(self.slide_speed) * self.angle.cos();

        if self.x.floor() as i64 % sally.slide_dust_every_pixels == 0 {
            events.push(SimEvent::Effect {
                sprite: sprite::SPR_DUST,
                x: self.x - sally.slide_dust_offset_x * gm_sign(self.slide_speed),
                y: self.y + sally.slide_dust_offset_y,
                xscale: 1.0,
                image_speed: EFFECT_SPEED,
                yspd: 0.0,
            });
            // Local sound only in the original (no net_sound_emit).
            events.push(SimEvent::LocalSound { sound: sound::SND_SALLY_SLIDE });
        }
        self.gspd = 0.0;
        self.xspd = self.slide_speed * self.angle.cos();
        self.yspd = self.slide_speed * -self.angle.sin();
    }

    /// Resets for death (scr_move_basic): attack, slide and shield end.
    pub(super) fn sally_stop_on_death(&mut self) {
        self.is_attacking = false;
        self.is_sliding = false;
        if self.shield_timer > 0 {
            self.shield_timer = 0;
        }
    }

    /// Spring resets from scr_collision_objects_after.
    pub(super) fn sally_spring_reset(&mut self, sally: &Sally) {
        if self.is_attacking {
            self.attack_timer = self.sally_attack_recharge(sally);
            self.is_attacking = false;
        }
        if self.is_sliding {
            self.slide_speed = 0.0;
            self.is_sliding = false;
        }
        // Overwritten by the shared reset right after, exactly as in the original.
        self.is_jumping = true;
    }

    /// scr_player_hurt: an active shield takes the hit instead of Sally.
    /// Returns true when the hit was absorbed.
    pub(super) fn sally_shield_absorbs(&mut self, cfg: &GameplayConfig, damage: i32, events: &mut Vec<SimEvent>) -> bool {
        if damage <= 0 || self.shield_timer <= 0 {
            return false;
        }
        events.push(SimEvent::ShieldBlocked { on_last_hit: !self.is_demonized() && self.hp <= damage });
        self.hurttime = config::ticks(cfg.sally.shield_block_invincibility_seconds);
        self.shield_timer = 0;
        self.show_shield_break(events);
        true
    }

    /// obj_sally Step_2 checks before the shared ones.
    pub(super) fn sally_state(&mut self) -> Option<usize> {
        if self.is_hurt {
            return Some(HURT);
        }
        if self.is_sliding {
            return Some(SALLY_SLIDE);
        }
        if self.is_attacking {
            return Some(SALLY_ATTACK);
        }
        if self.is_zipline {
            if !self.is_grounded {
                return Some(SALLY_ZIPLINE);
            }
            self.is_zipline = false;
        }
        None
    }

    /// obj_sally Draw_76 for states that differ from the shared ones.
    pub(super) fn sally_animate_state(&mut self, sprites: &Sprites, sally: &Sally) -> bool {
        let animation = &sally.animation;
        let last_frame = sprites.get(self.sprite_index).frame_count as f64 - 1.0;
        match self.state {
            HURT => self.image_speed = animation.hurt_speed,
            WALK => self.image_speed = animation.walk_base_speed + self.xspd.abs() / self.max_h_speed,
            FALL => self.image_index = 0.0,
            // The jump pose holds while rising and plays once while falling.
            JUMP => {
                if self.yspd > 0.0 {
                    self.image_speed = animation.jump_fall_speed;
                    if self.image_index >= last_frame {
                        self.image_index = last_frame;
                    }
                } else {
                    self.image_speed = 0.0;
                }
            }
            LOOKUP | LOOKDOWN => {
                self.image_speed = animation.look_speed;
                if self.image_index >= last_frame {
                    self.image_index = last_frame;
                }
            }
            SALLY_SLIDE => self.image_speed = 0.0,
            // The kick lasts as long as its animation.
            SALLY_ATTACK => {
                self.image_speed = animation.attack_speed;
                if self.image_index >= last_frame {
                    self.is_attacking = false;
                    self.image_index = last_frame;
                }
            }
            SALLY_EXTRA_EMOTION => self.image_speed = 1.0,
            _ => return false,
        }
        true
    }

    /// obj_sally Other_7: idle loops over its last frames.
    pub(super) fn sally_animation_end(&mut self, sally: &Sally, frame_count: f64) {
        if self.state == IDLE {
            self.image_index = frame_count - sally.animation.idle_loop_last_frames;
        }
    }
}
