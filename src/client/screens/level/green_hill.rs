//! Green Hill's weather: obj_ghz_controller's rain and its sound. The sky drifting
//! past is map data (the Background layer: parallax 0, hspeed -1).

use crate::client::audio::Audio;
use crate::client::canvas::Canvas;
use macroquad::rand::gen_range;
use crate::core::resources::names::{sound, sprite};
use crate::core::config::step;

/// obj_ghz_rain: depth -200.
pub const RAIN_DEPTH: i32 = -200;
/// obj_ghz_controller Step: the drops made each step, from above the view and from
/// its right side, twice.
const DROPS_FROM_ABOVE: usize = 3;
const DROPS_FROM_THE_RIGHT: usize = 2;
const ABOVE_SPAWN_X: (f64, f64) = (-120.0, 560.0);
const RIGHT_SPAWN_X: f64 = 481.0;
const RIGHT_SPAWN_Y: (f64, f64) = (-120.0, 350.0);
/// obj_ghz_rain Step: y += 5 + spd, x -= 3 + spd; gone 310 px below the view's top.
const FALL: f64 = 5.0;
const DRIFT: f64 = 3.0;
const GONE_BELOW: f64 = 310.0;

pub struct GreenHill {
    drops: Vec<RainDrop>,
}

struct RainDrop {
    x: f64,
    y: f64,
    /// spd = random_range(0, -1)
    speed: f64,
}

impl GreenHill {
    /// Create of obj_ghz_controller.
    pub fn open(audio: &mut Audio) -> GreenHill {
        audio.play(sound::SND_RAIN, true);
        GreenHill { drops: Vec::new() }
    }

    /// Step of the drops, then of the controller, which makes new ones around the view.
    pub fn step(&mut self, view: (f64, f64)) {
        for drop in &mut self.drops {
            drop.y += (FALL + drop.speed) * step();
            drop.x -= (DRIFT + drop.speed) * step();
        }
        self.drops.retain(|drop| drop.y < view.1 + GONE_BELOW);
        let drop = |x, y| RainDrop { x, y, speed: gen_range(-1.0, 0.0) };
        for _ in 0..2 {
            self.drops.extend((0..DROPS_FROM_ABOVE).map(|_| drop(view.0 + gen_range(ABOVE_SPAWN_X.0, ABOVE_SPAWN_X.1), view.1 - 1.0)));
            self.drops.extend((0..DROPS_FROM_THE_RIGHT).map(|_| drop(view.0 + RIGHT_SPAWN_X, view.1 + gen_range(RIGHT_SPAWN_Y.0, RIGHT_SPAWN_Y.1))));
        }
    }

    pub fn draw_rain(&self, canvas: &mut Canvas) {
        for drop in &self.drops {
            canvas.draw_sprite(sprite::SPR_GHZ_RAIN, 0.0, drop.x, drop.y);
        }
    }
}
