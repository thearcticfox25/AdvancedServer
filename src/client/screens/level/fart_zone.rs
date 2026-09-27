//! Fart Zone as clients show it: the numbers popping off the training dummy
//! (obj_fart_text) and the end of a cursed player's game (obj_fart_ass). The server
//! moves the dummy and passes the curse (sim level/dummy.rs, player/fart_zone.rs).

use crate::client::canvas::{make_color_rgb, Canvas, TextFont};
use crate::core::resources::names::sprite;

/// obj_fart_text: made at depth 0.
pub const TEXT_DEPTH: i32 = 0;
/// It rises at 5 px a step, slowing by 0.15, waits a second, then shrinks away.
const RISE_SPEED: f64 = 5.0;
const RISE_SLOWING: f64 = 0.15;
const WAIT_SECONDS: f64 = 1.0;
const SHRINK_RISE: f64 = 0.05;
const SHRINK_TARGET: f64 = 1.05;
/// Its colour shifts between these, a radian every 100 ms.
const COLOUR_A: u32 = make_color_rgb(0xee, 0x94, 0x28);
const COLOUR_B: u32 = make_color_rgb(0x87, 0x4d, 0x05);
use crate::core::config::ticks_per_second;
use crate::core::config::ticks;
use crate::core::config::step;
const COLOUR_MILLISECONDS: f64 = 100.0;
/// obj_fart_ass: the boom shows half a second before the game ends.
const BOOM_SECONDS: f64 = 0.5;

#[derive(Default)]
pub struct FartZone {
    texts: Vec<Text>,
    /// Ticks until the game ends, and the boom's frame.
    boom: Option<(i32, f64)>,
}

struct Text {
    text: String,
    x: f64,
    y: f64,
    speed: f64,
    wait: Option<i32>,
    scale: f64,
}

impl FartZone {
    /// A hit worth `worth` seconds of stun (or health, when negative).
    pub fn show_hit(&mut self, worth: i32, x: f64, y: f64) {
        let text = match worth {
            0 => "0".to_string(),
            seconds if seconds > 0 => format!("+{seconds}s"),
            health => health.to_string(),
        };
        self.texts.push(Text { text, x, y, speed: RISE_SPEED, wait: None, scale: 1.0 });
    }

    pub fn boom(&mut self) {
        self.boom.get_or_insert((crate::core::config::ticks(BOOM_SECONDS), 0.0));
    }

    /// Returns true once the game should end.
    pub fn step(&mut self, boom_fps: f64) -> bool {
        self.texts.retain_mut(|text| {
            text.y -= text.speed;
            match text.wait.as_mut() {
                None => {
                    text.speed -= RISE_SLOWING * step();
                    if text.speed < 0.0 {
                        text.speed = 0.0;
                        text.wait = Some(ticks(WAIT_SECONDS));
                    }
                    true
                }
                Some(wait) if *wait > 0 => {
                    *wait -= 1;
                    true
                }
                Some(_) => {
                    text.y -= SHRINK_RISE * step();
                    text.scale -= (SHRINK_TARGET - text.scale) * step();
                    text.scale > 0.0
                }
            }
        });
        let Some((ticks, frame)) = self.boom.as_mut() else { return false };
        *frame += boom_fps / ticks_per_second();
        *ticks -= 1;
        *ticks <= 0
    }

    pub fn draw_texts(&self, canvas: &mut Canvas, current_time_ms: f64) {
        let mix = ((current_time_ms / COLOUR_MILLISECONDS).sin() + 1.0) / 2.0;
        let colour = merge_colour(COLOUR_A, COLOUR_B, mix);
        for text in &self.texts {
            let (width, height) = canvas.font_text_size(TextFont::Big, &text.text);
            let (left, top) = (text.x - width * text.scale / 2.0, text.y - height * text.scale / 2.0);
            canvas.draw_font_text(TextFont::Big, left, top, &text.text, colour, 1.0, text.scale);
        }
    }

    pub fn draw_gui(&self, canvas: &mut Canvas) {
        if let Some((_, frame)) = self.boom {
            canvas.draw_sprite(sprite::SPR_BOOM, frame, 0.0, 0.0);
        }
    }
}

/// merge_color
fn merge_colour(a: u32, b: u32, amount: f64) -> u32 {
    let channel = |shift: u32| {
        let (from, to) = (((a >> shift) & 0xFF) as f64, ((b >> shift) & 0xFF) as f64);
        ((from + (to - from) * amount).round() as u32) << shift
    };
    channel(0) | channel(8) | channel(16)
}
