//! Echidna Ruins as clients show it (obj_marijuna_crystalcontroller and friends): the
//! crystals' glow and chime, the judgers' growl, the screen turning inside out around a
//! player whose controls a crystal reversed, and the statues and static once the big ring
//! is out. The server lights the crystals and wakes the judgers (sim level/echidna_ruins.rs).

use crate::client::canvas::Canvas;
use crate::core::resources::names::{sound, sprite};
use crate::core::resources::SoundId;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Character, Player};
use crate::core::world::{InstanceId, ObjectVars, World, C_WHITE};
use macroquad::rand::gen_range;
use crate::core::config::step;
use crate::core::config::ticks;

/// swapControls: the circle grows from the crystal, speeding up, and shrinks around
/// the player for the last 1.5 seconds.
const CIRCLE_START_SPEED: f64 = 0.016 * 400.0;
const CIRCLE_ACCELERATION: f64 = 0.016 * 4.0;
const SHRINK_FROM_SECONDS: f64 = 1.5;
/// Draw GUI: the sad face over the player, higher for the taller characters.
const FACE_ABOVE: f64 = 20.0;
const FACE_ABOVE_TALL: f64 = 25.0;
const FACE_ABOVE_EGGMAN: f64 = 36.0;
/// obj_marijuna_statue and obj_marjiuna_static fade by this a step; the static shows at a tenth.
const FADE: f64 = 0.016;
const STATIC_ALPHA: f64 = 0.1;
/// The frame one random statue changes to when the big ring comes.
const CHOSEN_STATUE_FRAME: f64 = 2.0;

pub struct EchidnaRuins {
    seen: Vec<(InstanceId, bool, u8)>,
    circle: (f64, f64, f64),
    speed: f64,
    /// Since the big ring came: the static's fade.
    static_fade: Option<f64>,
}

impl EchidnaRuins {
    pub fn open(world: &World) -> EchidnaRuins {
        EchidnaRuins { seen: states(world), circle: (0.0, 0.0, 0.0), speed: CIRCLE_START_SPEED, static_fade: None }
    }

    /// swapControls, on this player's client.
    pub fn reversed(&mut self, x: f64, y: f64) {
        self.circle = (x, y, 0.0);
        self.speed = CIRCLE_START_SPEED;
    }

    /// Returns the sounds heard from places (measured from the left ear).
    pub fn step(&mut self, canvas: &mut Canvas, world: &mut World, own: Option<&Player>, view: (f64, f64), big_ring_out: bool) -> Vec<(SoundId, f64, f64)> {
        let mut heard = Vec::new();
        let now = states(world);
        for (&(id, lit, state), &(_, was_lit, was_state)) in now.iter().zip(&self.seen) {
            let (x, y) = (world.instances[id].x, world.instances[id].y);
            // SERVER_MJCRYSTAL_STATE and SERVER_MJJUDGER_STATE.
            if lit && !was_lit {
                heard.push((sound::SND_DESTINY, x, y));
            }
            if state > 0 && state != was_state {
                heard.push((sound::SND_JUDGER, x, y));
            }
        }
        self.seen = now;

        // obj_marijuna_crystalcontroller Step
        canvas.inverse_circle = None;
        if let Some(own) = own.filter(|_| !world.game_ends) {
            let (x, y, size) = &mut self.circle;
            if own.controls_reversed > ticks(SHRINK_FROM_SECONDS) {
                if *size < world.room_height {
                    *size += self.speed * step();
                } else {
                    (*x, *y, *size) = (own.x, own.y, world.room_height);
                }
            } else {
                (*x, *y) = (own.x, own.y);
                *size = if *size > 0.0 { *size - self.speed } else { 0.0 };
            }
            self.speed += CIRCLE_ACCELERATION * step();
            canvas.inverse_circle = Some((*x - view.0, *y - view.1, *size));
        }

        // Once the big ring is out a statue changes, all fade into their other look and static comes.
        if big_ring_out && self.static_fade.is_none() {
            self.static_fade = Some(0.0);
            let statues: Vec<InstanceId> = world.ids_of(ObjectId::MarijunaStatue).collect();
            if !statues.is_empty() {
                world.instances[statues[gen_range(0, statues.len())]].image_index = CHOSEN_STATUE_FRAME;
            }
        }
        if let Some(fade) = self.static_fade.as_mut() {
            *fade = (*fade + FADE).min(1.0);
            let statues: Vec<InstanceId> = world.ids_of(ObjectId::MarijunaStatue).collect();
            for statue in statues {
                let instance = &mut world.instances[statue];
                instance.image_alpha = (instance.image_alpha - FADE).max(0.0);
            }
        }
        heard
    }

    /// obj_marjiuna_static Draw
    pub fn draw_static(&self, canvas: &mut Canvas, view: (f64, f64), current_time_ms: f64) {
        if let Some(fade) = self.static_fade {
            let frame = current_time_ms * canvas.sprites.get(sprite::SPR_STATIC).fps as f64 / 1000.0;
            canvas.draw_sprite_ext(sprite::SPR_STATIC, frame, view.0, view.1, 1.0, 1.0, 0.0, C_WHITE, fade * STATIC_ALPHA);
        }
    }

    /// obj_marijuna_crystalcontroller Draw GUI: the sad face while the controls are reversed.
    pub fn draw_gui(&self, canvas: &mut Canvas, own: Option<&Player>, view: (f64, f64)) {
        let Some(own) = own.filter(|own| own.controls_reversed > 0) else { return };
        let above = match own.character {
            Character::Sally | Character::Knux | Character::Amy => FACE_ABOVE_TALL,
            Character::Eggman => FACE_ABOVE_EGGMAN,
            _ => FACE_ABOVE,
        };
        canvas.draw_sprite(sprite::SPR_MARIJUNA_BOOHOO, 0.0, (own.x - view.0).ceil(), (own.y - view.1).ceil() - above);
    }
}

/// Each crystal's and judger's state as the world shows it now.
fn states(world: &World) -> Vec<(InstanceId, bool, u8)> {
    world
        .ids_of(ObjectId::MarijunaCrystal)
        .chain(world.ids_of(ObjectId::MarijunaJudger))
        .map(|id| match world.instances[id].vars {
            ObjectVars::MjCrystal { lit, .. } => (id, lit, 0),
            ObjectVars::Judger { state } => (id, false, state),
            _ => (id, false, 0),
        })
        .collect()
}
