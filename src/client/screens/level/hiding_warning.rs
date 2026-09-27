//! obj_player_warning: what a survivor who never leaves one screenful of the map is
//! shown. The rule behind it is the simulation's (player/movement.rs warn_about_hiding,
//! which also does the slowing); this is only the screen it puts up: the music goes
//! quiet, a line of red letters wobbles in the middle, and once in a while the game
//! borrows Terraria's night warnings in green instead.

use crate::client::audio::Audio;
use crate::client::canvas::{make_color_rgb, Canvas, TextFont, C_RED, VIEW_WIDTH};
use crate::client::text::draw_text;
use crate::core::resources::names::sound;
use crate::core::config::step;
use macroquad::rand::gen_range;

/// The lines obj_player_warning chooses between.
const LINES: [&str; 6] = [
    "the fear of leaving the shelter has shackled you",
    "your legs refuse to move...",
    "you try to be careful about your movements",
    "the fear of unknown disarms you",
    "your limbs are cramping with fear...",
    "you feel way too immobilized",
];
/// irandom_range(0, 50) == 20: one warning in fifty one quotes Terraria instead.
const TERRARIA_LINES: [&str; 5] = [
    "You feel an evil presence watching you...",
    "Impending doom approaches...",
    "You feel vibrations from deep below...",
    "The air is getting colder around you...",
    "This is going to be a terrible night...",
];
const TERRARIA_CHANCE: i32 = 51;
const TERRARIA_COLOUR: u32 = make_color_rgb(0x32, 0xFF, 0x82);
const TERRARIA_POSITION: (f64, f64) = (4.0, 180.0);
const BLACK: u32 = 0x000000;

const FADE_PER_STEP: f64 = 0.016;
/// The letters stand 8 px apart, waving around a line this far down the screen.
const LETTER_SPACING: f64 = 8.0;
const LINE_Y: f64 = 160.0;
const WAVE_PER_LETTER: f64 = 10.0;
const WAVE_PER_STEP: f64 = 0.5;
const SHAKE: f64 = 2.0;
const WAVE_JITTER: f64 = 10.0;
/// audio_sound_gain(global.music, ...): out while the warning holds, back when it goes.
const MUSIC_QUIET_SECONDS: f64 = 2.0;
const MUSIC_BACK_SECONDS: f64 = 0.5;

pub struct HidingWarning {
    text: &'static str,
    terraria: bool,
    fade: f64,
    /// The player moved on: the words fade out and the warning is done.
    leaving: bool,
    wave: f64,
}

impl HidingWarning {
    /// Create: the line, the music turned down and the sound that comes with it.
    pub fn new(audio: &mut Audio) -> HidingWarning {
        let terraria = gen_range(0, TERRARIA_CHANCE) == 20;
        let lines: &[&'static str] = if terraria { &TERRARIA_LINES } else { &LINES };
        audio.fade_music(true, MUSIC_QUIET_SECONDS);
        audio.play(if terraria { sound::SND_BOS } else { sound::SND_FNAC }, terraria);
        HidingWarning { text: lines[gen_range(0, lines.len())], terraria, fade: 0.0, leaving: false, wave: 0.0 }
    }

    pub fn leave(&mut self) {
        self.leaving = true;
    }

    /// Draw_75 and CleanUp: false once the words are gone and the music is back.
    pub fn draw(&mut self, canvas: &mut Canvas, audio: &mut Audio) -> bool {
        if self.leaving {
            self.fade -= FADE_PER_STEP * step();
            if self.fade <= 0.0 {
                audio.stop(sound::SND_BOS);
                audio.fade_music(false, MUSIC_BACK_SECONDS);
                return false;
            }
        } else {
            self.fade = (self.fade + FADE_PER_STEP * step()).min(1.0);
        }

        if self.terraria {
            // Four black copies around the words make the outline the original draws.
            for (dx, dy) in [(-1.0, 0.0), (0.0, 1.0), (1.0, 0.0), (0.0, -1.0)] {
                canvas.draw_font_text(TextFont::Big, TERRARIA_POSITION.0 + dx, TERRARIA_POSITION.1 + dy, self.text, BLACK, self.fade, 1.0);
            }
            canvas.draw_font_text(TextFont::Big, TERRARIA_POSITION.0, TERRARIA_POSITION.1, self.text, TERRARIA_COLOUR, self.fade, 1.0);
        } else {
            let left = VIEW_WIDTH / 2.0 - self.text.chars().count() as f64 * LETTER_SPACING / 2.0;
            for (index, letter) in self.text.chars().enumerate() {
                let along = (index + 1) as f64;
                let x = left + along * LETTER_SPACING + gen_range(-SHAKE, SHAKE);
                let y = LINE_Y + (self.wave + along * WAVE_PER_LETTER + gen_range(-WAVE_JITTER, WAVE_JITTER)).sin();
                draw_text(canvas, x, y, &letter.to_string(), C_RED, self.fade);
            }
        }
        self.wave += WAVE_PER_STEP * step();
        true
    }
}
