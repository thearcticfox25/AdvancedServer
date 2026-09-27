//! Limp City as clients show it: the chains' warning and shock sounds, the eyes
//! swinging on their chains and tilting, the pupil of a watched eye following the nearest
//! player, the camera looking through it, and the red over the city
//! (obj_redring_screen2). The server runs the chains and eyes (sim level/limp_city.rs).

use crate::client::canvas::Canvas;
use crate::core::resources::names::{sound, sprite};
use crate::core::resources::SoundId;
use crate::core::level::{hang_eyes, ChainState};
use crate::core::objects::ids::ObjectId;
use crate::core::world::{InstanceId, ObjectVars, World, C_WHITE};
use macroquad::rand::gen_range;
use crate::core::config::step;
use crate::core::config::ticks;

/// obj_redring_screen2 and the eyes' depths.
pub const RED_DEPTH: i32 = -500;
pub const PUPIL_DEPTH: i32 = 90;
const RED_ALPHA: f64 = 0.1;
/// obj_limpcity_eyeA and B: every second a new tilt, which straightens by half a degree a step.
const TILT_EVERY_SECONDS: f64 = 1.0;
const TILTS: [f64; 4] = [-16.0, 16.0, -8.0, 8.0];
const TILT_BACK: f64 = 0.5;
/// obj_limpcity_eyeB Draw: the pupil looks at the nearest player within this.
const PUPIL_SIGHT: f64 = 270.0;
const PUPIL_SEARCH: f64 = 512.0;
const PUPIL_FRAME: f64 = 5.0;
/// obj_soundemitter: stype SOUNDEMT_LCCHAIN, sounding 16 px above it.
const CHAIN_EMITTER: i64 = 1;
const EMITTER_ABOVE: f64 = 16.0;

pub struct LimpCity {
    chains_seen: u8,
    /// Each eye's tilt and its timer.
    tilts: Vec<(InstanceId, f64, i32)>,
}

impl LimpCity {
    pub fn open(world: &World) -> LimpCity {
        let eyes = world.ids_of(ObjectId::LimpcityEyea).chain(world.ids_of(ObjectId::LimpcityEyeb));
        LimpCity { chains_seen: 0, tilts: eyes.map(|eye| (eye, 0.0, ticks(TILT_EVERY_SECONDS))).collect() }
    }

    /// Returns the sounds heard from places (measured from the view's centre).
    pub fn step(&mut self, world: &mut World, current_time_ms: f64) -> Vec<(SoundId, f64, f64)> {
        let mut heard = Vec::new();
        let state = world.ids_of(ObjectId::LimpcityEchain1).find_map(|id| match world.instances[id].vars {
            ObjectVars::ElectricChain { state } => Some(state),
            _ => None,
        });
        // SERVER_LCCHAIN_STATE 0 and 1.
        if let Some(state) = state.filter(|&state| state != self.chains_seen) {
            let sound = match state {
                state if state == ChainState::Warning as u8 => Some(sound::SND_ECHAIN_PREPARE),
                state if state == ChainState::Shocking as u8 => Some(sound::SND_ECHAIN),
                _ => None,
            };
            for emitter in world.ids_of(ObjectId::Soundemitter).filter(|&id| world.instances[id].vars == (ObjectVars::SoundEmitter { kind: CHAIN_EMITTER })) {
                if let Some(sound) = sound {
                    heard.push((sound, world.instances[emitter].x, world.instances[emitter].y - EMITTER_ABOVE));
                }
            }
            self.chains_seen = state;
        }

        hang_eyes(world, current_time_ms);
        for (eye, tilt, timer) in &mut self.tilts {
            *timer -= 1;
            if *timer < 0 {
                *tilt = TILTS[gen_range(0, TILTS.len())];
                *timer = ticks(TILT_EVERY_SECONDS);
            }
            world.instances[*eye].image_angle = *tilt;
            if *tilt != 0.0 {
                *tilt -= TILT_BACK * tilt.signum() * step();
            }
        }
        heard
    }

    /// obj_limpcity_eyeB Draw: a watched eye's pupil turns to the nearest player it sees.
    pub fn draw_pupils(&self, canvas: &mut Canvas, world: &World, players: &[(f64, f64)]) {
        for eye in world.ids_of(ObjectId::LimpcityEyeb) {
            let instance = &world.instances[eye];
            if !matches!(instance.vars, ObjectVars::Eye { used: true, .. }) {
                continue;
            }
            let distance = |&(x, y): &(f64, f64)| (x - instance.x).hypot(y - instance.y);
            let nearest = players.iter().filter(|player| distance(player) < PUPIL_SEARCH).min_by(|a, b| distance(a).total_cmp(&distance(b)));
            let seen = nearest.filter(|player| distance(player) < PUPIL_SIGHT);
            let angle = seen.map_or(0.0, |&(x, y)| (instance.y - y).atan2(x - instance.x).to_degrees());
            let pupil = if seen.is_some() { sprite::SPR_LIMPCITY_EYELID } else { sprite::SPR_LIMPCITY_EYE_RECHARGE };
            canvas.draw_sprite_ext(pupil, PUPIL_FRAME, instance.x, instance.y, 1.0, 1.0, angle, C_WHITE, instance.image_alpha);
        }
    }

    pub fn draw_red(&self, canvas: &mut Canvas, view: (f64, f64), current_time_ms: f64) {
        let frame = current_time_ms * canvas.sprites.get(sprite::SPR_REDRING_FORE).fps as f64 / 1000.0;
        canvas.draw_sprite_ext(sprite::SPR_REDRING_FORE, frame, view.0, view.1, 1.0, 1.0, 0.0, C_WHITE, RED_ALPHA);
    }
}
