//! One-shot animations of a level: obj_quickeffect (plays once) and
//! obj_quickeffect_fade (fades out, the copies a running character leaves behind).

use crate::client::canvas::Canvas;
use crate::client::palette::Colours;
use crate::core::resources::sprites::Sprites;
use crate::core::resources::SpriteId;
use crate::core::config::ticks_per_second;
use crate::core::objects::ids::ObjectId;
use crate::core::world::{World, C_WHITE};
use crate::core::config::step;

/// obj_quickeffect and obj_quickeffect_fade: depth = -200.
pub const EFFECT_DEPTH: i32 = -200;
/// A trail copy stands behind its character (depth + 10).
pub const TRAIL_BEHIND: i32 = 10;
/// scr_effect_fade_quick: effSpeed = 0.05.
const TRAIL_FADE_PER_STEP: f64 = 0.05;

pub struct Effect {
    pub depth: i32,
    sprite: SpriteId,
    x: f64,
    y: f64,
    xscale: f64,
    image_index: f64,
    image_speed: f64,
    /// vspeed: the effect drifts up or down.
    yspd: f64,
    blend: u32,
    alpha: f64,
    /// obj_quickeffect_fade: alpha lost per step, and the colours of the character.
    fade: Option<(f64, Colours, Colours)>,
    /// obj_chaos_liquid: how fast it is still falling towards the floor.
    falling: Option<f64>,
}

impl Effect {
    /// net_quick_effect / scr_effect_quick: plays the sprite once.
    pub fn quick(sprite: SpriteId, x: f64, y: f64, xscale: f64, image_speed: f64, blend: u32) -> Effect {
        Effect { depth: EFFECT_DEPTH, sprite, x, y, xscale, image_index: 0.0, image_speed, yspd: 0.0, blend, alpha: 1.0, fade: None, falling: None }
    }

    /// vspeed of net_quick_effect.
    pub fn set_yspd(&mut self, yspd: f64) {
        self.yspd = yspd;
    }

    /// scr_effect_fade_quick of a character's Draw event: a still copy that fades out.
    pub fn trail(sprite: SpriteId, image_index: f64, x: f64, y: f64, xscale: f64, depth: i32, colours: (Colours, Colours), blend: u32) -> Effect {
        let (from, to) = colours;
        Effect { depth, sprite, x, y, xscale, image_index, image_speed: 0.0, yspd: 0.0, blend, alpha: 1.0, fade: Some((TRAIL_FADE_PER_STEP, from, to)), falling: None }
    }

    /// obj_chaos_liquid: a piece of Chaos, falling at one of the speeds it chooses
    /// between, in the colours of the Chaos it came off.
    pub fn liquid(sprite: SpriteId, x: f64, y: f64, speed: f64, depth: i32, colours: (Colours, Colours), blend: u32) -> Effect {
        let (from, to) = colours;
        Effect {
            depth,
            sprite,
            x,
            y,
            xscale: 1.0,
            image_index: 0.0,
            image_speed: 0.0,
            yspd: 0.0,
            blend,
            alpha: 1.0,
            // The colours are the palette swap; a drop never fades, so the rate is 0.
            fade: Some((0.0, from, to)),
            falling: Some(speed),
        }
    }

    /// One step: the image moves on, then the Step event and the motion. False once the effect destroys itself.
    pub fn step(&mut self, sprites: &Sprites, world: &World) -> bool {
        let meta = sprites.get(self.sprite);
        // obj_chaos_liquid Step: it drops until it meets the floor, and only then breaks.
        if let Some(speed) = self.falling {
            if world.position_meeting(self.x, self.y, ObjectId::FloorParent) {
                self.falling = None;
                self.image_speed = 1.0;
            } else {
                self.y += speed * step();
                return true;
            }
        }
        self.image_index += self.image_speed * meta.fps as f64 / ticks_per_second();
        self.y += self.yspd;
        match &self.fade {
            Some((fade_per_step, _, _)) if *fade_per_step > 0.0 => {
                self.alpha = (self.alpha - fade_per_step * step()).clamp(0.0, 1.0);
                self.alpha > 0.0
            }
            Some(_) => meta.frame_count == 0 || self.image_index <= meta.frame_count as f64 - 1.0,
            None => meta.frame_count == 0 || self.image_index <= meta.frame_count as f64 - 1.0,
        }
    }

    pub fn draw(&self, canvas: &mut Canvas) {
        if let Some((_, from, to)) = &self.fade {
            canvas.set_palette_swap(from, to);
        }
        canvas.draw_sprite_ext(self.sprite, self.image_index, self.x, self.y, self.xscale, 1.0, 0.0, self.blend, self.alpha);
        if self.fade.is_some() {
            canvas.reset_shader();
        }
    }
}

/// obj_heal_particle: a sparkle rising and swaying over a teammate being healed.
pub struct HealSparkle {
    frame: f64,
    start_x: f64,
    x: f64,
    y: f64,
    rise: f64,
    sway: f64,
    phase_ms: f64,
    fade: f64,
    alpha: f64,
}

/// obj_heal_particle: depth = -10.
pub const HEAL_SPARKLE_DEPTH: i32 = -10;

impl HealSparkle {
    /// Create_0: a random frame, speed, sway and fade.
    pub fn new(x: f64, y: f64) -> HealSparkle {
        use macroquad::rand::{gen_range, ChooseRandom};
        HealSparkle {
            frame: *[0.0, 1.0].choose().unwrap(),
            start_x: x,
            x,
            y,
            rise: gen_range(-2.0, -1.0),
            sway: *[4.0, 5.0, 6.0, 7.0, 8.0].choose().unwrap(),
            phase_ms: gen_range(0.0, 9999.0),
            fade: *[0.016, 0.032, 0.064].choose().unwrap(),
            alpha: 1.0,
        }
    }

    /// Step_0. False once it has faded away.
    pub fn step(&mut self, current_time_ms: f64) -> bool {
        self.alpha -= self.fade * step();
        if self.alpha <= 0.0 {
            return false;
        }
        self.y += self.rise;
        self.x = self.start_x + ((current_time_ms + self.phase_ms) / 300.0).sin() * self.sway;
        true
    }

    pub fn draw(&self, canvas: &mut Canvas) {
        canvas.draw_sprite_ext(crate::core::resources::names::sprite::SPR_HEAL_PART, self.frame, self.x, self.y, 1.0, 1.0, 0.0, C_WHITE, self.alpha);
    }
}

/// scr_effect_quick in Act 9 makes every effect black.
pub fn effect_blend(act9: bool) -> u32 {
    if act9 {
        0x000000
    } else {
        C_WHITE
    }
}
