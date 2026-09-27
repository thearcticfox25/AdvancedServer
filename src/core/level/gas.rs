//! You Can't Run's gas (the server's you_cant_run): every few seconds it either fills
//! one of the smoke areas, picked at random, or clears.

use super::Random;
use crate::core::config::{ticks, YouCantRunRules};
use crate::core::objects::ids::ObjectId;
use crate::core::world::{ObjectVars, World};
use serde::{Deserialize, Serialize};

/// SERVER_YCRSMOKE_READY comes a second into each wait (it changes nothing on clients).
const READY_SECONDS: f64 = 1.0;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Gas {
    timer: i32,
    ready: bool,
    active: bool,
    area: u8,
}

impl Gas {
    pub(super) fn tick(&mut self, rules: &YouCantRunRules, random: &mut Random) {
        if !self.ready {
            self.ready = self.timer >= ticks(READY_SECONDS);
        } else if self.timer >= ticks(rules.gas_seconds) {
            self.ready = false;
            self.timer = 0;
            self.active = !self.active;
            self.area = if self.active { random.below(rules.gas_areas) as u8 } else { 0 };
        }
        self.timer += 1;
    }

    /// The nid of the smoke areas full of gas.
    pub(super) fn gassed_area(&self) -> Option<u8> {
        self.active.then_some(self.area)
    }
}

pub(super) fn apply(world: &mut World, gassed_area: Option<u8>) {
    let areas: Vec<_> = world.ids_of(ObjectId::YcrSmokearea).collect();
    for area in areas {
        if let ObjectVars::SmokeArea { nid, .. } = world.instances[area].vars {
            world.instances[area].vars = ObjectVars::SmokeArea { nid, gassed: Some(nid) == gassed_area };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gas_fills_an_area_then_clears_every_six_seconds() {
        let rules = YouCantRunRules::default();
        let mut gas = Gas::default();
        let mut random = Random::new(3);
        let mut changes = Vec::new();
        let mut last = None;
        for tick in 0..ticks(rules.gas_seconds) * 4 + 8 {
            gas.tick(&rules, &mut random);
            if gas.gassed_area().is_some() != last.is_some() {
                changes.push(tick);
            }
            last = gas.gassed_area();
        }
        assert_eq!(changes.len(), 4, "on, off, on, off: {changes:?}");
        assert!(changes.windows(2).all(|pair| pair[1] - pair[0] == ticks(rules.gas_seconds)), "{changes:?}");
    }
}
