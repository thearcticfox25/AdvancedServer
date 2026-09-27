//! What Volcano Valley does to a player: its lava columns burn whoever they touch
//! (obj_vv_lavacolumn Step), and its vases break under an attack or a fall (obj_vv_vase Step).

use super::amy::AMY_HJUMP;
use super::hurt::Hit;
use super::sally::SALLY_SLIDE;
use super::{gm_sign, Character, Player};
use crate::core::resources::names::sound;
use crate::core::collision::sprite_bbox;
use crate::core::config::GameplayConfig;
use crate::core::events::SimEvent;
use crate::core::objects::ids::ObjectId;
use crate::core::world::{ObjectVars, World};

impl Player {
    /// obj_vv_lavacolumn Step, a column at a time.
    pub(super) fn touch_lava(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let rules = &cfg.levels.volcano_valley.lava;
        for column in world.ids_of(ObjectId::VvLavacolumn) {
            let body = sprite_bbox(world.sprites.get(self.sprite_index), self.x, self.y, self.image_xscale, 1.0, 0.0);
            if world.bbox(column).is_some_and(|bbox| bbox.overlaps(&body)) {
                let xpw = gm_sign(self.x - world.instances[column].x) * rules.knockback_x;
                let hit = Hit { xpw, ypw: rules.knockback_y, sound: sound::SND_LAVAHIT, ..Hit::damage(cfg, rules.damage) };
                self.hurt(world, cfg, hit, events);
            }
        }
    }

    /// obj_vv_vase Step: an attack, a spin, a slide or falling onto a vase breaks it;
    /// EXE and demons pass through.
    pub(super) fn break_vases(&self, world: &World, events: &mut Vec<SimEvent>) {
        let character = self.character;
        let mut breaks = (self.is_attacking && matches!(character, Character::Amy | Character::Knux | Character::Eggman | Character::Sally))
            || (character == Character::Amy && self.state == AMY_HJUMP)
            || self.is_spinning
            || (!self.is_grounded && self.yspd > 0.0)
            || (character == Character::Sally && self.state == SALLY_SLIDE);
        if self.revival_times >= 2 || character == Character::Exe {
            breaks = false;
        }
        if !breaks {
            return;
        }
        let body = sprite_bbox(world.sprites.get(self.sprite_index), self.x, self.y, self.image_xscale, 1.0, 0.0);
        for vase in world.ids_of(ObjectId::VvVase).filter(|&vase| world.instances[vase].visible) {
            if let (ObjectVars::Vase { nid }, Some(bbox)) = (&world.instances[vase].vars, world.bbox(vase)) {
                if bbox.overlaps(&body) {
                    events.push(SimEvent::CameraShake);
                    events.push(SimEvent::VaseHit { nid: *nid });
                }
            }
        }
    }
}
