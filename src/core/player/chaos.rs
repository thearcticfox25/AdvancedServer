//! obj_chaos + scr_chaos_special: aimed air dash, sticking to walls and floors
//! after a dash (and dashing off them), and the liquid form.

use super::animation::{BALANCING, EMOTION1, EMOTION2, EMOTION3, FALL, HURT, IDLE, JUMP, RUN, WALK};
use super::{gm_sign, Buttons, Player};
use crate::core::resources::names::{sound, sprite};
use crate::core::resources::sprites::Sprites;
use crate::core::config::{self, step, Chaos, GameplayConfig};
use crate::core::events::{SimEvent, EFFECT_SPEED};
use crate::core::world::World;

// Chaos' own animation states (obj_chaos Step_2).
pub const CHAOS_ATTACK: usize = BALANCING + 1;
pub const CHAOS_AIR_ATTACK: usize = BALANCING + 2;
pub const CHAOS_SHOCKED: usize = BALANCING + 3;
pub const CHAOS_WON: usize = BALANCING + 4;
pub const CHAOS_LOST: usize = BALANCING + 5;  // same number as EXE_LOST, the default lost pose
pub const CHAOS_LOST2: usize = BALANCING + 6;
pub const CHAOS_ZIPLINE: usize = BALANCING + 7;
/// Stuck to the floor (grounded) or to a wall (in the air).
pub const CHAOS_STUCK_GROUND: usize = BALANCING + 8;
pub const CHAOS_STUCK_WALL: usize = BALANCING + 9;
pub const CHAOS_TRANSFORM: usize = BALANCING + 10;
pub const CHAOS_AIR_TRANSFORM: usize = BALANCING + 11;
/// The table holds the normal poses first and the liquid ones after them.
const LIQUID_POSES_OFFSET: usize = CHAOS_AIR_TRANSFORM + 1;

impl Player {
    pub(super) fn chaos_special(&mut self, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let chaos = &cfg.chaos;
        let liquid_recharge_bottom = -config::ticks(chaos.liquid_recharge_seconds);
        let can_act = self.controls_enabled && self.can_move && !self.is_zipline;
        if can_act && self.is_jumping && self.pressed(Buttons::A) && self.slime_timer <= 0 {
            self.is_jumping = false;
        }
        self.try_chaos_dash(chaos, can_act, events);
        self.try_turn_liquid(chaos, can_act, liquid_recharge_bottom, events);
        self.forget_released_directions();
        self.update_liquid_form(cfg, events);
        if self.attack_timer > 0 && self.stuck_timer <= 0 {
            self.attack_timer -= 1;
        }
        if self.slime_timer > liquid_recharge_bottom {
            self.slime_timer -= 1;
        }
        if self.is_attacking && self.keep_chaos_dashing(chaos, events) {
            return;
        }
        if self.dash_wall_window > 0 {
            self.dash_wall_window -= 1;
        }
        if self.stuck_timer > 0 && self.stay_stuck(cfg, events) {
            return;
        }
        if self.stuck_dash_timer > 0 {
            self.dash_off_surface(chaos);
        }
    }

    /// B: an attack on the ground, an aimed dash in the air.
    fn try_chaos_dash(&mut self, chaos: &Chaos, can_act: bool, events: &mut Vec<SimEvent>) {
        let ready = self.stuck_dash_timer <= 0 && self.attack_timer <= 0 && self.slime_timer <= 0;
        if !(can_act && self.pressed(Buttons::B) && ready) {
            return;
        }
        // Raw keyboard_check in the original.
        if self.buttons.held(Buttons::LEFT) {
            self.image_xscale = -1.0;
        } else if self.buttons.held(Buttons::RIGHT) {
            self.image_xscale = 1.0;
        }
        if self.is_grounded {
            events.push(SimEvent::Sound { sound: sound::SND_CHAOS_ATTACK, x: self.x, y: self.y });
        } else {
            self.aim_air_dash();
            self.dash_wall_window = crate::core::config::original_ticks(chaos.dash_wall_stick_window_ticks);
            self.effect_time = crate::core::config::original_ticks(chaos.dash_trail_ticks);
            // obj_achivements.alarm[5]: the client times the dash from its sound (achievements.rs).
            events.push(SimEvent::Sound { sound: sound::SND_CHAOS_DASH, x: self.x, y: self.y });
        }
        self.image_index = 0.0;
        self.attack_timer = config::ticks(if self.is_grounded { chaos.ground_attack_recharge_seconds } else { chaos.air_attack_recharge_seconds });
        self.is_attacking = true;
        self.is_jumping = false;
        self.is_spinning = false;
    }

    /// The dash flies where the arrows point; without arrows straight ahead.
    fn aim_air_dash(&mut self) {
        self.dash_dir_x = self.image_xscale;
        self.dash_dir_y = 0.0;
        self.is_jumping = false;
        if self.buttons.held(Buttons::LEFT) {
            self.dash_dir_x = -1.0;
        } else if self.buttons.held(Buttons::RIGHT) {
            self.dash_dir_x = 1.0;
        }
        if self.buttons.held(Buttons::UP) {
            self.dash_dir_y = -1.0;
        } else if self.buttons.held(Buttons::DOWN) {
            self.dash_dir_y = 1.0;
        }
    }

    /// C: the liquid form, faster and unable to look, spin or attack.
    fn try_turn_liquid(&mut self, chaos: &Chaos, can_act: bool, recharge_bottom: i32, events: &mut Vec<SimEvent>) {
        if !(can_act && self.pressed(Buttons::C) && self.slime_timer <= recharge_bottom) {
            return;
        }
        events.push(SimEvent::Sound { sound: sound::SND_CHAOS_STURN, x: self.x, y: self.y });
        self.slime_anim = false;
        self.slime_timer = config::ticks(chaos.liquid_seconds);
        self.acc = chaos.movement.acceleration_per_tick * chaos.liquid_acceleration_multiplier;
        self.max_h_speed = chaos.movement.max_speed_per_tick * chaos.liquid_max_speed_multiplier;
        self.can_look_down = false;
        self.can_look_up = false;
        self.can_spin = false;
    }

    /// Releasing a direction re-arms it for dashing off a surface.
    fn forget_released_directions(&mut self) {
        if self.released(Buttons::RIGHT) {
            self.right_pressed = false;
        }
        if self.released(Buttons::LEFT) {
            self.left_pressed = false;
        }
        if self.released(Buttons::UP) {
            self.up_pressed = false;
        }
    }

    fn update_liquid_form(&mut self, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let chaos = &cfg.chaos;
        if self.slime_timer > 0 {
            // A second C press ends the liquid form, but not in its first moments.
            let cancel_from = config::ticks(chaos.liquid_seconds) - crate::core::config::original_ticks(chaos.liquid_cancel_after_ticks);
            if self.slime_timer <= cancel_from && self.controls_enabled && self.pressed(Buttons::C) {
                self.slime_timer = 0;
            }
            if !self.is_grounded && self.effect_time <= 0 {
                self.effect_time = crate::core::config::original_ticks(chaos.liquid_air_trail_ticks);
            }
            if self.shocked_timer > 0 || self.won || self.lost || self.is_zipline {
                self.slime_timer = 0;
            }
        }
        if self.slime_timer == 0 {
            events.push(SimEvent::Sound { sound: sound::SND_CHAOS_STURN, x: self.x, y: self.y });
            let movement = self.movement(cfg);
            self.acc = movement.acceleration_per_tick;
            self.max_h_speed = movement.max_speed_per_tick;
            self.can_look_down = true;
            self.can_look_up = true;
            self.can_spin = true;
        }
    }

    /// Returns true when the special script stops here (the air dash is running).
    fn keep_chaos_dashing(&mut self, chaos: &Chaos, events: &mut Vec<SimEvent>) -> bool {
        if !self.is_grounded || self.stuck_dash_timer > 0 {
            self.xspd = self.dash_dir_x * chaos.air_dash_speed_x_per_tick;
            self.yspd = self.dash_dir_y * chaos.air_dash_speed_y_per_tick;
            let air_recharge = config::ticks(chaos.air_attack_recharge_seconds);
            if self.attack_timer <= air_recharge - crate::core::config::original_ticks(chaos.air_dash_ticks) {
                self.xspd = self.dash_dir_x * chaos.air_dash_end_speed_per_tick;
                self.yspd = self.dash_dir_y * chaos.air_dash_end_speed_per_tick;
                self.is_attacking = false;
                self.effect_time = 0;
            }
            return true;
        }
        if self.yspd > 0.0 {
            self.yspd = if self.is_grounded { 0.0 } else { chaos.air_attack_fall_per_tick };
        }
        self.chaos_dust(chaos, events);
        false
    }

    fn chaos_dust(&self, chaos: &Chaos, events: &mut Vec<SimEvent>) {
        if self.is_grounded && self.xspd.abs() > 0.0 && self.x.floor() as i64 % chaos.dust_every_pixels == 0 {
            events.push(SimEvent::Effect {
                sprite: sprite::SPR_DUST,
                x: self.x - chaos.dust_offset_x * gm_sign(self.image_xscale),
                y: self.y + chaos.dust_offset_y,
                xscale: 1.0,
                image_speed: EFFECT_SPEED,
                yspd: 0.0,
            });
        }
    }

    /// Stuck after dashing into a surface. Returns true when the script stops here.
    fn stay_stuck(&mut self, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) -> bool {
        let chaos = &cfg.chaos;
        let knocked_loose = (self.prev_grounded && !self.is_grounded)
            || (!self.is_grounded && self.xspd.abs() > 0.0)
            || self.is_boosting
            || self.is_zipline
            || self.is_hurt
            || self.shocked_timer > 0
            || self.won
            || self.lost;
        if knocked_loose {
            self.stuck_timer = 0;
            self.can_move = true;
            self.can_spin = true;
            return true;
        }
        self.chaos_dust(chaos, events);
        self.can_spin = false;
        self.can_move = false;
        self.is_jumping = false;
        self.is_attacking = false;
        self.is_spinning = false;
        self.image_xscale = self.stuck_dir;

        if self.is_grounded {
            let friction = self.acc * chaos.stuck_friction_multiplier * step();
            self.gspd -= self.gspd.abs().min(friction) * gm_sign(self.gspd);
            self.xspd -= self.xspd.abs().min(friction) * gm_sign(self.xspd);
            self.xspd -= chaos.stuck_slope_factor * self.angle.sin() * step();
        } else {
            self.yspd = 0.0;
        }

        // The original counts the timer down inside this very comparison.
        let still_early = self.stuck_timer >= crate::core::config::original_ticks(chaos.stuck_escape_window_ticks);
        self.stuck_timer -= 1;
        if still_early && self.pressed_raw(Buttons::B) && self.pushes_away_from_surface() && self.direction_was_released() {
            self.dash_off_start(chaos, events);
        }
        if self.stuck_timer == 0 {
            self.can_move = true;
            self.can_spin = true;
        }
        false
    }

    /// Holding the arrow that points away from the surface (up on a wall).
    fn pushes_away_from_surface(&self) -> bool {
        if !self.is_grounded {
            return self.buttons.held(Buttons::UP);
        }
        (self.stuck_dir > 0.0 && self.buttons.held(Buttons::RIGHT)) || (self.stuck_dir < 0.0 && self.buttons.held(Buttons::LEFT))
    }

    /// The arrow held while landing on the surface does not count; it must be pressed anew.
    fn direction_was_released(&self) -> bool {
        if !self.is_grounded {
            return !self.up_pressed;
        }
        !((self.stuck_dir > 0.0 && self.right_pressed) || (self.stuck_dir < 0.0 && self.left_pressed))
    }

    fn dash_off_start(&mut self, chaos: &Chaos, events: &mut Vec<SimEvent>) {
        self.can_spin = true;
        self.can_move = true;
        self.dash_wall_window = 0;
        self.stuck_dash_timer = crate::core::config::original_ticks(chaos.stuck_dash_ticks);
        self.stuck_timer = 0;
        self.dash_dir_x = if self.is_grounded { self.image_xscale } else { 0.0 };
        self.dash_dir_y = if self.is_grounded { 0.0 } else { chaos.wall_jump_speed_y };
        self.is_jumping = false;
        if !self.is_grounded {
            self.image_xscale = -self.stuck_dir;
            self.attack_timer = config::ticks(chaos.air_attack_recharge_seconds);
            self.is_attacking = true;
        }
        self.image_index = 0.0;
        self.is_spinning = false;
        self.effect_time = crate::core::config::original_ticks(chaos.dash_trail_ticks);
        events.push(SimEvent::Sound { sound: sound::SND_CHAOS_DASH, x: self.x, y: self.y });
    }

    /// The short burst after leaving a surface: a spin along the floor, or a jump off the wall.
    fn dash_off_surface(&mut self, chaos: &Chaos) {
        let interrupted = (self.prev_grounded && !self.is_grounded)
            || self.is_boosting
            || self.is_zipline
            || self.is_hurt
            || self.shocked_timer > 0
            || self.won
            || self.lost;
        if interrupted {
            self.is_attacking = false;
            self.stuck_dash_timer = 0;
            self.can_move = true;
            return;
        }
        if self.is_grounded {
            self.gspd = self.image_xscale * chaos.stuck_dash_ground_speed_per_tick;
            self.is_spinning = true;
        } else {
            self.image_xscale = -self.stuck_dir;
        }
        self.stuck_dash_timer -= 1;
        if self.stuck_dash_timer == 0 {
            self.is_attacking = false;
        }
    }

    /// scr_collision_basic: an air dash into a wall (side sensors) or the floor sticks Chaos there.
    /// `stuck_dir` is the direction Chaos will face while stuck.
    pub(super) fn chaos_try_stick(&mut self, chaos: &Chaos, surface_allows_sticking: bool, stuck_dir: f64, events: &mut Vec<SimEvent>) {
        let dashing_into_it = self.stuck_timer <= 0 && self.state == CHAOS_AIR_ATTACK && self.dash_wall_window > 0;
        if !(surface_allows_sticking && dashing_into_it) {
            return;
        }
        self.can_spin = false;
        self.stuck_timer = crate::core::config::original_ticks(chaos.stuck_ticks);
        self.dash_wall_window = 0;
        self.stuck_dir = stuck_dir;
        // Arrows already held on impact must be released before they can launch a dash.
        self.left_pressed = false;
        self.right_pressed = false;
        self.up_pressed = false;
        if stuck_dir > 0.0 && self.buttons.held(Buttons::RIGHT) {
            self.right_pressed = true;
        }
        if stuck_dir < 0.0 && self.buttons.held(Buttons::LEFT) {
            self.left_pressed = true;
        }
        if self.buttons.held(Buttons::UP) {
            self.up_pressed = true;
        }
        events.push(SimEvent::Sound { sound: sound::SND_CHAOS_LAND, x: self.x, y: self.y });
    }

    /// scr_collision_check: hitting a wall shortens a running ground attack, then maybe sticks.
    pub(super) fn chaos_touch_wall(&mut self, world: &World, chaos: &Chaos, wall: Option<usize>, events: &mut Vec<SimEvent>) {
        if self.stuck_dash_timer <= 0 {
            let air_recharge = config::ticks(chaos.air_attack_recharge_seconds);
            self.attack_timer = self.attack_timer.min(air_recharge - crate::core::config::original_ticks(chaos.air_dash_ticks));
        }
        let wall_allows_sticking = wall.is_some_and(|id| world.instances[id].can_stuck);
        let stuck_dir = -self.image_xscale;
        self.chaos_try_stick(chaos, wall_allows_sticking, stuck_dir, events);
    }

    /// Resets from scr_move_basic when hurt and from the stun branch of Step_0.
    pub(super) fn chaos_stop_abilities(&mut self) {
        if self.slime_timer > 0 {
            self.slime_timer = 0;
        }
        if self.stuck_timer > 0 {
            self.can_move = true;
            self.stuck_timer = 0;
        }
        self.stuck_dash_timer = 0;
    }

    /// Spring resets from scr_collision_objects_after (on top of the shared EXE reset).
    pub(super) fn chaos_spring_reset(&mut self) {
        if self.stuck_timer > 0 {
            self.can_spin = true;
            self.can_move = true;
            self.stuck_timer = 0;
        }
    }

    /// alarm[4] of obj_chaos: the slowdown ends into liquid speeds when liquid.
    pub(super) fn chaos_end_slowdown(&mut self, cfg: &GameplayConfig) {
        let chaos = &cfg.chaos;
        let (acc_multiplier, speed_multiplier) = if self.slime_timer > 0 {
            (chaos.liquid_acceleration_multiplier, chaos.liquid_max_speed_multiplier)
        } else {
            (1.0, 1.0)
        };
        self.acc = chaos.movement.acceleration_per_tick * acc_multiplier;
        self.max_h_speed = chaos.movement.max_speed_per_tick * speed_multiplier;
        self.is_slow = false;
    }

    /// obj_chaos Step_2 checks before the shared ones.
    pub(super) fn chaos_state(&mut self) -> Option<usize> {
        if self.won && self.is_grounded {
            return Some(CHAOS_WON);
        }
        if self.lost && self.is_grounded {
            return Some(self.lost_state);
        }
        if self.stuck_timer > 0 {
            return Some(if self.is_grounded { CHAOS_STUCK_GROUND } else { CHAOS_STUCK_WALL });
        }
        if self.is_hurt {
            return Some(HURT);
        }
        if self.slime_timer > 0 && !self.slime_anim {
            return Some(if self.is_grounded { CHAOS_TRANSFORM } else { CHAOS_AIR_TRANSFORM });
        }
        if self.shocked_timer > 0 {
            return Some(if self.is_grounded { CHAOS_SHOCKED } else { HURT });
        }
        if self.is_attacking {
            // Landing during an air attack, or leaving the ground during a ground attack, ends it.
            if self.is_grounded != self.prev_grounded {
                self.is_attacking = false;
                return Some(self.state);
            }
            return Some(if self.is_grounded { CHAOS_ATTACK } else { CHAOS_AIR_ATTACK });
        }
        if self.is_zipline {
            if self.is_grounded {
                self.is_zipline = false;
            }
            return Some(CHAOS_ZIPLINE);
        }
        None
    }

    /// Sprite table index: liquid poses follow the normal ones once the transformation played.
    pub(super) fn chaos_pose_index(&self) -> usize {
        if self.slime_timer > 0 && self.slime_anim {
            self.state + LIQUID_POSES_OFFSET
        } else {
            self.state
        }
    }

    /// obj_chaos Draw_76 for states that differ from the shared ones.
    pub(super) fn chaos_animate_state(&mut self, sprites: &Sprites, cfg: &GameplayConfig) -> bool {
        let chaos = &cfg.chaos;
        let animation = &chaos.animation;
        let shared = &cfg.physics.animation;
        let liquid = self.slime_timer > 0;
        let frame_count = sprites.get(self.sprite_index).frame_count as f64;
        let last_frame = frame_count - 1.0;
        let run_speed = (self.xspd.abs() / shared.run_speed_divisor).max(shared.run_min_speed) * animation.run_multiplier;
        match self.state {
            HURT | CHAOS_ZIPLINE if liquid => self.image_speed = 1.0,
            HURT | CHAOS_ZIPLINE => {
                self.image_speed = 0.0;
                self.image_index = 0.0;
            }
            FALL => {
                self.image_speed = 1.0;
                self.image_index = if self.yspd > 0.0 { 1.0 } else { 0.0 };
            }
            WALK => {
                let multiplier = if liquid { animation.liquid_walk_multiplier } else { animation.walk_multiplier };
                self.image_speed = self.xspd.abs() / self.max_h_speed * multiplier;
            }
            RUN => self.image_speed = if liquid { animation.liquid_run_speed } else { run_speed },
            JUMP if liquid => {
                self.image_speed = animation.liquid_jump_speed;
                if self.image_index >= last_frame {
                    self.image_index = last_frame;
                }
            }
            JUMP => self.image_speed = run_speed,
            CHAOS_ATTACK => {
                self.image_speed = animation.attack_speed;
                if self.image_index >= last_frame {
                    self.xspd *= chaos.attack_end_speed_keep;
                    self.gspd *= chaos.attack_end_speed_keep;
                    self.is_attacking = false;
                }
            }
            CHAOS_AIR_ATTACK => {
                if self.xspd.abs() > 0.0 {
                    self.image_xscale = gm_sign(self.xspd);
                }
                self.image_speed = animation.air_attack_speed;
            }
            CHAOS_SHOCKED => self.image_speed = animation.shocked_speed,
            CHAOS_LOST | CHAOS_WON => {
                self.image_speed = 1.0;
                if self.image_index >= last_frame {
                    self.image_index = last_frame;
                }
            }
            CHAOS_LOST2 => {
                self.image_speed = 1.0;
                if self.image_index >= last_frame {
                    self.image_index = frame_count - animation.lost2_loop_last_frames;
                }
            }
            CHAOS_STUCK_GROUND | CHAOS_STUCK_WALL => {
                self.image_xscale = self.stuck_dir;
                // The original flickers frames 0 and 1 by wall clock time while the escape
                // window is open; that is presentation, the client animates it.
                self.image_index = if self.stuck_timer >= crate::core::config::original_ticks(chaos.stuck_escape_window_ticks) { 0.0 } else { animation.stuck_frame };
            }
            CHAOS_TRANSFORM | CHAOS_AIR_TRANSFORM => {
                self.image_speed = 1.0;
                if self.image_index >= last_frame {
                    self.slime_anim = true;
                }
            }
            EMOTION1 | EMOTION2 | EMOTION3 => self.image_speed = 1.0,
            _ => return false,
        }
        true
    }

    /// obj_chaos Other_7: idle loops over its last frames.
    pub(super) fn chaos_animation_end(&mut self, chaos: &Chaos, frame_count: f64) {
        if self.state == IDLE {
            self.image_index = frame_count - chaos.animation.idle_loop_last_frames;
        }
    }
}
