//! Volcano Valley's vases (the server's vv_vase): each holds a few rings, which go to
//! whoever breaks it; broken vases stay gone.

use super::Random;
use crate::core::config::GameplayConfig;
use crate::core::events::SimEvent;
use crate::core::objects::ids::ObjectId;
use crate::core::player::Player;
use crate::core::world::{ObjectVars, World};
use serde::{Deserialize, Serialize};

/// vv_init makes this many.
const VASES: usize = 14;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vases {
    /// The rings in each vase, by nid; None once broken.
    rings: Vec<Option<i32>>,
}

impl Vases {
    pub(super) fn new(cfg: &GameplayConfig, random: &mut Random) -> Vases {
        let max_rings = cfg.levels.volcano_valley.vase_max_rings;
        Vases { rings: (0..VASES).map(|_| Some(random.below(max_rings) as i32 + 1)).collect() }
    }

    /// CLIENT_VVVASE_BREAK
    pub(super) fn hit(&mut self, world: &World, cfg: &GameplayConfig, nid: u8, breaker: &mut Player, events: &mut Vec<SimEvent>) {
        if let Some(rings) = self.rings.get_mut(nid as usize).and_then(Option::take) {
            breaker.take_vase_rings(world, cfg, rings, events);
        }
    }

    pub(super) fn broken_mask(&self) -> u16 {
        self.rings.iter().enumerate().filter(|(_, rings)| rings.is_none()).fold(0, |mask, (nid, _)| mask | 1 << nid)
    }
}

/// SERVER_VVVASE_STATE on clients: a broken vase is hidden.
pub(super) fn apply(world: &mut World, broken_mask: u16) {
    let vases: Vec<_> = world.ids_of(ObjectId::VvVase).collect();
    for vase in vases {
        if let ObjectVars::Vase { nid } = world.instances[vase].vars {
            world.instances[vase].visible = !(nid < 16 && broken_mask & (1 << nid) != 0);
        }
    }
}
