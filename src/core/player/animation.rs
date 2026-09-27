//! Animation state: which state the character is in (End Step), which sprite
//! and speed that state uses (Pre-Draw), and what happens when an animation
//! loops (Animation End). The server runs this too: attack frames and
//! collision masks depend on the current sprite frame.
//!
//! The parts every character shares live here. Each character file adds its
//! own states (checked before the shared ones, in the original order).

use super::animation_tables::*;
use super::{Buttons, ExeCharacter, Player};
use crate::core::resources::sprites::Sprites;
use crate::core::resources::SpriteId;
use crate::core::config::GameplayConfig;
use crate::core::events::SimEvent;
use crate::core::rooms::ids::RoomId;

/// CHARACTER_* of the original, same numbers.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Character {
    Exe = 0,
    Tails = 1,
    Knux = 2,
    Eggman = 3,
    Amy = 4,
    Cream = 5,
    Sally = 6,
}

// Shared animation states (scr_survivors.gml).
pub const IDLE: usize = 0;
pub const WALK: usize = 1;
pub const RUN: usize = 2;
pub const JUMP: usize = 3;
pub const FALL: usize = 4;
pub const HURT: usize = 5;
pub const DEAD: usize = 6;
pub const LOOKUP: usize = 7;
pub const LOOKDOWN: usize = 8;
pub const EMOTION1: usize = 9;
pub const EMOTION2: usize = 10;
pub const EMOTION3: usize = 11;
pub const SPIN: usize = 12;
pub const BALANCING: usize = 13;

/// GameMaker's animation clock: sprite speeds are frames per second of a 60 steps/s game.
const STEPS_PER_SECOND: f64 = 60.0;

impl Player {
    fn animation_table(&self) -> &'static [SpriteId] {
        let demonized = self.is_demonized();
        match (self.character, demonized) {
            (Character::Tails, false) => &SURVIVOR_TAILS,
            (Character::Tails, true) => &DEMONIZED_TAILS,
            (Character::Knux, false) => &SURVIVOR_KNUX,
            (Character::Knux, true) => &DEMONIZED_KNUX,
            (Character::Eggman, false) => &SURVIVOR_EGGMAN,
            (Character::Eggman, true) => &DEMONIZED_EGGMAN,
            (Character::Amy, false) => &SURVIVOR_AMY,
            (Character::Amy, true) => &DEMONIZED_AMY,
            (Character::Cream, false) => &SURVIVOR_CREAM,
            (Character::Cream, true) => &DEMONIZED_CREAM,
            (Character::Sally, false) => &SURVIVOR_SALLY,
            (Character::Sally, true) => &DEMONIZED_SALLY,
            (Character::Exe, _) => match self.exe_character {
                ExeCharacter::Chaos => &EXE_CHAOS,
                ExeCharacter::Exetior => &EXE_EXETIOR,
                ExeCharacter::Exeller => &EXE_EXELLER,
                ExeCharacter::Original => &EXE_ORIGINAL,
            },
        }
    }

    pub(super) fn animation_idle_sprite(&self) -> SpriteId {
        self.animation_table()[IDLE]
    }

    /// GameMaker's image update at the start of a step: advance by
    /// image_speed * sprite fps / 60 and fire Animation End on wrap-around.
    pub(super) fn advance_animation(&mut self, sprites: &Sprites, cfg: &GameplayConfig) {
        let frame_count = sprites.get(self.sprite_index).frame_count as f64;
        self.image_index += self.image_speed * sprites.get(self.sprite_index).fps as f64 / STEPS_PER_SECOND;
        if self.image_index >= frame_count {
            self.image_index -= frame_count;
            self.on_animation_end(cfg, frame_count);
        } else if self.image_index < 0.0 {
            self.image_index += frame_count;
            self.on_animation_end(cfg, frame_count);
        }
    }

    /// Other_7 of the character object.
    fn on_animation_end(&mut self, cfg: &GameplayConfig, frame_count: f64) {
        if self.end_emotion_animation(frame_count) {
            return;
        }
        match self.character {
            Character::Tails => self.tails_animation_end(&cfg.tails),
            Character::Knux => self.knux_animation_end(&cfg.knuckles, frame_count),
            Character::Eggman => self.egg_animation_end(&cfg.eggman, frame_count),
            Character::Amy => self.amy_animation_end(&cfg.amy, frame_count),
            Character::Cream => self.cream_animation_end(&cfg.cream, frame_count),
            Character::Sally => self.sally_animation_end(&cfg.sally, frame_count),
            Character::Exe => match self.exe_character {
                ExeCharacter::Chaos => self.chaos_animation_end(&cfg.chaos, frame_count),
                ExeCharacter::Exetior => self.exetior_animation_end(&cfg.exetior, frame_count),
                ExeCharacter::Exeller => self.exeller_animation_end(&cfg.exeller, frame_count),
                ExeCharacter::Original => self.exe_animation_end(&cfg.exe, frame_count),
            },
        }
    }

    /// The emotion cases of Other_7: holding the key repeats the taunt from its loop
    /// frame, letting go returns to idle. Returns whether the state was an emotion.
    fn end_emotion_animation(&mut self, frame_count: f64) -> bool {
        let (key, number) = match self.state {
            EMOTION1 => (Buttons::EMOTION1, 0),
            EMOTION2 => (Buttons::EMOTION2, 1),
            EMOTION3 => (Buttons::EMOTION3, 2),
            _ => return false,
        };
        if self.buttons.held(key) {
            self.image_index = emotion_loop_frame(self.character, self.exe_character, number, frame_count);
        } else {
            self.image_index = 0.0;
            self.state = IDLE;
            self.emotion = false;
        }
        true
    }

    /// Step_2 of the character object: pick the state from the movement flags.
    /// Same check order and early returns as the original, the first match wins.
    pub(super) fn choose_state(&mut self, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        if !self.emotion {
            self.state = IDLE;
        }
        // Survivors only: EXE has its own stun poses and never dies.
        if self.character != Character::Exe && (self.is_dead || self.shocked_timer > 0) {
            self.state = DEAD;
            return;
        }
        let own_state = match self.character {
            Character::Tails => self.tails_state(),
            Character::Knux => self.knux_state(),
            Character::Eggman => self.egg_state(),
            Character::Amy => self.amy_state(),
            Character::Cream => self.cream_state(),
            Character::Sally => self.sally_state(),
            Character::Exe => match self.exe_character {
                ExeCharacter::Chaos => self.chaos_state(),
                ExeCharacter::Exetior => self.exetior_state(),
                ExeCharacter::Exeller => self.exeller_state(),
                ExeCharacter::Original => self.exe_state(),
            },
        };
        if let Some(state) = own_state {
            self.state = state;
            return;
        }
        self.choose_movement_state(cfg, events);
    }

    /// The part of Step_2 every character ends with.
    fn choose_movement_state(&mut self, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        if self.is_looking_up {
            self.state = LOOKUP;
            return;
        }
        if self.is_looking_down {
            self.state = LOOKDOWN;
            return;
        }
        if self.is_spinning {
            self.state = SPIN;
            return;
        }
        if self.xspd.abs() > 0.0 && self.is_grounded {
            let run_speed = self.movement(cfg).run_animation_speed_per_tick;
            self.state = if self.xspd.abs() < run_speed { WALK } else { RUN };
        }
        if self.is_on_edge {
            self.state = BALANCING;
            return;
        }
        if !self.is_grounded {
            self.state = match self.character {
                Character::Eggman => self.egg_air_state(&cfg.eggman, events),
                _ if self.is_jumping => JUMP,
                _ => FALL,
            };
        }
    }


    /// Draw_76 of the character object: sprite and animation speed for the state.
    pub(super) fn apply_state_animation(&mut self, sprites: &Sprites, cfg: &GameplayConfig, room: Option<RoomId>) {
        let pose = match (self.character, self.exe_character) {
            (Character::Exe, ExeCharacter::Chaos) => self.chaos_pose_index(),
            (Character::Exe, ExeCharacter::Exetior) => self.state,
            // Exeller's Draw_76 adds the invisible offset too, but Exeller never turns invisible.
            (Character::Exe, ExeCharacter::Original | ExeCharacter::Exeller) => self.exe_pose_index(),
            _ => self.state,
        };
        self.sprite_index = self.animation_table()[pose];
        let handled_by_character = match self.character {
            Character::Tails => self.tails_animate_state(&cfg.tails),
            Character::Knux => self.knux_animate_state(sprites, &cfg.knuckles, room),
            Character::Eggman => self.egg_animate_state(sprites, &cfg.eggman),
            Character::Amy => self.amy_animate_state(sprites, &cfg.amy),
            Character::Cream => self.cream_animate_state(sprites, &cfg.cream),
            Character::Sally => self.sally_animate_state(sprites, &cfg.sally),
            Character::Exe => match self.exe_character {
                ExeCharacter::Chaos => self.chaos_animate_state(sprites, cfg),
                ExeCharacter::Exetior => self.exetior_animate_state(sprites, &cfg.exetior),
                ExeCharacter::Exeller => self.exeller_animate_state(sprites, &cfg.exeller),
                ExeCharacter::Original => self.exe_animate_state(sprites, &cfg.exe),
            },
        };
        if !handled_by_character {
            self.animate_shared_state(sprites, cfg);
        }
    }

    fn animate_shared_state(&mut self, sprites: &Sprites, cfg: &GameplayConfig) {
        let shared = &cfg.physics.animation;
        match self.state {
            IDLE | EMOTION1 | EMOTION2 | EMOTION3 => self.image_speed = 1.0,
            HURT => {
                self.image_speed = 0.0;
                self.image_index = 0.0;
            }
            BALANCING => {
                self.image_xscale = self.edge_dir;
                self.image_speed = 1.0;
            }
            FALL => self.image_index = if self.yspd > 0.0 { 1.0 } else { 0.0 },
            WALK => self.image_speed = self.xspd.abs() / self.max_h_speed,
            RUN | JUMP => self.image_speed = (self.xspd.abs() / shared.run_speed_divisor).max(shared.run_min_speed),
            DEAD => self.animate_dead(sprites, shared.demonized_dead_speed),
            LOOKUP | LOOKDOWN => self.animate_look(shared.look_speed),
            SPIN => self.image_speed = self.xspd.abs() / shared.spin_speed_divisor,
            _ => {}
        }
    }

    /// Falling pose chosen by direction: frame 1 while falling, frame 0 while rising.
    pub(super) fn animate_fall_by_direction(&mut self, speed: f64) {
        self.image_speed = speed;
        self.image_index = if self.yspd > 0.0 { 1.0 } else { 0.0 };
    }

    /// Looking up or down stops on the second frame.
    pub(super) fn animate_look(&mut self, speed: f64) {
        self.image_speed = speed;
        if self.image_index >= 1.0 {
            self.image_index = 1.0;
        }
    }

    fn animate_dead(&mut self, sprites: &Sprites, demonized_speed: f64) {
        if self.is_demonized() {
            self.image_speed = demonized_speed;
            return;
        }
        self.image_speed = if self.is_grounded { 1.0 } else { 0.0 };
        let last_frame = sprites.get(self.sprite_index).frame_count as f64 - 1.0;
        if self.image_index >= last_frame {
            self.image_index = last_frame;
        }
    }

    /// Draw_0 keeps the speed trail going: a fading copy every 3 ticks.
    pub(super) fn update_trail(&mut self, events: &mut Vec<SimEvent>) {
        if self.effect_time <= 0 {
            return;
        }
        self.effect_time -= 1;
        // obj_chaos Draw_0: Chaos also sheds a piece every fifth or sixth step. The
        // original picks between the two at random; the step's own parity does here.
        if self.is_exe(super::ExeCharacter::Chaos) && self.effect_time % crate::core::config::original_ticks(5 + self.effect_time % 2).max(1) == 0 {
            events.push(SimEvent::ChaosLiquid { x: self.x, y: self.y });
        }
        if self.effect_time % crate::core::config::original_ticks(3).max(1) == 0 {
            events.push(SimEvent::Trail {
                sprite: self.sprite_index,
                image_index: self.image_index,
                x: self.x,
                y: self.y,
                xscale: self.image_xscale,
            });
        }
    }
}

/// Where a held taunt starts over (Other_7 of each character), by emotion 0 to 2.
fn emotion_loop_frame(character: Character, exe: ExeCharacter, emotion: usize, frame_count: f64) -> f64 {
    match (character, exe, emotion) {
        (Character::Tails, _, 2) => frame_count - 8.0,
        (Character::Amy, _, 2) => frame_count - 10.0,
        (Character::Knux, _, 0 | 2) | (Character::Sally, _, 0) => 2.0,
        (Character::Cream, _, 0) => 1.0,
        (Character::Exe, ExeCharacter::Chaos, 0) => frame_count - 1.0,
        (Character::Exe, ExeCharacter::Exetior, 0) => 17.0,
        _ => 0.0,
    }
}
