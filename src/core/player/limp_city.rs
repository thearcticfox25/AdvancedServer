//! Limp City as a player meets it: the electric chains shock (obj_limpcity_echain1
//! Step) and looking down or up at an eye asks to see through another
//! (obj_limpcity_eyeA Step).

use super::hurt::Hit;
use super::Player;
use crate::core::collision::sprite_bbox;
use crate::core::config::GameplayConfig;
use crate::core::events::SimEvent;
use crate::core::level::ChainState;
use crate::core::objects::ids::ObjectId;
use crate::core::world::{ObjectVars, World};

impl Player {
    pub(super) fn meet_limp_city(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let rules = &cfg.levels.limp_city;
        let body = sprite_bbox(world.sprites.get(self.sprite_index), self.x, self.y, self.image_xscale, 1.0, 0.0);
        for chain in world.ids_of(ObjectId::LimpcityEchain1) {
            let active = matches!(world.instances[chain].vars, ObjectVars::ElectricChain { state } if state == ChainState::Shocking as u8);
            if active && world.bbox(chain).is_some_and(|bbox| bbox.overlaps(&body)) {
                let hit = Hit { xpw: -self.image_xscale * rules.chain_knockback_x, ..Hit::damage(cfg, rules.chain_damage) };
                self.hurt(world, cfg, hit, events);
            }
        }
        if self.is_dead {
            return;
        }
        for eye in world.ids_of(ObjectId::LimpcityEyea) {
            let ObjectVars::Eye { nid, targets: Some((below, above)), used, charge, .. } = world.instances[eye].vars else { continue };
            let using = used && self.watching_eye == Some(nid);
            let over = world.bbox(eye).is_some_and(|bbox| bbox.overlaps(&body));
            let looking = self.is_looking_down || self.is_looking_up;
            if over && !using && charge >= rules.eye_min_charge && looking {
                let target = if self.is_looking_down { below } else { above };
                events.push(SimEvent::EyeRequest { eye: nid, target, on: true });
            }
            if using && (!over || !looking) {
                events.push(SimEvent::EyeRequest { eye: nid, target: above, on: false });
            }
        }
    }
}
