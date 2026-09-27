//! Full-screen fades several rooms create: obj_blackfadeout, obj_blackfadein and
//! the white flash obj_lobby_white.

use crate::client::canvas::Canvas;
use crate::core::resources::names::sprite;
use crate::core::world::C_WHITE;
use crate::core::config::step;

const BLACK_FADE_PER_STEP: f64 = 0.02;
const WHITE_FLASH_FADE_PER_STEP: f64 = 0.1;

/// Black over everything, fading away (obj_blackfadeout, drawn in Draw GUI).
pub struct BlackFadeOut {
    alpha: f64,
}

impl BlackFadeOut {
    pub fn new() -> BlackFadeOut {
        BlackFadeOut { alpha: 1.0 }
    }

    /// Returns false once the fade is over and the object destroys itself.
    pub fn step(&mut self) -> bool {
        self.alpha -= BLACK_FADE_PER_STEP * step();
        self.alpha > 0.0
    }

    pub fn draw_gui(&self, canvas: &mut Canvas) {
        canvas.draw_sprite_ext(sprite::SPR_BLACK, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, C_WHITE, self.alpha);
    }
}

/// Black over everything, growing (obj_blackfadein, drawn in Draw GUI).
pub struct BlackFadeIn {
    alpha: f64,
}

impl BlackFadeIn {
    pub fn new() -> BlackFadeIn {
        BlackFadeIn { alpha: 0.0 }
    }

    pub fn step(&mut self) {
        if self.alpha < 1.0 {
            self.alpha += BLACK_FADE_PER_STEP * step();
        }
    }

    pub fn draw_gui(&self, canvas: &mut Canvas) {
        canvas.draw_sprite_ext(sprite::SPR_BLACK, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, C_WHITE, self.alpha);
    }
}

/// A white flash that fades quickly (obj_lobby_white).
pub struct WhiteFlash {
    alpha: f64,
}

impl WhiteFlash {
    pub fn new() -> WhiteFlash {
        WhiteFlash { alpha: 1.0 }
    }

    /// Returns false once the flash is over and the object destroys itself.
    pub fn step(&mut self) -> bool {
        self.alpha -= WHITE_FLASH_FADE_PER_STEP * step();
        self.alpha > 0.0
    }

    pub fn draw(&self, canvas: &mut Canvas) {
        canvas.draw_sprite_ext(sprite::SPR_WHITE, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, C_WHITE, self.alpha);
    }
}
