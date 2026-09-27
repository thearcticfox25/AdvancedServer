//! Ravine Mist as clients show it: the shards (obj_ravintmist_shard), the slugs
//! (obj_rmzsonic) and the found shards counter (obj_ravinemist_controller). The server
//! places, drops and counts the shards and moves the slugs (sim level/ravine_mist.rs).

use crate::client::canvas::Canvas;
use crate::core::resources::names::{sound, sprite};
use crate::core::resources::SoundId;
use crate::core::world::{World, C_WHITE};

/// obj_ravintmist_shard: the player's depth + 5; obj_rmzsonic: 0.
pub const SHARD_DEPTH: i32 = 5;
pub const SLUG_DEPTH: i32 = 0;
/// A shard on the ground floats 3 px up and down, a radian every 200 ms.
const SHARD_BOB: f64 = 3.0;
const SHARD_BOB_MILLISECONDS: f64 = 200.0;
/// obj_ravinemist_controller Draw GUI: the counter's place.
const COUNTER_PLACE: (f64, f64) = (154.0, 231.0);

#[derive(Default)]
pub struct RavineMist {
    found: u8,
}

impl RavineMist {
    /// SERVER_RMZSHARD_STATE 3: snd_shard when more shards have been found.
    pub fn step(&mut self, world: &World) -> Option<SoundId> {
        let found = world.ravine_mist.as_ref().map_or(0, |level| level.found);
        let louder = found > self.found;
        self.found = found;
        louder.then_some(sound::SND_SHARD)
    }

    pub fn draw_shards(&self, canvas: &mut Canvas, world: &World, current_time_ms: f64) {
        let Some(level) = &world.ravine_mist else { return };
        let frame = current_time_ms * canvas.sprites.get(sprite::SPR_RAVINEMIST_SHARD).fps as f64 / 1000.0;
        for shard in &level.shards {
            let bob = if shard.falling { 0.0 } else { (current_time_ms / SHARD_BOB_MILLISECONDS).sin() * SHARD_BOB };
            canvas.draw_sprite(sprite::SPR_RAVINEMIST_SHARD, frame, shard.x as f64, shard.y as f64 + bob);
        }
    }

    pub fn draw_slugs(&self, canvas: &mut Canvas, world: &World, current_time_ms: f64) {
        let Some(level) = &world.ravine_mist else { return };
        for slug in &level.slugs {
            let sprite = slug.sprite();
            let frame = current_time_ms * canvas.sprites.get(sprite).fps as f64 / 1000.0;
            canvas.draw_sprite_ext(sprite, frame, slug.x as f64, slug.y as f64, slug.xscale(), 1.0, 0.0, C_WHITE, slug.alpha());
        }
    }

    pub fn draw_gui(&self, canvas: &mut Canvas) {
        canvas.draw_sprite(sprite::SPR_RAVINEMIST_UI, self.found as f64, COUNTER_PLACE.0, COUNTER_PLACE.1);
    }
}
