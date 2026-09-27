//! The round clock of the waiting room and the level HUD: SERVER_GAME_TIME_SYNC
//! sets it, and a hand turns a step with every sync.

use crate::client::audio::Audio;
use crate::client::canvas::Canvas;
use crate::client::text::{draw_counter, draw_counter_padded};
use crate::core::resources::names::{sound, sprite};
use crate::core::config::ORIGINAL_TICKS_PER_SECOND;
use crate::core::world::C_WHITE;

const TICKS_PER_MINUTE: f64 = 3600.0;
/// The clock hand turns 45 degrees per time sync.
const HAND_STEP_DEGREES: f64 = -45.0;
const HAND_SMOOTHING: f64 = 0.2;
/// With this many seconds left in the last minute, every time sync ticks audibly.
const WARNING_SECONDS: i32 = 10;
/// spr_counter frame of the colon between minutes and seconds.
const COUNTER_COLON_FRAME: f64 = 10.0;

#[derive(Default)]
pub struct RoundClock {
    /// global.timeMinutes, global.timeSeconds
    pub minutes: i32,
    pub seconds: i32,
    /// timeFrame: time syncs so far, the hand's target.
    frame: i32,
    angle: f64,
}

impl RoundClock {
    /// SERVER_GAME_TIME_SYNC with the ticks left in the round.
    pub fn sync(&mut self, audio: &mut Audio, ticks_left: u16) {
        let ticks_left = ticks_left as f64;
        self.minutes = ((ticks_left + 1.0) / TICKS_PER_MINUTE).ceil() as i32 - 1;
        self.seconds = (ticks_left / ORIGINAL_TICKS_PER_SECOND).ceil() as i32 % 60;
        self.frame += 1;
        if self.minutes == 0 && self.seconds <= WARNING_SECONDS {
            audio.play(sound::SND_CLOCK, false);
        }
    }

    /// The clock at the top of the screen; the hand eases towards its step every draw.
    pub fn draw(&mut self, canvas: &mut Canvas) {
        canvas.draw_sprite(sprite::SPR_CLOCK, 0.0, 211.0, 0.0);
        self.angle += (HAND_STEP_DEGREES * self.frame as f64 - self.angle) * crate::core::config::eased_share(HAND_SMOOTHING);
        canvas.draw_sprite_ext(sprite::SPR_CLOCKHAND, 0.0, 212.0, 11.5, 1.0, 1.0, self.angle, C_WHITE, 1.0);
        canvas.draw_sprite(sprite::SPR_COUNTER, COUNTER_COLON_FRAME, 233.0, 4.0);
        draw_counter(canvas, self.minutes, 224.0, 4.0);
        draw_counter_padded(canvas, self.seconds, 244.0, 4.0, 2);
    }
}
