//! obj_exeller + scr_exeller_special: dash attacks and clones to teleport to.

use super::animation::{FALL, HURT, IDLE};
use super::exe::{EXE_AIR_ATTACK, EXE_ATTACK, EXE_LOST, EXE_SHOCKED, EXE_WON, EXE_ZIPLINE};
use super::{gm_sign, Buttons, Player};
use crate::core::resources::names::{sound, sprite};
use crate::core::resources::sprites::Sprites;
use crate::core::config::{self, Exeller, GameplayConfig};
use crate::core::events::{SimEvent, EFFECT_SPEED};

// Exeller's own animation state (obj_exeller Step_2); the others share EXE's numbers.
pub const EXELLER_LOST2: usize = EXE_ZIPLINE + 1;

/// Which clone a teleport goes to: the first one with up, the second with down.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum CloneSlot {
    Up = 0,
    Down = 1,
}

impl Player {
    pub(super) fn exeller_special(&mut self, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let exeller = &cfg.exeller;
        if self.won || self.lost {
            self.is_attacking = false;
            self.attack_timer = 0;
        }
        // Pressing jump again in the air clears the jumping flag (no double jump).
        if self.controls_enabled && self.is_jumping && self.pressed(Buttons::A) {
            self.is_jumping = false;
        }
        if self.clone_timer > 0 {
            self.clone_timer -= 1;
        }
        if self.controls_enabled && self.pressed(Buttons::C) && self.use_clones(exeller, events) {
            // A teleport request ends the special script for this tick, as in the original.
            return;
        }
        self.try_exeller_dash(exeller, events);
        if self.is_attacking {
            self.keep_exeller_dashing(exeller, events);
        }
        if self.attack_timer > 0 {
            self.attack_timer -= 1;
        }
    }

    /// C with up or down teleports to that clone; C alone places a new clone.
    /// Returns true when the script stops here (a teleport was requested).
    fn use_clones(&mut self, exeller: &Exeller, events: &mut Vec<SimEvent>) -> bool {
        // Raw keyboard_check in the original.
        let up = self.buttons.held(Buttons::UP);
        let down = self.buttons.held(Buttons::DOWN);
        if up && self.clones[CloneSlot::Up as usize] != NO_CLONE {
            events.push(SimEvent::TeleportToClone { slot: CloneSlot::Up });
            return true;
        }
        if down && self.clones[CloneSlot::Down as usize] != NO_CLONE {
            events.push(SimEvent::TeleportToClone { slot: CloneSlot::Down });
            return true;
        }
        if !up && !down && self.clone_count < exeller.max_clones && self.clone_timer <= 0 {
            // The voice line (random, skipped during taunts) is the client's business.
            events.push(SimEvent::SpawnExellerClone { x: self.x, y: self.y, dir: self.image_xscale });
            self.clone_timer = config::ticks(exeller.clone_place_cooldown_seconds);
        }
        false
    }

    fn try_exeller_dash(&mut self, exeller: &Exeller, events: &mut Vec<SimEvent>) {
        if !(self.controls_enabled && self.pressed(Buttons::B) && self.attack_timer <= 0 && self.invis_timer <= 0) {
            return;
        }
        // Raw keyboard_check in the original.
        if self.buttons.held(Buttons::LEFT) {
            self.image_xscale = -1.0;
        } else if self.buttons.held(Buttons::RIGHT) {
            self.image_xscale = 1.0;
        }
        self.image_index = 0.0;
        let recharge = if self.is_grounded { exeller.ground_attack_recharge_seconds } else { exeller.air_attack_recharge_seconds };
        self.attack_timer = config::ticks(recharge);
        self.is_attacking = true;
        self.is_jumping = false;
        self.is_spinning = false;
        events.push(SimEvent::Sound { sound: sound::SND_DASH, x: self.x, y: self.y });
    }

    fn keep_exeller_dashing(&mut self, exeller: &Exeller, events: &mut Vec<SimEvent>) {
        if self.yspd > 0.0 {
            self.yspd = if self.is_grounded { 0.0 } else { exeller.air_attack_fall_per_tick };
        }
        if self.is_grounded && self.xspd.abs() > 0.0 && self.x.floor() as i64 % exeller.dust_every_pixels == 0 {
            events.push(SimEvent::Effect {
                sprite: sprite::SPR_DUST,
                x: self.x - exeller.dust_offset_x * gm_sign(self.image_xscale),
                y: self.y + exeller.dust_offset_y,
                xscale: 1.0,
                image_speed: EFFECT_SPEED,
                yspd: 0.0,
            });
        }
    }

    /// SERVER_EXELLERCLONE_STATE, spawn: the new clone takes the first free slot.
    pub fn exeller_clone_placed(&mut self, clone_id: i32) {
        let slot = if self.clones[CloneSlot::Up as usize] != NO_CLONE { CloneSlot::Down } else { CloneSlot::Up };
        self.clones[slot as usize] = clone_id;
        self.clone_count += 1;
    }

    /// SERVER_EXELLERCLONE_STATE, teleport: Exeller jumps to the clone, which disappears.
    pub fn exeller_teleported(&mut self, cfg: &GameplayConfig, slot: CloneSlot, x: f64, y: f64, events: &mut Vec<SimEvent>) {
        self.x = x;
        self.y = y;
        self.clones[slot as usize] = NO_CLONE;
        self.clone_timer = config::ticks(cfg.exeller.clone_recharge_seconds);
        self.clone_count -= 1;
        events.push(SimEvent::Sound { sound: sound::SND_EXE_APPEAR, x, y });
        events.push(SimEvent::Effect { sprite: sprite::SPR_RING_TELEPORT, x, y: y + cfg.exeller.clone_effect_offset_y, xscale: 1.0, image_speed: 2.0, yspd: 0.0 });
    }

    /// obj_exeller Step_2 checks before the shared ones.
    pub(super) fn exeller_state(&mut self) -> Option<usize> {
        if self.won && self.is_grounded {
            return Some(EXE_WON);
        }
        if self.lost && self.is_grounded {
            return Some(self.lost_state);
        }
        if self.is_hurt {
            return Some(HURT);
        }
        if self.shocked_timer > 0 {
            return Some(if self.is_grounded { EXE_SHOCKED } else { HURT });
        }
        if self.is_attacking {
            // Same check as obj_exe: it compares with the state already reset to IDLE,
            // so it only fires during an emotion; kept exactly as written.
            let (current, other) = if self.is_grounded { (EXE_ATTACK, EXE_AIR_ATTACK) } else { (EXE_AIR_ATTACK, EXE_ATTACK) };
            if self.state == other {
                self.is_attacking = false;
                return Some(self.state);
            }
            return Some(current);
        }
        if self.is_zipline {
            if self.is_grounded {
                self.is_zipline = false;
            }
            return Some(EXE_ZIPLINE);
        }
        None
    }

    /// obj_exeller Draw_76 for states that differ from the shared ones.
    pub(super) fn exeller_animate_state(&mut self, sprites: &Sprites, exeller: &Exeller) -> bool {
        match self.state {
            FALL => self.animate_fall_by_direction(exeller.animation.fall_speed),
            EXELLER_LOST2 => self.image_speed = exeller.animation.end_pose_speed,
            _ => return self.animate_killer_state(sprites, &exeller.animation, exeller.dash_end_speed_keep),
        }
        true
    }

    /// obj_exeller Other_7: idle loops over its last frames.
    pub(super) fn exeller_animation_end(&mut self, exeller: &Exeller, frame_count: f64) {
        if self.state == IDLE {
            self.image_index = frame_count - exeller.animation.idle_loop_last_frames;
        }
    }
}

/// `clones[slot]` value of an empty clone slot (-1 in the original).
pub const NO_CLONE: i32 = -1;

// EXE_LOST is the default lost pose of Exeller; the server may pick EXELLER_LOST2.
pub const EXELLER_LOST: usize = EXE_LOST;
