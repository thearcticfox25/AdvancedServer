//! scr_move_basic: acceleration, rolling, air control, jumping, balancing.

use super::{gm_sign, Buttons, Character, Player, Sensor};
use crate::core::resources::names::sound;
use crate::core::config::{self, step, GameplayConfig, Physics};
use crate::core::events::SimEvent;
use crate::core::objects::ids::ObjectId;
use crate::core::rooms::ids::RoomId;
use crate::core::world::World;

/// The idle key jumps the idle animation to its fidget.
const FIDGET_FRAME: f64 = 48.0;
const AMY_FIDGET_FRAME: f64 = 53.0;

impl Player {
    pub(super) fn move_basic(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        // Before the round starts the original runs the collision script here AND
        // again in the Step event, so gravity applies twice per tick. Kept for parity.
        if !world.round_started {
            self.collision_basic(world, cfg, events);
            return;
        }
        // Achievement 21 (25 rings): client achievements.rs rings.

        self.just_jumped = false;
        self.is_looking_down = false;
        self.is_looking_up = false;

        let physics = &cfg.physics;
        if self.is_dead {
            self.slide_while_dead(physics, events);
            return;
        }
        self.update_bounce_timer();
        if self.is_hurt {
            self.fly_back_while_hurt(physics);
            return;
        }
        if self.is_boosting {
            self.boost(cfg);
            return;
        }
        self.warn_about_hiding(world, cfg, events);
        self.update_balancing(world, physics);
        self.update_emotions(events);
        if self.is_grounded {
            self.move_on_ground(physics, events);
        }
        self.special(world, cfg, events);
        if !self.is_grounded {
            self.move_in_air(physics);
        } else {
            self.try_jump(cfg, events);
        }
    }

    /// scr_move_basic: a survivor who stays within one screenful of the map for twenty
    /// seconds is told off for hiding and slowed down until they move somewhere else.
    /// Act 9 is left out (there the walls do the moving), and so are the last minute,
    /// the dead and anyone out of revivals.
    fn warn_about_hiding(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let rules = &cfg.hiding;
        let watched = self.character != Character::Exe
            && world.room != Some(RoomId::Act9)
            && !self.is_dead
            && self.revival_times < 2
            && world.timer_ticks >= config::ticks(rules.last_minute_seconds) as i64;
        if !watched {
            self.end_hiding_warning(events);
            return;
        }

        self.chunk_ticks += 1;
        if self.chunk_ticks % config::ticks(rules.chunk_check_seconds) == 0 {
            let corner = (self.x - rules.chunk_width / 2.0, self.y - rules.chunk_height / 2.0);
            let left_the_chunk = (corner.0 + rules.chunk_width <= self.chunk.0)
                || (corner.0 >= self.chunk.0 + rules.chunk_width)
                || (corner.1 + rules.chunk_height <= self.chunk.1)
                || (corner.1 >= self.chunk.1 + rules.chunk_height);
            if left_the_chunk {
                self.chunk = corner;
                self.chunk_ticks = 0;
                self.end_hiding_warning(events);
            }
        }
        if self.chunk_ticks == config::ticks(rules.warning_after_seconds) {
            self.hiding_warning = true;
            events.push(SimEvent::HidingWarning { shown: true });
        }
        // obj_player_warning Draw calls scr_player_slow(5) every step it is up.
        if self.hiding_warning {
            self.slow_down(cfg, rules.slow_percent, rules.slow_seconds);
        }
    }

    /// The warning leaves when the player moves on, goes down or the clock runs low.
    fn end_hiding_warning(&mut self, events: &mut Vec<SimEvent>) {
        if self.hiding_warning {
            self.hiding_warning = false;
            events.push(SimEvent::HidingWarning { shown: false });
        }
    }

    /// Emotions of scr_move_basic: a taunt pose on each emotion key, the idle fidget on the idle key.
    fn update_emotions(&mut self, events: &mut Vec<SimEvent>) {
        use super::amy::AMY_HJUMP;
        use super::animation::{EMOTION1, EMOTION2, EMOTION3, IDLE};
        use super::sally::SALLY_SLIDE;
        let keeps_emotion = matches!(self.state, IDLE | EMOTION1 | EMOTION2 | EMOTION3)
            || (self.character == Character::Amy && self.state == AMY_HJUMP)
            || (self.character == Character::Sally && self.state == SALLY_SLIDE);
        if !keeps_emotion {
            self.emotion = false;
        }
        if self.state == IDLE && self.pressed_raw(Buttons::IDLE_POSE) {
            self.image_index = if self.character == Character::Amy { AMY_FIDGET_FRAME } else { FIDGET_FRAME };
        }
        if !(self.controls_enabled && !self.emotion && !self.is_on_edge) {
            return;
        }
        let taunts = [(Buttons::EMOTION1, EMOTION1), (Buttons::EMOTION2, EMOTION2), (Buttons::EMOTION3, EMOTION3)];
        let Some(number) = taunts.iter().position(|&(button, _)| self.pressed_raw(button)) else { return };
        if self.state == IDLE {
            self.image_index = 0.0;
            self.state = taunts[number].1;
            self.emotion = true;
        }
        if self.character == Character::Exe {
            events.push(SimEvent::Taunt { exe: self.exe_character, emotion: number as u8 + 1, x: self.x, y: self.y });
        }
    }

    fn slide_while_dead(&mut self, physics: &Physics, events: &mut Vec<SimEvent>) {
        if self.red_ring_timer > 0 {
            events.push(SimEvent::RedRingEnded);
            self.red_ring_timer = 0;
        }
        self.emotion = false;
        self.is_zipline = false;
        self.is_attacking = false;
        self.is_boosting = false;
        self.is_hiding = false;
        match self.character {
            Character::Cream => self.cream_stop_abilities(),
            Character::Sally => self.sally_stop_on_death(),
            _ => {}
        }
        self.gspd -= self.gspd.abs().min(self.acc * step()) * gm_sign(self.gspd);
        self.xspd -= self.xspd.abs().min(self.acc * step()) * gm_sign(self.xspd);
        self.xspd -= physics.dead_slope_factor * self.angle.sin() * step();
    }

    /// After bouncing off an enemy, jump/spin flags are cleared when the timer runs out.
    fn update_bounce_timer(&mut self) {
        self.bounce_timer -= 1;
        if self.bounce_timer > 0 && self.is_grounded {
            self.bounce_timer = -1;
        }
        if self.bounce_timer == 0 {
            if self.character != Character::Exe {
                self.is_jumping = false;
                self.is_spinning = false;
            }
            if self.is_grounded {
                self.bounce_timer = -1;
            }
        }
    }

    fn fly_back_while_hurt(&mut self, physics: &Physics) {
        match self.character {
            Character::Tails => self.attack_after = 0,
            Character::Cream => self.cream_stop_abilities(),
            Character::Sally => {
                self.is_attacking = false;
                self.is_sliding = false;
            }
            Character::Exe => {
                self.is_attacking = false;
                match self.exe_character {
                    super::ExeCharacter::Chaos => self.chaos_stop_abilities(),
                    super::ExeCharacter::Exetior => self.is_stomping = false,
                    _ => {}
                }
            }
            _ => {}
        }
        self.gspd = 0.0;
        let recover = physics.hurt_recover_speed;
        if self.xspd > -recover && self.xspd < recover && self.is_grounded {
            self.is_hurt = false;
        }
        if self.xspd > 0.0 {
            self.xspd -= physics.hurt_air_friction_per_tick * step();
        } else {
            self.xspd += physics.hurt_air_friction_per_tick * step();
        }
        self.yspd += physics.hurt_gravity_per_tick * step();
        self.is_boosting = false;
        self.emotion = false;
    }

    /// Launched by a speed monitor: run at full speed until something stops it.
    fn boost(&mut self, cfg: &GameplayConfig) {
        if self.effect_time <= 0 {
            self.effect_time = crate::core::config::original_ticks(cfg.physics.boost_trail_ticks);
        }
        match self.character {
            Character::Knux => {
                self.is_gliding = false;
                self.unstick();
            }
            Character::Eggman => self.egg_boost_reset(&cfg.eggman),
            Character::Sally => {
                if self.shield_timer > 0 {
                    self.shield_timer = 0;
                }
            }
            // Unlike the hurt reset, a boost does not give movement back to a stuck Chaos.
            Character::Exe if self.exe_character == super::ExeCharacter::Chaos => {
                self.stuck_timer = 0;
                self.stuck_dash_timer = 0;
            }
            _ => {}
        }
        if self.is_grounded {
            self.gspd = cfg.physics.boost_speed_per_tick * self.image_xscale;
            self.gspd -= cfg.physics.slope_factor_running * self.angle.sin() * step();
            self.xspd = self.gspd * self.angle.cos();
            self.yspd = self.gspd * -self.angle.sin();
        }
    }

    /// Balancing: standing still with one bottom sensor over empty space.
    fn update_balancing(&mut self, world: &World, physics: &Physics) {
        let one_sensor_on_floor = !(self.sensor_bl.coll && self.sensor_br.coll);
        if !(self.is_grounded && self.xspd.abs() <= 0.0 && one_sensor_on_floor) {
            self.is_on_edge = false;
            return;
        }
        let sensor = if self.sensor_bl.coll {
            (self.sensor_bl, false)
        } else if self.sensor_br.coll {
            (self.sensor_br, true)
        } else {
            return;
        };
        let (found_gap, direction) = find_edge_gap(world, physics, sensor.0, sensor.1);
        self.is_on_edge = found_gap;
        self.edge_dir = if direction >= 0.0 { 1.0 } else { -1.0 };
    }

    fn move_on_ground(&mut self, physics: &Physics, events: &mut Vec<SimEvent>) {
        self.is_jumping = false;

        if self.can_look_down && self.held(Buttons::DOWN) {
            if self.gspd == 0.0 {
                self.is_looking_down = true;
            } else if self.gspd.abs() >= physics.min_speed_to_roll && !self.is_spinning && self.can_spin {
                events.push(SimEvent::Sound { sound: sound::SND_SPIN, x: self.x, y: self.y });
                self.is_spinning = true;
            }
        }
        if self.can_look_up && self.held(Buttons::UP) && self.gspd == 0.0 {
            self.is_looking_up = true;
        }

        if !self.is_spinning {
            if !self.is_looking_down && !self.is_looking_up {
                self.run_on_ground(physics);
            }
        } else {
            self.roll_on_ground(physics);
        }

        // Slopes slow the player down; rolling uses a stronger factor so it gains speed downhill.
        let slope_factor = if self.is_spinning { physics.slope_factor_rolling } else { physics.slope_factor_running };
        self.gspd -= slope_factor * self.angle.sin() * step();
        self.xspd = self.gspd * self.angle.cos();
        self.yspd = self.gspd * -self.angle.sin();

        if self.gspd == 0.0 {
            self.is_spinning = false;
        }
    }

    fn run_on_ground(&mut self, physics: &Physics) {
        let turn = physics.turn_around_deceleration_per_tick * step();
        if self.held(Buttons::LEFT) {
            if self.gspd < 0.0 {
                self.image_xscale = -1.0;
            }
            if self.gspd > 0.0 {
                self.gspd -= turn;
                if self.gspd <= 0.0 {
                    self.gspd = -turn;
                }
            } else if self.gspd > -self.max_h_speed {
                self.gspd -= self.acc * step();
                if self.gspd <= -self.max_h_speed {
                    self.gspd = -self.max_h_speed;
                }
            }
        } else if self.held(Buttons::RIGHT) {
            if self.gspd > 0.0 {
                self.image_xscale = 1.0;
            }
            if self.gspd < 0.0 {
                self.gspd += turn;
                if self.gspd >= 0.0 {
                    self.gspd = turn;
                }
            } else if self.gspd < self.max_h_speed {
                self.gspd += self.acc * step();
                if self.gspd >= self.max_h_speed {
                    self.gspd = self.max_h_speed;
                }
            }
        } else {
            self.gspd -= self.gspd.abs().min(self.acc * step()) * gm_sign(self.gspd);
        }
    }

    fn roll_on_ground(&mut self, physics: &Physics) {
        if self.held(Buttons::LEFT) && self.gspd > 0.0 {
            self.gspd -= physics.roll_brake_per_tick * step();
            if self.gspd <= 0.0 {
                self.is_spinning = false;
            }
        } else if self.held(Buttons::RIGHT) && self.gspd < 0.0 {
            self.gspd += physics.roll_brake_per_tick * step();
            if self.gspd >= 0.0 {
                self.is_spinning = false;
            }
        }
        self.gspd -= self.gspd.abs().min(physics.roll_friction_per_tick * step()) * gm_sign(self.gspd);
    }

    fn move_in_air(&mut self, physics: &Physics) {
        self.angle = 0.0;
        let air_acc = self.acc * physics.air_acceleration_multiplier * step();
        if self.held(Buttons::LEFT) {
            self.image_xscale = -1.0;
            self.xspd -= air_acc;
        } else if self.held(Buttons::RIGHT) {
            self.image_xscale = 1.0;
            self.xspd += air_acc;
        }

        // Air drag, only near the top of a jump.
        if self.yspd < 0.0 && self.yspd > -physics.air_drag_below_rise_speed && self.xspd.abs() > 0.0 {
            let steps = (self.xspd / physics.air_drag_step).ceil();
            self.xspd -= steps / physics.air_drag_divisor * step();
        }
    }

    fn try_jump(&mut self, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        if !self.pressed(Buttons::A) || self.is_jumping {
            return;
        }
        if matches!(self.character, Character::Knux | Character::Amy | Character::Exe) && self.is_attacking {
            return;
        }
        if self.character == Character::Cream && self.rings_spawn > 0 {
            return;
        }
        events.push(SimEvent::Sound { sound: sound::SND_JUMP, x: self.x, y: self.y });
        let jump_force = self.movement(cfg).jump_force_per_tick;
        self.is_grounded = false;
        self.is_spinning = false;
        self.is_jumping = true;
        self.just_jumped = true;
        self.xspd -= jump_force * self.angle.sin();
        self.yspd = -jump_force * self.angle.cos();
    }
}

/// Looks for empty floor beside a bottom sensor (the balancing check).
/// Returns whether a gap was found and on which side.
fn find_edge_gap(world: &World, physics: &Physics, sensor: Sensor, is_right_sensor: bool) -> (bool, f64) {
    let half = physics.edge_search_half_width as i32;
    // The original scans the right sensor left to right and the left one right to left.
    let offsets: Vec<i32> = if is_right_sensor { (-half..half).collect() } else { (-half + 1..=half).rev().collect() };
    for i in offsets {
        let i = i as f64;
        let has_floor_below = (0..physics.edge_search_depth as i32)
            .any(|j| crate::core::collision::sensor_meeting(world, sensor.x + i, sensor.y + j as f64, ObjectId::FloorParent));
        if !has_floor_below {
            return (true, gm_sign(i));
        }
    }
    (false, 1.0)
}
