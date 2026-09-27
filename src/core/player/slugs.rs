//! Ravine Mist's slugs as a player meets them (obj_rmzsonic Step): attacks, spins,
//! slides and jumps squash them (a jump throws the survivor up); standing in one costs
//! rings or health.

use super::amy::AMY_HJUMP;
use super::animation::FALL;
use super::hurt::Hit;
use super::sally::SALLY_SLIDE;
use super::{Character, Player};
use crate::core::resources::names::{sound, sprite};
use crate::core::collision::sprite_bbox;
use crate::core::config::GameplayConfig;
use crate::core::events::SimEvent;
use crate::core::world::World;

impl Player {
    pub(super) fn touch_slugs(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let Some(level) = &world.ravine_mist else { return };
        let rules = &cfg.levels.ravine_mist;
        self.slug_bites.retain(|(id, _)| level.slugs.iter().any(|slug| slug.id == *id));
        for slug in level.slugs.iter().filter(|slug| slug.awake()) {
            let body = sprite_bbox(world.sprites.get(self.sprite_index), self.x, self.y, self.image_xscale, 1.0, 0.0);
            let bite_timer = match self.slug_bites.iter().position(|(id, _)| *id == slug.id) {
                Some(index) => index,
                None => {
                    self.slug_bites.push((slug.id, crate::core::config::original_ticks(rules.slug_bite_every_ticks)));
                    self.slug_bites.len() - 1
                }
            };
            if self.hp <= 0 || !slug.bbox(world).overlaps(&body) {
                self.slug_bites[bite_timer].1 = crate::core::config::original_ticks(rules.slug_bite_every_ticks);
                continue;
            }
            let character = self.character;
            let mut squashes = false;
            if self.is_attacking && matches!(character, Character::Exe | Character::Amy | Character::Knux | Character::Eggman | Character::Sally) {
                events.push(SimEvent::CameraShake);
                squashes = true;
            }
            if character == Character::Amy && self.state == AMY_HJUMP {
                squashes = true;
            }
            if self.revival_times < 2 {
                if (self.is_jumping && character != Character::Exe) || (self.state == FALL && character == Character::Amy) {
                    let bounce = Hit { damage: 0, xpw: 0.0, ypw: rules.slug_bounce_speed, sound: sound::SND_NONE, blood: sprite::SPR_BLOOD2, ignore: false };
                    self.hurt(world, cfg, bounce, events);
                    squashes = true;
                }
                if self.is_spinning && matches!(character, Character::Cream | Character::Knux | Character::Tails) {
                    squashes = true;
                }
                if self.state == SALLY_SLIDE && character == Character::Sally {
                    squashes = true;
                }
            }
            if squashes {
                events.push(SimEvent::SlugHit { id: slug.id });
                continue;
            }
            if character == Character::Exe || self.revival_times >= 2 {
                self.slug_bites[bite_timer].1 = crate::core::config::original_ticks(rules.slug_bite_every_ticks);
                continue;
            }
            let timer = &mut self.slug_bites[bite_timer].1;
            let bites = *timer >= crate::core::config::original_ticks(rules.slug_bite_every_ticks);
            *timer += 1;
            if !bites {
                continue;
            }
            *timer = 0;
            if self.rings > 0 {
                events.push(SimEvent::Sound { sound: sound::SND_RINGABSORB, x: self.x, y: self.y });
                self.rings -= 1;
            } else {
                let hit = Hit { xpw: -self.image_xscale * rules.slug_knockback_x, ..Hit::damage(cfg, rules.slug_damage) };
                self.hurt(world, cfg, hit, events);
            }
        }
    }
}
