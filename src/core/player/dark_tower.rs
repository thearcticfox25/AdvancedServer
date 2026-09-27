//! What Dark Tower's balls (obj_darktower_ball Step) and falling stalactites
//! (obj_darktower_stalactite End Step) do to a player.

use super::hurt::Hit;
use super::Player;
use crate::core::collision::sprite_bbox;
use crate::core::config::GameplayConfig;
use crate::core::events::SimEvent;
use crate::core::objects::ids::ObjectId;
use crate::core::world::{ObjectVars, World};

impl Player {
    pub(super) fn touch_dark_tower_balls(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let rules = &cfg.levels.dark_tower;
        for ball in world.ids_of(ObjectId::DarktowerBall) {
            if self.touches(world, ball) {
                let hit = Hit { xpw: -self.image_xscale * rules.ball_knockback_x, ..Hit::damage(cfg, rules.ball_damage) };
                self.hurt(world, cfg, hit, events);
            }
        }
    }

    pub(super) fn touch_falling_stalactites(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let rules = &cfg.levels.dark_tower;
        for stalactite in world.ids_of(ObjectId::DarktowerStalactite) {
            let falling = matches!(world.instances[stalactite].vars, ObjectVars::Stalactite { falling: true, .. });
            if falling && world.instances[stalactite].visible && self.touches(world, stalactite) {
                let hit = Hit { xpw: -self.image_xscale * rules.stalactite_knockback_x, ..Hit::damage(cfg, rules.stalactite_damage) };
                self.hurt(world, cfg, hit, events);
            }
        }
    }

    /// place_meeting(x, y, instance) by bounding boxes.
    fn touches(&self, world: &World, id: crate::core::world::InstanceId) -> bool {
        let body = sprite_bbox(world.sprites.get(self.sprite_index), self.x, self.y, self.image_xscale, 1.0, 0.0);
        world.bbox(id).is_some_and(|bbox| bbox.overlaps(&body))
    }
}
