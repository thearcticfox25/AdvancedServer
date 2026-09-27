//! Kind and Fair's speed monitors (the server's kaf_speedbox): an attack, a boost or a
//! Tails shot breaks one, the player who broke it speeds off, and it is back a while later.

use super::Random;
use crate::core::config::{ticks, KindAndFairRules};
use crate::core::objects::ids::ObjectId;
use crate::core::player::Player;
use crate::core::world::{ObjectVars, World};
use serde::{Deserialize, Serialize};

/// kind_and_fair's init makes this many; instance_find(nid % count) pairs them with the map's.
const MONITORS: usize = 11;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Monitors {
    /// Ticks until each broken monitor is back; None while whole.
    broken: Vec<Option<i32>>,
}

impl Default for Monitors {
    fn default() -> Monitors {
        Monitors { broken: vec![None; MONITORS] }
    }
}

impl Monitors {
    pub(super) fn tick(&mut self) {
        for timer in &mut self.broken {
            match timer {
                Some(left) if *left > 0 => *left -= 1,
                Some(_) => *timer = None,
                None => {}
            }
        }
    }

    /// CLIENT_KAFMONITOR_ACTIVATE: `breaker` is the player whose attack broke it, None for a shot.
    pub(super) fn hit(&mut self, rules: &KindAndFairRules, random: &mut Random, nid: u8, breaker: Option<&mut Player>) {
        let Some(timer) = self.broken.get_mut(nid as usize).filter(|timer| timer.is_none()) else { return };
        *timer = Some(ticks(rules.monitor_broken_seconds + random.below(rules.monitor_broken_offset_seconds) as f64));
        // SERVER_KAFMONITOR_STATE 2 on the breaker's client.
        if let Some(player) = breaker.filter(|player| !player.is_boosting) {
            player.is_boosting = true;
            player.is_spinning = false;
            player.is_jumping = false;
        }
    }

    pub(super) fn broken_mask(&self) -> u16 {
        self.broken.iter().enumerate().filter(|(_, timer)| timer.is_some()).fold(0, |mask, (nid, _)| mask | 1 << nid)
    }
}

/// SERVER_KAFMONITOR_STATE 1 and 2 on clients: a broken monitor shows its second frame.
pub(super) fn apply(world: &mut World, broken_mask: u16) {
    let monitors: Vec<_> = world.ids_of(ObjectId::KafSpeedbox).collect();
    for monitor in monitors {
        if let ObjectVars::Monitor { nid, .. } = world.instances[monitor].vars {
            let broken = nid < 16 && broken_mask & (1 << nid) != 0;
            world.instances[monitor].vars = ObjectVars::Monitor { nid, broken };
            world.instances[monitor].image_index = if broken { 1.0 } else { 0.0 };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::GameplayConfig;
    use crate::core::player::Character;

    #[test]
    fn a_broken_monitor_boosts_its_breaker_once_and_comes_back() {
        let rules = KindAndFairRules { monitor_broken_offset_seconds: 0, ..KindAndFairRules::default() };
        let mut monitors = Monitors::default();
        let mut random = Random::new(5);
        let mut tails = Player::new(Character::Tails, 0.0, 0.0, &GameplayConfig::default());
        monitors.hit(&rules, &mut random, 3, Some(&mut tails));
        assert!(tails.is_boosting);
        assert_eq!(monitors.broken_mask(), 1 << 3);
        tails.is_boosting = false;
        monitors.hit(&rules, &mut random, 3, Some(&mut tails));
        assert!(!tails.is_boosting, "an already broken monitor gives nothing");
        for _ in 0..=ticks(rules.monitor_broken_seconds) {
            monitors.tick();
        }
        assert_eq!(monitors.broken_mask(), 0, "back after its time");
    }
}
