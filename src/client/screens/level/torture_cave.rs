//! Torture Cave as clients show it: dripping water (obj_am_controller), the acid's hiss
//! where a group rises, and the face over the view in the last minute
//! (obj_abandon_mine). The server raises the acid (sim level/torture_cave.rs).

use crate::client::audio::Audio;
use crate::client::canvas::Canvas;
use crate::core::resources::names::{sound, sprite};
use crate::core::resources::SoundId;
use crate::core::objects::ids::ObjectId;
use crate::core::world::{ObjectVars, World, C_WHITE};
use crate::core::config::step;

/// obj_abandon_mine: in the last minute it shows, fading in over a minute.
const FACE_FADE_IN: f64 = 0.016 / 60.0;
/// The last minute of the round, in ticks of the rate the round runs at.
fn last_minute_ticks() -> i64 {
    crate::core::config::ticks(60.0) as i64
}
/// obj_soundemitter: the hiss sounds 16 px above it.
const EMITTER_ABOVE: f64 = 16.0;

pub struct TortureCave {
    risen: Vec<bool>,
    face: f64,
}

impl TortureCave {
    pub fn open(audio: &mut Audio) -> TortureCave {
        audio.play(sound::SND_WATERDROPS, true);
        TortureCave { risen: Vec::new(), face: 0.0 }
    }

    /// Returns the sounds heard from places.
    pub fn step(&mut self, world: &World) -> Vec<(SoundId, f64, f64)> {
        let mut heard = Vec::new();
        let clouds: Vec<_> = world.ids_of(ObjectId::AbadonCloud).collect();
        // SERVER_TCGOM_STATE: a cloud that shows again means its group rose.
        let risen: Vec<bool> = clouds.iter().map(|&cloud| world.instances[cloud].visible).collect();
        for (index, &cloud) in clouds.iter().enumerate() {
            let rose = risen[index] && !self.risen.get(index).copied().unwrap_or(false);
            let ObjectVars::AcidCloud { nid, .. } = world.instances[cloud].vars else { continue };
            let first_of_group = clouds[..index].iter().all(|&other| !matches!(world.instances[other].vars, ObjectVars::AcidCloud { nid: other_nid, .. } if other_nid == nid));
            if rose && first_of_group {
                // The acid's emitters are the ones whose stype is its group.
                for emitter in world.ids_of(ObjectId::Soundemitter).filter(|&id| world.instances[id].vars == (ObjectVars::SoundEmitter { kind: nid as i64 })) {
                    heard.push((sound::SND_ACID, world.instances[emitter].x, world.instances[emitter].y - EMITTER_ABOVE));
                }
            }
        }
        self.risen = risen;
        if world.timer_ticks >= last_minute_ticks() || !world.round_started {
            self.face = 0.0;
        } else if self.face < 1.0 {
            self.face += FACE_FADE_IN * step();
        }
        heard
    }

    /// obj_abandon_mine Draw GUI
    pub fn draw_gui(&self, canvas: &mut Canvas) {
        canvas.draw_sprite_ext(sprite::SPR_AM_OOH, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, C_WHITE, self.face);
    }
}
