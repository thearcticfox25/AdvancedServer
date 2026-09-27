//! Nasty Paradise's ice blocks (the server's nap_ice): landing on one, an attack, a
//! gliding Knuckles or a Tails shot breaks it, and it grows back a while later.

use crate::core::resources::names::sprite;
use crate::core::config::{ticks, NastyParadiseRules};
use crate::core::objects::ids::ObjectId;
use crate::core::world::{ObjectVars, World};
use serde::{Deserialize, Serialize};

/// nap_init makes this many; CLIENT_NAPICE_ACTIVATE with a higher nid is ignored.
const ICE_BLOCKS: usize = 10;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IceBlocks {
    /// Ticks until each broken block is back; None while whole.
    broken: Vec<Option<i32>>,
}

impl Default for IceBlocks {
    fn default() -> IceBlocks {
        IceBlocks { broken: vec![None; ICE_BLOCKS] }
    }
}

impl IceBlocks {
    pub(super) fn tick(&mut self) {
        for timer in &mut self.broken {
            if let Some(left) = timer {
                *left -= 1;
                if *left <= 0 {
                    *timer = None;
                }
            }
        }
    }

    /// CLIENT_NAPICE_ACTIVATE
    pub(super) fn hit(&mut self, rules: &NastyParadiseRules, nid: u8) {
        if let Some(timer) = self.broken.get_mut(nid as usize).filter(|timer| timer.is_none()) {
            *timer = Some(ticks(rules.ice_regeneration_seconds));
        }
    }

    pub(super) fn broken_mask(&self) -> u16 {
        self.broken.iter().enumerate().filter(|(_, timer)| timer.is_some()).fold(0, |mask, (nid, _)| mask | 1 << nid)
    }
}

/// SERVER_NAPICE_STATE on clients: a block that broke hides with an empty mask, one
/// that is back grows (spr_nap_iceblock2) and then stands (obj_nap_iceblock Step).
pub(super) fn apply(world: &mut World, broken_mask: u16) {
    let blocks: Vec<_> = world.ids_of(ObjectId::NapIceblock).collect();
    for block in blocks {
        let ObjectVars::IceBlock { nid } = world.instances[block].vars else { continue };
        let broken = nid < 16 && broken_mask & (1 << nid) != 0;
        let growing_frames = world.sprites.get(sprite::SPR_NAP_ICEBLOCK2).frame_count as f64;
        let instance = &mut world.instances[block];
        if broken && instance.visible {
            instance.visible = false;
            instance.sprite_index = Some(sprite::SPR_NAP_ICEBLOCK3);
        } else if !broken && !instance.visible {
            instance.visible = true;
            instance.sprite_index = Some(sprite::SPR_NAP_ICEBLOCK2);
            instance.image_index = 0.0;
        }
        if instance.visible && instance.sprite_index == Some(sprite::SPR_NAP_ICEBLOCK2) && instance.image_index >= growing_frames - 1.0 {
            instance.sprite_index = Some(sprite::SPR_NAP_ICEBLOCK);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_broken_block_is_back_after_its_time() {
        let rules = NastyParadiseRules::default();
        let mut blocks = IceBlocks::default();
        blocks.hit(&rules, 4);
        blocks.hit(&rules, 200);
        assert_eq!(blocks.broken_mask(), 1 << 4);
        for _ in 1..ticks(rules.ice_regeneration_seconds) {
            blocks.tick();
        }
        assert_eq!(blocks.broken_mask(), 1 << 4, "still broken a tick before its time");
        blocks.tick();
        assert_eq!(blocks.broken_mask(), 0);
    }
}
