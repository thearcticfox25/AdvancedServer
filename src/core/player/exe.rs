//! obj_exe + scr_exe_special: Sonic.EXE (the original killer). Invisibility and dash attack.

use super::animation::{BALANCING, EMOTION1, EMOTION2, EMOTION3, FALL, HURT, IDLE, WALK};
use super::{gm_sign, Buttons, Player};
use crate::core::resources::names::{sound, sprite};
use crate::core::resources::sprites::Sprites;
use crate::core::config::{self, ExeOriginal, GameplayConfig, KillerAnimation};
use crate::core::events::{SimEvent, EFFECT_SPEED};

/// EXE_* of the original: which killer plays the EXE role.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum ExeCharacter {
    Original = 0,
    Chaos = 1,
    Exetior = 2,
    Exeller = 3,
}

// Sonic.EXE's own animation states (obj_exe Step_2).
pub const EXE_ATTACK: usize = BALANCING + 1;
pub const EXE_AIR_ATTACK: usize = BALANCING + 2;
pub const EXE_SHOCKED: usize = BALANCING + 3;
pub const EXE_WON: usize = BALANCING + 4;
pub const EXE_LOST: usize = BALANCING + 5;
pub const EXE_ZIPLINE: usize = BALANCING + 6;
/// The table holds the visible poses first and the invisible ones after them.
const INVISIBLE_POSES_OFFSET: usize = EXE_ZIPLINE + 1;

impl Player {
    pub(super) fn exe_special(&mut self, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let exe = &cfg.exe;
        let recharge_bottom = -config::ticks(exe.invisibility_recharge_seconds);
        if self.won || self.lost {
            self.invis_timer = recharge_bottom;
            self.is_attacking = false;
            self.attack_timer = 0;
        }
        // Pressing jump again in the air clears the jumping flag (no double jump).
        if self.controls_enabled && self.is_jumping && self.pressed(Buttons::A) {
            self.is_jumping = false;
        }
        self.toggle_invisibility(exe, recharge_bottom, events);
        if self.invis_timer == 0 {
            // Became visible this tick (by timeout or by pressing C again).
            events.push(SimEvent::ExeAppeared { x: self.x, y: self.y });
        }
        if self.invis_timer > recharge_bottom {
            self.invis_timer -= 1;
        }
        self.try_exe_dash(exe, events);
        if self.invis_timer > 0 {
            self.is_attacking = false;
            return;
        }
        if self.is_attacking {
            self.keep_dashing(exe, events);
        }
        if self.attack_timer > 0 {
            self.attack_timer -= 1;
        }
    }

    /// C: turn invisible when recharged, or reappear early while invisible.
    fn toggle_invisibility(&mut self, exe: &ExeOriginal, recharge_bottom: i32, events: &mut Vec<SimEvent>) {
        if !(self.controls_enabled && self.pressed(Buttons::C)) {
            return;
        }
        if self.invis_timer <= recharge_bottom {
            // The sound variant and whether a taunt is already playing are the client's business.
            events.push(SimEvent::ExeVanished { x: self.x, y: self.y });
            self.invis_timer = config::ticks(exe.invisibility_seconds);
        } else if self.invis_timer > 0 {
            self.invis_timer = 0;
        }
    }

    fn try_exe_dash(&mut self, exe: &ExeOriginal, events: &mut Vec<SimEvent>) {
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
        self.attack_timer = self.exe_attack_recharge(exe);
        self.is_attacking = true;
        self.is_jumping = false;
        self.is_spinning = false;
        events.push(SimEvent::Sound { sound: sound::SND_DASH, x: self.x, y: self.y });
    }

    fn exe_attack_recharge(&self, exe: &ExeOriginal) -> i32 {
        let seconds = if self.is_grounded { exe.ground_attack_recharge_seconds } else { exe.air_attack_recharge_seconds };
        config::ticks(seconds)
    }

    fn keep_dashing(&mut self, exe: &ExeOriginal, events: &mut Vec<SimEvent>) {
        if self.yspd > 0.0 {
            self.yspd = if self.is_grounded { 0.0 } else { exe.air_attack_fall_per_tick };
        }
        if self.is_grounded && self.xspd.abs() > 0.0 && self.x.floor() as i64 % exe.dash_dust_every_pixels == 0 {
            events.push(SimEvent::Effect {
                sprite: sprite::SPR_DUST,
                x: self.x - exe.dash_dust_offset_x * gm_sign(self.image_xscale),
                y: self.y + exe.dash_dust_offset_y,
                xscale: 1.0,
                image_speed: EFFECT_SPEED,
                yspd: 0.0,
            });
        }
    }

    /// Spring resets from scr_collision_objects_after.
    pub(super) fn exe_spring_reset(&mut self, exe: &ExeOriginal) {
        if self.is_attacking {
            self.attack_timer = self.exe_attack_recharge(exe);
            self.is_attacking = false;
        }
    }

    /// obj_exe Step_2 checks before the shared ones. Unlike survivors, a stunned
    /// EXE is not "dead": it shows the hurt pose in the air and a daze on the ground.
    pub(super) fn exe_state(&mut self) -> Option<usize> {
        if self.won && self.is_grounded {
            return Some(EXE_WON);
        }
        if self.lost && self.is_grounded {
            return Some(EXE_LOST);
        }
        if self.is_hurt {
            return Some(HURT);
        }
        if self.shocked_timer > 0 {
            return Some(if self.is_grounded { EXE_SHOCKED } else { HURT });
        }
        if self.is_attacking {
            // The original ends the dash when the previous state was the other dash pose.
            // That state was already reset to IDLE at the start of End Step, so this only
            // fires during an emotion; kept exactly as written.
            let (current, other) = if self.is_grounded { (EXE_ATTACK, EXE_AIR_ATTACK) } else { (EXE_AIR_ATTACK, EXE_ATTACK) };
            if self.state == other {
                self.is_attacking = false;
                return Some(self.state);
            }
            return Some(current);
        }
        if self.is_zipline {
            // Unlike the survivors, EXE keeps the zipline pose for the landing tick.
            if self.is_grounded {
                self.is_zipline = false;
            }
            return Some(EXE_ZIPLINE);
        }
        None
    }

    /// Sprite table index: invisible poses follow the visible ones.
    pub(super) fn exe_pose_index(&self) -> usize {
        if self.invis_timer > 0 {
            self.state + INVISIBLE_POSES_OFFSET
        } else {
            self.state
        }
    }

    /// obj_exe Draw_76 for states that differ from the shared ones.
    pub(super) fn exe_animate_state(&mut self, sprites: &Sprites, exe: &ExeOriginal) -> bool {
        if self.state == FALL {
            self.image_speed = exe.animation.fall_speed;
            let last_frame = sprites.get(self.sprite_index).frame_count as f64 - 1.0;
            if self.image_index >= last_frame {
                self.image_index = last_frame;
            }
            if self.xspd > 0.0 {
                self.image_xscale = 1.0;
            } else if self.xspd < 0.0 {
                self.image_xscale = -1.0;
            }
            return true;
        }
        self.animate_killer_state(sprites, &exe.animation, exe.dash_end_speed_keep)
    }

    /// Draw_76 arms Sonic.EXE and Exetior share (same state numbers, same rules).
    pub(super) fn animate_killer_state(&mut self, sprites: &Sprites, animation: &KillerAnimation, dash_end_speed_keep: f64) -> bool {
        let last_frame = sprites.get(self.sprite_index).frame_count as f64 - 1.0;
        match self.state {
            EXE_ZIPLINE => {
                self.image_speed = 0.0;
                self.image_index = 0.0;
            }
            WALK => self.image_speed = self.xspd.abs() / self.max_h_speed,
            EXE_ATTACK | EXE_AIR_ATTACK => {
                let air = self.state == EXE_AIR_ATTACK;
                self.image_speed = if air { animation.air_attack_speed } else { animation.attack_speed };
                // The dash lasts as long as its animation and leaves some of its speed behind.
                if self.image_index >= last_frame {
                    self.xspd *= dash_end_speed_keep;
                    self.gspd *= dash_end_speed_keep;
                    self.is_attacking = false;
                    if air {
                        self.is_jumping = false;
                    }
                }
            }
            EXE_SHOCKED => self.image_speed = animation.shocked_speed,
            EXE_WON | EXE_LOST => self.image_speed = animation.end_pose_speed,
            EMOTION1 | EMOTION2 | EMOTION3 => self.image_speed = animation.emotion_speed,
            _ => return false,
        }
        true
    }

    /// obj_exe Other_7: idle loops over its last frames.
    pub(super) fn exe_animation_end(&mut self, exe: &ExeOriginal, frame_count: f64) {
        if self.state == IDLE {
            self.image_index = frame_count - exe.animation.idle_loop_last_frames;
        }
    }
}
