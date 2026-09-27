//! DotDotDot as clients show it (obj_dotdotdot_i): the sky changes with the part of the
//! level the view is in, fading out and back in, the screen tints with it, and the music
//! turns to its second track in the last part.

use crate::client::audio::Audio;
use crate::client::canvas::Canvas;
use crate::client::room::{LayerContent, Room};
use crate::core::resources::names::sprite;
use crate::core::resources::SpriteId;
use crate::core::objects::ids::ObjectId;
use crate::core::world::{World, C_WHITE};
use crate::core::config::step;

const SKY_FADE: f64 = 0.05;
const TINT_FADE: f64 = 0.025;
const MUSIC_FADE_SECONDS: f64 = 2.0;

pub struct DotDotDot {
    sky: SpriteId,
    fade: f64,
    fading_out: bool,
    changing: bool,
    tints: [f64; 2],
}

impl Default for DotDotDot {
    fn default() -> DotDotDot {
        DotDotDot { sky: sprite::BACKGROUND_DOTDOTDOT, fade: 0.0, fading_out: true, changing: false, tints: [0.0; 2] }
    }
}

impl DotDotDot {
    pub fn step(&mut self, room: &mut Room, world: &World, audio: &mut Audio, view: (f64, f64)) {
        let Some(LayerContent::Background(sky)) = room.layer_mut("Background").map(|layer| &mut layer.content) else { return };
        if self.fading_out {
            if self.fade > 0.0 {
                self.fade -= SKY_FADE * step();
            } else {
                sky.sprite = Some(self.sky);
                self.fading_out = false;
            }
        } else if self.fade < 1.0 {
            self.fade += SKY_FADE * step();
        } else {
            self.changing = false;
        }
        sky.alpha = self.fade;

        // The part of the level at the middle of the view (the original measured from 427 x 240).
        let (x, y) = (view.0 + VIEW_WIDTH_ORIGINAL / 2.0, view.1 + VIEW_HEIGHT_ORIGINAL / 2.0);
        let shown = sky.sprite;
        let parts = [
            (ObjectId::DotdotdotTrigger3, sprite::BACKGROUND_DOTDOTDOT3, Some(true)),
            (ObjectId::DotdotdotTrigger2, sprite::BACKGROUND_DOTDOTDOT2, None),
            (ObjectId::DotdotdotTrigger, sprite::BACKGROUND_DOTDOTDOT, Some(false)),
        ];
        if let Some(&(_, wanted, music)) = parts.iter().find(|(trigger, _, _)| world.position_meeting(x, y, *trigger)) {
            if !self.changing && shown != Some(wanted) {
                if let Some(second_track) = music {
                    audio.crossfade(second_track, MUSIC_FADE_SECONDS);
                }
                self.changing = true;
                self.fading_out = true;
                self.sky = wanted;
            }
        }
    }

    /// Draw GUI: the tints of the second and third parts.
    pub fn draw_gui(&mut self, canvas: &mut Canvas) {
        let wanted = match self.sky {
            sprite::BACKGROUND_DOTDOTDOT2 => [1.0, 0.0],
            sprite::BACKGROUND_DOTDOTDOT3 => [0.0, 1.0],
            _ => [0.0, 0.0],
        };
        for (frame, (tint, wanted)) in self.tints.iter_mut().zip(wanted).enumerate() {
            if *tint < wanted {
                *tint += TINT_FADE * step();
            } else if *tint > wanted {
                *tint -= TINT_FADE * step();
            }
            canvas.draw_sprite_ext(sprite::SPR_SCREENOVERLAY2, frame as f64, 0.0, 0.0, 1.0, 1.0, 0.0, C_WHITE, *tint);
        }
    }
}

/// obj_dotdotdot_i Step places itself at the middle of a 427 x 240 view.
const VIEW_WIDTH_ORIGINAL: f64 = 427.0;
const VIEW_HEIGHT_ORIGINAL: f64 = 240.0;
