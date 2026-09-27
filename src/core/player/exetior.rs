//! obj_exetior + scr_exetior_special: dash attacks, the stomp with its shockwaves,
//! and black rings.

use super::animation::{FALL, HURT, IDLE};
use super::exe::{EXE_AIR_ATTACK, EXE_ATTACK, EXE_LOST, EXE_SHOCKED, EXE_WON, EXE_ZIPLINE};
use super::{gm_sign, Buttons, Player};
use crate::core::resources::names::{sound, sprite};
use crate::core::resources::sprites::Sprites;
use crate::core::config::{self, Exetior, GameplayConfig};
use crate::core::events::{SimEvent, EFFECT_SPEED};
use crate::core::world::World;

// Exetior's own animation states (obj_exetior Step_2); the others share EXE's numbers.
pub const EXETIOR_STOMP: usize = EXE_ZIPLINE + 1;
pub const EXETIOR_STOMP_LAND: usize = EXE_ZIPLINE + 2;

impl Player {
    pub(super) fn exetior_special(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let exetior = &cfg.exetior;
        // Pressing jump again in the air clears the jumping flag (no double jump).
        if self.controls_enabled && self.is_jumping && self.pressed(Buttons::A) {
            self.is_jumping = false;
        }
        self.try_exetior_attack(exetior, events);
        self.try_place_black_ring(world, exetior, events);
        if self.bring_timer > 0 {
            self.bring_timer -= 1;
        }
        if self.is_attacking {
            self.keep_exetior_dashing(exetior, events);
        }
        if self.attack_timer > 0 {
            self.attack_timer -= 1;
        }
        if self.is_stomping {
            self.stomp(exetior, events);
        }
    }

    /// B: ground dash, air dash, or with down in the air a stomp.
    fn try_exetior_attack(&mut self, exetior: &Exetior, events: &mut Vec<SimEvent>) {
        if !(self.controls_enabled && self.pressed(Buttons::B) && !self.is_zipline && self.attack_timer <= 0) {
            return;
        }
        // Raw keyboard_check in the original.
        if self.buttons.held(Buttons::LEFT) {
            self.image_xscale = -1.0;
        } else if self.buttons.held(Buttons::RIGHT) {
            self.image_xscale = 1.0;
        }
        self.image_index = 0.0;
        self.is_attacking = true;
        self.is_jumping = false;
        self.is_spinning = false;

        if !self.is_grounded && self.buttons.held(Buttons::DOWN) {
            // The stomp sets its own cooldown every tick while it runs.
            self.is_stomping = true;
            self.just_landed = false;
            events.push(SimEvent::Sound { sound: sound::SND_EXETIOR_STOMP, x: self.x, y: self.y });
            return;
        }
        let recharge = if self.is_grounded { exetior.ground_attack_recharge_seconds } else { exetior.air_attack_recharge_seconds };
        self.attack_timer = config::ticks(recharge);
        events.push(SimEvent::Sound { sound: sound::SND_DASH, x: self.x, y: self.y });
    }

    /// C: a black ring behind Exetior.
    fn try_place_black_ring(&mut self, world: &World, exetior: &Exetior, events: &mut Vec<SimEvent>) {
        let body = crate::core::collision::sprite_bbox(world.sprites.get(self.sprite_index), self.x, self.y, self.image_xscale, 1.0, 0.0);
        let can_spawn_rings = world.black_rings.iter().all(|ring| world.black_ring_bbox(ring).distance(&body) >= exetior.black_ring_spacing);
        if !(self.controls_enabled && self.pressed(Buttons::C) && self.bring_timer <= 0 && can_spawn_rings) {
            return;
        }
        events.push(SimEvent::SpawnBlackRing {
            x: self.x + exetior.black_ring_offset_x,
            y: self.y + exetior.black_ring_offset_y,
        });
        events.push(SimEvent::Sound { sound: sound::SND_BLACKRING, x: self.x, y: self.y });
        self.bring_timer = config::ticks(exetior.black_ring_recharge_seconds);
    }

    fn keep_exetior_dashing(&mut self, exetior: &Exetior, events: &mut Vec<SimEvent>) {
        if self.is_grounded && self.xspd.abs() > 0.0 && self.x.floor() as i64 % exetior.dust_every_pixels == 0 {
            events.push(SimEvent::Effect {
                sprite: sprite::SPR_DUST,
                x: self.x - exetior.dust_offset_x * gm_sign(self.image_xscale),
                y: self.y + exetior.dust_offset_y,
                xscale: 1.0,
                image_speed: EFFECT_SPEED,
                yspd: 0.0,
            });
        }
        if self.yspd > 0.0 {
            self.yspd = if self.is_grounded { 0.0 } else { exetior.air_attack_fall_per_tick };
        }
    }

    /// Falling straight down; on landing a shockwave runs along the floor to both sides.
    fn stomp(&mut self, exetior: &Exetior, events: &mut Vec<SimEvent>) {
        self.attack_timer = config::ticks(exetior.stomp_cooldown_seconds);
        if self.is_zipline || self.is_hurt || self.shocked_timer > 0 || self.won || self.lost {
            self.is_attacking = false;
            self.is_stomping = false;
            self.can_move = true;
            return;
        }
        self.xspd = 0.0;
        self.can_move = false;
        if !self.is_grounded {
            if self.effect_time <= 0 {
                self.effect_time = crate::core::config::original_ticks(exetior.stomp_trail_ticks);
            }
            events.push(SimEvent::SmallCameraShake);
            self.is_attacking = true;
            self.yspd = exetior.stomp_fall_speed_per_tick;
            return;
        }
        if !self.just_landed {
            events.push(SimEvent::Sound { sound: sound::SND_EXETIOR_STOMPLAND, x: self.x, y: self.y });
            events.push(SimEvent::SpawnStompWaves { x: self.x, y: self.y });
            self.just_landed = true;
        }
        events.push(SimEvent::CameraShake);
        self.is_attacking = false;
        self.yspd = 0.0;
    }

    /// Resets from the stun branch of Step_0 and from scr_move_basic when hurt.
    pub(super) fn exetior_stop_stomp(&mut self) {
        self.can_move = true;
        self.is_stomping = false;
    }

    /// obj_exetior Step_2 checks before the shared ones.
    pub(super) fn exetior_state(&mut self) -> Option<usize> {
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
        if self.is_zipline {
            if self.is_grounded {
                self.is_zipline = false;
            }
            return Some(EXE_ZIPLINE);
        }
        if self.is_stomping {
            return Some(if self.is_grounded { EXETIOR_STOMP_LAND } else { EXETIOR_STOMP });
        }
        if self.is_attacking {
            // Landing during an air dash, or leaving the ground during a ground dash, ends it.
            if self.is_grounded != self.prev_grounded {
                self.is_attacking = false;
                return Some(self.state);
            }
            return Some(if self.is_grounded { EXE_ATTACK } else { EXE_AIR_ATTACK });
        }
        None
    }

    /// obj_exetior Draw_76 for states that differ from the shared ones.
    pub(super) fn exetior_animate_state(&mut self, sprites: &Sprites, exetior: &Exetior) -> bool {
        match self.state {
            FALL => self.animate_fall_by_direction(exetior.animation.fall_speed),
            EXETIOR_STOMP => self.image_speed = exetior.stomp_animation_speed,
            // The landing pose keeps Exetior in place until it has played.
            EXETIOR_STOMP_LAND => {
                self.image_speed = exetior.stomp_land_animation_speed;
                if self.image_index >= sprites.get(self.sprite_index).frame_count as f64 - 1.0 {
                    self.can_move = true;
                    self.is_stomping = false;
                }
            }
            _ => return self.animate_killer_state(sprites, &exetior.animation, exetior.dash_end_speed_keep),
        }
        true
    }

    /// obj_exetior Other_7: idle loops over its last frames.
    pub(super) fn exetior_animation_end(&mut self, exetior: &Exetior, frame_count: f64) {
        if self.state == IDLE {
            self.image_index = frame_count - exetior.animation.idle_loop_last_frames;
        }
    }
}
