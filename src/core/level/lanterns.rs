//! Weed Zone's lanterns (the server's wd_latern): after a dark while one random lantern
//! is lit for a while, and the survivors near it are safe from the weed.

use super::Random;
use crate::core::config::{ticks, WeedZoneRules};
use crate::core::objects::ids::ObjectId;
use crate::core::world::{ObjectVars, World};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lanterns {
    timer: i32,
    /// How long the current dark or lit while lasts.
    wait: i32,
    lit: Option<u8>,
}

impl Lanterns {
    pub(super) fn new(rules: &WeedZoneRules, random: &mut Random) -> Lanterns {
        Lanterns { timer: 0, wait: ticks(rules.lantern_dark_seconds + random.below(rules.lantern_dark_extra_seconds) as f64), lit: None }
    }

    pub(super) fn tick(&mut self, rules: &WeedZoneRules, world: &World, random: &mut Random) {
        self.timer += 1;
        if self.timer < self.wait {
            return;
        }
        self.timer = 0;
        self.lit = match self.lit {
            None => {
                self.wait = ticks(rules.lantern_lit_seconds + random.below(rules.lantern_lit_extra_seconds) as f64);
                Some(random.below(world.ids_of(ObjectId::WeedLantern).count() as u32) as u8)
            }
            Some(_) => {
                self.wait = ticks(rules.lantern_dark_seconds + random.below(rules.lantern_dark_extra_seconds) as f64);
                None
            }
        };
    }

    pub(super) fn lit(&self) -> Option<u8> {
        self.lit
    }
}

/// SERVER_WDLATERN_ACTIVATE: the lantern with that nid is lit and every other is not.
pub(super) fn apply(world: &mut World, lit: Option<u8>) {
    let lanterns: Vec<_> = world.ids_of(ObjectId::WeedLantern).collect();
    for lantern in lanterns {
        if let ObjectVars::Lantern { nid, .. } = world.instances[lantern].vars {
            world.instances[lantern].vars = ObjectVars::Lantern { nid, lit: Some(nid) == lit };
        }
    }
}
