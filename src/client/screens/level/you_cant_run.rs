//! You Can't Run's gas as clients show it: smoke puffs (obj_ycr_smoke) over the areas
//! the gas fills, made when it comes and fading when it clears.

use crate::client::canvas::Canvas;
use macroquad::rand::gen_range;
use crate::core::resources::names::sprite;
use crate::core::config::ticks_per_second;
use crate::core::objects::ids::ObjectId;
use crate::core::world::{ObjectVars, World, C_WHITE};
use crate::core::config::step;

/// obj_ycr_smoke: depth -21.
pub const SMOKE_DEPTH: i32 = -21;
const FADE_IN_PER_STEP: f64 = 0.0025;
const FADE_OUT_PER_STEP: f64 = 0.01;
/// x = xP + sin(current_time / 300 + xOff) * 4, xOff = random_range(-20, 20).
const SWAY_MILLISECONDS_PER_RADIAN: f64 = 300.0;
const SWAY: f64 = 4.0;
const SWAY_PHASE: f64 = 20.0;

#[derive(Default)]
pub struct YouCantRun {
    gassed: Option<u8>,
    puffs: Vec<Puff>,
}

struct Puff {
    x: f64,
    y: f64,
    phase: f64,
    image_index: f64,
    alpha: f64,
    clearing: bool,
}

impl YouCantRun {
    /// The puffs' Step, and SERVER_YCRSMOKE_STATE as the world now shows it.
    /// Returns the middles of the areas the gas just filled, where snd_smoke plays.
    pub fn step(&mut self, world: &World) -> Vec<(f64, f64)> {
        let smoke = world.sprites.get(sprite::SPR_SMOKE);
        for puff in &mut self.puffs {
            puff.image_index += smoke.fps as f64 / ticks_per_second();
            if puff.clearing {
                puff.alpha -= FADE_OUT_PER_STEP * step();
            } else if puff.alpha < 1.0 {
                puff.alpha += FADE_IN_PER_STEP * step();
            }
        }
        self.puffs.retain(|puff| !(puff.clearing && puff.alpha <= 0.0));

        let areas = || world.ids_of(ObjectId::YcrSmokearea).filter_map(|area| match world.instances[area].vars {
            ObjectVars::SmokeArea { nid, gassed } => Some((area, nid, gassed)),
            _ => None,
        });
        let gassed = areas().find(|&(_, _, gassed)| gassed).map(|(_, nid, _)| nid);
        if gassed == self.gassed {
            return Vec::new();
        }
        self.gassed = gassed;
        let Some(gassed) = gassed else {
            self.puffs.iter_mut().for_each(|puff| puff.clearing = true);
            return Vec::new();
        };
        let mut sounds = Vec::new();
        for (area, _, _) in areas().filter(|&(_, nid, _)| nid == gassed) {
            let Some(bbox) = world.bbox(area) else { continue };
            sounds.push(((bbox.left + bbox.right) / 2.0, (bbox.top + bbox.bottom) / 2.0));
            let (width, height) = (smoke.width as usize, smoke.height as usize);
            for column in (0..(bbox.right - bbox.left) as usize).step_by(width) {
                for row in (0..(bbox.bottom - bbox.top) as usize).step_by(height) {
                    self.puffs.push(Puff {
                        x: bbox.left + column as f64,
                        y: bbox.top + row as f64,
                        phase: gen_range(-SWAY_PHASE, SWAY_PHASE),
                        image_index: gen_range(0, smoke.frame_count + 1) as f64,
                        alpha: 0.0,
                        clearing: false,
                    });
                }
            }
        }
        sounds
    }

    pub fn draw(&self, canvas: &mut Canvas, current_time_ms: f64) {
        for puff in &self.puffs {
            let x = puff.x + (current_time_ms / SWAY_MILLISECONDS_PER_RADIAN + puff.phase).sin() * SWAY;
            canvas.draw_sprite_ext(sprite::SPR_SMOKE, puff.image_index, x, puff.y, 1.0, 1.0, 0.0, C_WHITE, puff.alpha);
        }
    }
}
