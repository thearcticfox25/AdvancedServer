//! Black rings (obj_blackring): the ones Priceless Freedom and Fart Zone start with and
//! the ones Exetior places. Once faded in, the first survivor to touch one takes it,
//! paying rings or health.

use crate::core::collision::{sprite_bbox, Bbox};
use crate::core::config::{ticks, GameplayConfig};
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Character, Player};
use crate::core::world::World;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlackRing {
    /// Tells rings apart for clients (their hum follows one ring).
    pub id: u16,
    pub x: f64,
    pub y: f64,
    pub alpha: f64,
    /// The Exetior who placed it; None for the map's own.
    pub owner: Option<usize>,
}

impl World {
    /// SERVER_BRING_STATE 0: the round's first black rings stand on the map's first
    /// `count` spawners.
    pub fn place_map_black_rings(&mut self, count: usize) {
        let spawners: Vec<(f64, f64)> = self.ids_of(ObjectId::BlackringSpawner).take(count).map(|id| (self.instances[id].x, self.instances[id].y)).collect();
        for (x, y) in spawners {
            self.place_black_ring(x, y, None);
        }
    }

    pub fn place_black_ring(&mut self, x: f64, y: f64, owner: Option<usize>) {
        let id = self.black_rings.last().map_or(0, |ring| ring.id.wrapping_add(1));
        self.black_rings.push(BlackRing { id, x, y, alpha: 0.0, owner });
    }

    /// obj_blackring Step: image_alpha grows by a step of its fade-in.
    pub fn fade_in_black_rings(&mut self) {
        let step = 1.0 / ticks(self.config.hazards.black_ring_fade_in_seconds).max(1) as f64;
        for ring in &mut self.black_rings {
            if ring.alpha < 1.0 {
                ring.alpha += step;
            }
        }
    }

    /// The black ring a survivor takes now, if any (CLIENT_BRING_COLLECTED).
    pub fn black_ring_taken_by(&self, player: &Player) -> Option<usize> {
        if player.character == Character::Exe || player.is_demonized() || player.hp <= 0 || player.removed {
            return None;
        }
        let body = sprite_bbox(self.sprites.get(player.sprite_index), player.x, player.y, player.image_xscale, 1.0, 0.0);
        self.black_rings.iter().position(|ring| ring.alpha >= 1.0 && self.black_ring_bbox(ring).overlaps(&body))
    }

    pub fn black_ring_bbox(&self, ring: &BlackRing) -> Bbox {
        sprite_bbox(self.sprites.get(crate::core::resources::names::sprite::SPR_BLACKRING), ring.x, ring.y, 1.0, 1.0, 0.0)
    }
}

impl Player {
    /// SERVER_BRING_COLLECTED: rings are paid while there are enough, health otherwise.
    /// `index`: the player's place in the game, which the blood screen goes to.
    pub fn take_black_ring(&mut self, index: usize, world: &World, cfg: &GameplayConfig, events: &mut Vec<crate::core::events::SimEvent>) {
        use crate::core::resources::names::sound;
        use crate::core::events::SimEvent;
        let hazards = &cfg.hazards;
        events.push(SimEvent::BlackRingTaken { player: index });
        if self.rings >= hazards.black_ring_rings {
            self.rings -= hazards.black_ring_rings;
            events.push(SimEvent::Sound { sound: sound::SND_RINGABSORB, x: self.x, y: self.y });
            return;
        }
        self.hp -= hazards.black_ring_damage;
        if self.hp <= 0 {
            self.hp = 0;
            self.instakill(world, cfg, events);
        }
        events.push(SimEvent::BloodScreen { victim: index });
        events.push(SimEvent::Sound { sound: sound::SND_BLACKRING, x: self.x, y: self.y });
    }
}
