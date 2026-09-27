//! Eggman's tracker mine: placement from the server entity (entities/eggman_tracker.rs
//! of AdvancedServer), triggering from obj_eggtrack.

use crate::core::resources::names::{sound, sprite};
use crate::core::resources::SpriteId;
use crate::core::collision::sprite_bbox;
use crate::core::config::GameplayConfig;
use crate::core::events::SimEvent;
use crate::core::player::{Character, Player};
use crate::core::world::World;

#[derive(Clone, Debug)]
pub struct EggTracker {
    pub x: f64,
    pub y: f64,
    /// Index of the Eggman player who placed it.
    pub owner: usize,
    pub sprite_index: SpriteId,
    pub image_index: f64,
    pub activated: bool,
}

impl EggTracker {
    pub fn new(x: f64, y: f64, owner: usize) -> EggTracker {
        EggTracker { x, y, owner, sprite_index: sprite::SPR_EGGTRACK, image_index: 0.0, activated: false }
    }

    /// Returns false once the destroy animation has finished.
    pub fn tick(&mut self, world: &World, players: &mut [Player], events: &mut Vec<SimEvent>) -> bool {
        let meta = world.sprites.get(self.sprite_index);
        self.image_index = (self.image_index + meta.fps as f64 / 60.0) % meta.frame_count as f64;
        if self.sprite_index == sprite::SPR_EGGTRACK_DESTROY && self.image_index >= meta.frame_count as f64 - 1.0 {
            return false;
        }
        if self.activated {
            return true;
        }
        // The trap works for the side Eggman is not on at the moment it triggers.
        let owner_demonized = players.get(self.owner).is_some_and(|owner| owner.is_demonized());
        for (index, player) in players.iter_mut().enumerate() {
            if index != self.owner && self.catches(world, player, owner_demonized) {
                self.trigger(&world.config, player, index, events);
                break;
            }
        }
        true
    }

    /// The original skipped every Eggman, which was the owner in a round with one
    /// Eggman; the caller skips the owner, so another Eggman on the other side is caught.
    fn catches(&self, world: &World, player: &Player, owner_demonized: bool) -> bool {
        if player.is_dead {
            return false;
        }
        let victim_is_enemy_side = player.is_demonized() || player.character == Character::Exe;
        if owner_demonized == victim_is_enemy_side {
            return false;
        }
        let trap = sprite_bbox(world.sprites.get(self.sprite_index), self.x, self.y, 1.0, 1.0, 0.0);
        let body = sprite_bbox(world.sprites.get(player.sprite_index), player.x, player.y, player.image_xscale, 1.0, 0.0);
        trap.overlaps(&body)
    }

    fn trigger(&mut self, cfg: &GameplayConfig, player: &mut Player, index: usize, events: &mut Vec<SimEvent>) {
        let tracker = &cfg.eggman.tracker;
        // EXE uses the tracker's own percent, everyone else scr_player_slow's.
        let percent = if player.character == Character::Exe { tracker.exe_slow_percent } else { cfg.physics.slowdown_percent };
        player.slow_down(cfg, percent, tracker.slow_seconds);
        events.push(SimEvent::Sound { sound: sound::SND_EGG_TRACKER_ACTIVATE, x: self.x, y: self.y });
        events.push(SimEvent::TrackerCaught { eggman: self.owner, victim: index });
        self.activated = true;
        self.sprite_index = sprite::SPR_EGGTRACK_DESTROY;
        self.image_index = 0.0;
    }
}
