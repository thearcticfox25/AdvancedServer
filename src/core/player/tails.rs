//! obj_tails + scr_tails_special: flight and the charged shot.

use super::animation::{BALANCING, HURT, IDLE};
use super::{gm_sign, Buttons, Player};
use crate::core::resources::names::sound;
use crate::core::config::{self, step, GameplayConfig, Tails};
use crate::core::events::SimEvent;

// Tails' own animation states (obj_tails Step_2).
pub const TAILS_FLY: usize = BALANCING + 1;
pub const TAILS_ATTACK1: usize = BALANCING + 2;
pub const TAILS_ZIPLINE: usize = BALANCING + 3;

/// How strong a released shot is: index into the config's weak/medium/strong arrays.
fn charge_strength(tails: &Tails, charge_ticks: i32) -> usize {
    if charge_ticks >= config::ticks(tails.strong_charge_seconds) {
        2
    } else if charge_ticks >= config::ticks(tails.medium_charge_seconds) {
        1
    } else {
        0
    }
}

impl Player {
    pub(super) fn tails_special(&mut self, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let tails = &cfg.tails;
        let fly_ready = -config::ticks(tails.flight_recharge_seconds);
        if let Some(delay) = self.ability_delay_ticks(cfg) {
            self.attack_timer = self.attack_timer.max(delay);
            self.fly_timer = self.fly_timer.max(fly_ready + delay);
        }
        self.try_fly(tails, fly_ready, events);
        self.keep_flying(tails);
        if self.fly_timer > 0 && self.fly_timer % crate::core::config::original_ticks(tails.flight_sound_every_ticks).max(1) == 0 {
            events.push(SimEvent::Sound { sound: sound::SND_TAILS_FLY, x: self.x, y: self.y });
        }
        self.try_start_charge(tails, events);
        if self.attack_charge > 0 {
            self.charge_shot(tails, events);
        }
        self.recoil_after_shot(tails);
        if self.attack_timer > 0 {
            self.attack_timer -= 1;
        }
        self.count_down_flight(fly_ready);
    }

    fn try_fly(&mut self, tails: &Tails, fly_ready: i32, events: &mut Vec<SimEvent>) {
        let no_shot = self.attack_charge <= 0 && self.attack_after <= 0;
        if !(self.controls_enabled && !self.is_grounded && self.pressed(Buttons::A) && no_shot) {
            return;
        }
        if !self.is_flying && self.fly_timer <= fly_ready {
            events.push(SimEvent::Sound { sound: sound::SND_TAILS_FLY, x: self.x, y: self.y });
            self.fly_timer = config::ticks(tails.flight_duration_seconds);
            self.is_flying = true;
        }
        if self.is_flying {
            // Each jump press while flying gives an upward kick until the flight time runs out.
            if self.fly_timer > 0 {
                self.fly_grv = tails.flight_kick_per_tick;
            } else {
                self.is_flying = false;
            }
            self.is_jumping = false;
            if self.xspd.abs() > 0.0 {
                self.image_xscale = gm_sign(self.xspd);
            }
        }
    }

    fn keep_flying(&mut self, tails: &Tails) {
        if !self.is_flying {
            return;
        }
        self.is_spinning = false;
        if self.fly_grv < tails.flight_max_fall_per_tick {
            self.fly_grv += tails.flight_gravity_per_tick * step();
        }
        if self.fly_timer <= 0 {
            self.is_flying = false;
        }
        self.yspd = self.fly_grv;
    }

    fn try_start_charge(&mut self, tails: &Tails, events: &mut Vec<SimEvent>) {
        if !(self.controls_enabled && self.pressed(Buttons::B) && self.attack_timer == 0 && self.attack_charge == 0) {
            return;
        }
        events.push(SimEvent::Sound { sound: sound::SND_TAILS_CHARGE, x: self.x, y: self.y });
        self.attack_timer = self.tails_attack_recharge(tails);
        self.attack_charge = 1;
    }

    fn tails_attack_recharge(&self, tails: &Tails) -> i32 {
        let seconds = if self.is_demonized() { tails.demonized_attack_recharge_seconds } else { tails.attack_recharge_seconds };
        config::ticks(seconds)
    }

    /// Standing still while the shot charges; releasing B (or any interruption) fires.
    fn charge_shot(&mut self, tails: &Tails, events: &mut Vec<SimEvent>) {
        self.emotion = false;
        self.attack_timer = self.tails_attack_recharge(tails);
        self.attack_charge += 1;
        self.is_jumping = false;
        self.is_flying = false;
        self.xspd = 0.0;
        self.gspd = 0.0;
        self.can_move = false;

        // Raw keyboard_check in the original: turning works even without controls.
        if self.buttons.held(Buttons::LEFT) {
            self.image_xscale = -1.0;
        } else if self.buttons.held(Buttons::RIGHT) {
            self.image_xscale = 1.0;
        }
        self.attack_k_dir = self.image_xscale;

        let interrupted = self.is_zipline || self.is_hurt || self.is_dead || self.shocked_timer > 0;
        let fully_charged = self.attack_charge >= config::ticks(tails.max_charge_seconds);
        if fully_charged || !self.controls_enabled || interrupted || !self.buttons.held(Buttons::B) {
            self.fire_shot(tails, events);
        }
        if self.is_hurt || self.hp <= 0 || self.is_dead || self.shocked_timer > 0 || self.is_zipline {
            self.cancel_shot();
        }
    }

    fn fire_shot(&mut self, tails: &Tails, events: &mut Vec<SimEvent>) {
        events.push(SimEvent::StopSound { sound: sound::SND_TAILS_CHARGE });
        // The charge meter in percent of the full charge, then in 20 % steps. Same
        // operation order as the original: on f64 it decides the last bit and so the floor.
        let charge_percent = (self.attack_charge as f64 / config::ticks_per_second() / tails.max_charge_seconds * 100.0).floor();
        let charge_level = (charge_percent / 20.0).ceil() as i32;
        let strength = charge_strength(tails, self.attack_charge);
        let damage = if self.is_demonized() {
            // Demonized Tails hurts survivors.
            tails.demonized_damage[strength]
        } else {
            // Survivor Tails stuns EXE for this many seconds. GML divides as a real
            // number, but the value travels as u8 and arrives truncated, which
            // integer division reproduces.
            1 + self.attack_charge / tails.stun_charge_ticks_per_second
        };
        events.push(SimEvent::SpawnTailsProjectile {
            x: self.x,
            y: self.y,
            dir: self.image_xscale,
            damage,
            hurts_survivors: self.is_demonized(),
            charge: charge_level,
        });
        events.push(SimEvent::CameraShake);
        events.push(SimEvent::Sound { sound: sound::SND_TAILS_SHOOT, x: self.x, y: self.y });

        self.attack_after = config::ticks(tails.shot_pose_seconds[strength]);
        self.recoil = -self.attack_k_dir * tails.shot_recoil[strength];
    }

    fn cancel_shot(&mut self) {
        self.is_jumping = false;
        self.is_flying = false;
        self.is_spinning = false;
        self.can_move = true;
        self.attack_charge = 0;
        self.attack_after = 0;
    }

    /// Pushed backwards for a moment after shooting; no spin during it.
    fn recoil_after_shot(&mut self, tails: &Tails) {
        if self.attack_after <= 0 {
            self.can_spin = true;
            return;
        }
        if self.is_hurt || self.hp <= 0 || self.is_dead || self.shocked_timer > 0 || self.is_zipline {
            self.cancel_shot();
        }
        self.can_spin = false;
        self.attack_after -= 1;
        self.attack_charge = 0;
        self.is_jumping = false;
        self.is_flying = false;
        self.is_spinning = false;
        self.can_move = true;
        self.xspd = self.recoil;
        self.gspd = self.recoil;
        self.recoil -= self.recoil.abs().min(self.acc * tails.recoil_friction_multiplier * step()) * gm_sign(self.recoil);
    }

    fn count_down_flight(&mut self, fly_ready: i32) {
        if self.is_grounded {
            if self.is_flying {
                self.is_flying = false;
                self.fly_timer = 0;
            }
            if self.fly_timer > fly_ready {
                self.fly_timer -= 1;
            }
        } else if self.fly_timer > 0 {
            self.fly_timer -= 1;
        }
    }

    /// obj_tails Step_2 checks before the shared ones.
    pub(super) fn tails_state(&mut self) -> Option<usize> {
        // Stopping snd_tails_fly once nobody flies is the client's (level/mod.rs).
        if self.attack_charge > 0 || self.attack_after > 0 {
            return Some(TAILS_ATTACK1);
        }
        if self.is_hurt {
            return Some(HURT);
        }
        if self.is_flying {
            return Some(TAILS_FLY);
        }
        if self.is_zipline {
            if !self.is_grounded {
                return Some(TAILS_ZIPLINE);
            }
            self.is_zipline = false;
        }
        None
    }

    /// obj_tails Draw_76 for states that differ from the shared ones.
    pub(super) fn tails_animate_state(&mut self, tails: &Tails) -> bool {
        match self.state {
            TAILS_ZIPLINE => {
                self.image_speed = 0.0;
                self.image_index = 0.0;
            }
            TAILS_FLY => self.image_speed = tails.animation.fly_speed,
            _ => return false,
        }
        true
    }

    /// obj_tails Other_7: idle keeps looping from a later frame after the first play.
    pub(super) fn tails_animation_end(&mut self, tails: &Tails) {
        if self.state == IDLE {
            self.image_index = tails.animation.idle_loop_frame;
        }
    }
}
