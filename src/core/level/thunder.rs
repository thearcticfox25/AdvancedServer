//! Green Hill's lightning (the server's hill_thunder): now and then it strikes, and
//! for a moment the water shocks whoever stands in it (obj_ghz_water.electro).

use super::Random;
use crate::core::config::{ticks, GreenHillRules};
use crate::core::events::SimEvent;
use crate::core::objects::ids::ObjectId;
use crate::core::world::{ObjectVars, World};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Thunder {
    /// Ticks until the shock ends and the next cycle starts.
    timer: i32,
    struck: bool,
}

impl Thunder {
    pub(super) fn new(rules: &GreenHillRules, random: &mut Random) -> Thunder {
        Thunder { timer: cycle_ticks(rules, random), struck: false }
    }

    pub(super) fn tick(&mut self, rules: &GreenHillRules, random: &mut Random, events: &mut Vec<SimEvent>) {
        if self.timer <= ticks(rules.shock_seconds) && !self.struck {
            self.struck = true;
            events.push(SimEvent::Lightning);
        } else if self.timer <= 0 {
            self.struck = false;
            self.timer = cycle_ticks(rules, random);
            return;
        }
        self.timer -= 1;
    }

    pub(super) fn water_shocks(&self) -> bool {
        self.struck
    }
}

fn cycle_ticks(rules: &GreenHillRules, random: &mut Random) -> i32 {
    ticks(rules.thunder_cycle_seconds + random.below(rules.thunder_cycle_offset_seconds) as f64)
}

pub(super) fn apply(world: &mut World, water_shocks: bool) {
    let waters: Vec<_> = world.ids_of(ObjectId::GhzWater).collect();
    for water in waters {
        world.instances[water].vars = ObjectVars::Water { electro: water_shocks };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_shocks_for_two_seconds_once_a_cycle() {
        let rules = GreenHillRules { thunder_cycle_offset_seconds: 0, ..GreenHillRules::default() };
        let mut random = Random::new(7);
        let mut thunder = Thunder::new(&rules, &mut random);
        let mut events = Vec::new();
        let mut shock_lengths = Vec::new();
        let mut shocked_before = false;
        // hill_thunder spends one extra tick on each reset.
        for _ in 0..2 * (ticks(rules.thunder_cycle_seconds) + 1) {
            thunder.tick(&rules, &mut random, &mut events);
            match (shocked_before, thunder.water_shocks()) {
                (false, true) => shock_lengths.push(1),
                (true, true) => *shock_lengths.last_mut().unwrap() += 1,
                _ => {}
            }
            shocked_before = thunder.water_shocks();
        }
        let strikes = events.iter().filter(|event| **event == SimEvent::Lightning).count();
        assert_eq!(strikes, 2, "one strike a cycle");
        assert_eq!(shock_lengths, vec![ticks(rules.shock_seconds); 2]);
    }
}
