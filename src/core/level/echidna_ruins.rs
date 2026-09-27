//! Echidna Ruins' crystals (the server's mj_ass) and judgers (mj_judger). The lava
//! platform is lava.rs.

use super::Random;
use crate::core::config::{step, ticks, EchidnaRuinsRules};
use crate::core::objects::ids::ObjectId;
use crate::core::world::{ObjectVars, World};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EchidnaRuins {
    crystal_timer: i32,
    crystal_wait: i32,
    lit: bool,
    glow: f64,
    judger_timer: i32,
    judger_wait: i32,
    judger: Judger,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
enum Judger {
    Waiting = 0,
    Ready = 1,
    Firing = 2,
}

/// What clients are told: whether the crystals are lit and how much they glow, and the
/// judgers' state.
pub type EchidnaRuinsView = (bool, f32, u8);

impl EchidnaRuins {
    pub(super) fn new(rules: &EchidnaRuinsRules, random: &mut Random) -> EchidnaRuins {
        EchidnaRuins {
            crystal_timer: 0,
            crystal_wait: ticks(rules.crystal_seconds),
            lit: false,
            glow: 0.0,
            judger_timer: 0,
            judger_wait: ticks(rules.judger_wait_seconds + random.below(rules.judger_wait_extra_seconds) as f64),
            judger: Judger::Waiting,
        }
    }

    pub(super) fn tick(&mut self, rules: &EchidnaRuinsRules, random: &mut Random) {
        // mj_ass
        if self.crystal_timer >= self.crystal_wait {
            self.lit = !self.lit;
            self.crystal_timer = 0;
            self.crystal_wait = ticks(rules.crystal_seconds + random.below(rules.crystal_extra_seconds) as f64);
        }
        self.crystal_timer += 1;
        // obj_marijuna_crystal Step
        let glow_step = rules.crystal_glow_per_tick * step();
        self.glow = if self.lit { (self.glow + glow_step).min(1.0) } else { (self.glow - glow_step).max(0.0) };
        // mj_judger
        if self.judger_timer >= self.judger_wait {
            (self.judger, self.judger_wait) = match self.judger {
                Judger::Waiting => (Judger::Ready, ticks(rules.judger_ready_seconds)),
                Judger::Ready => (Judger::Firing, ticks(rules.judger_fire_seconds + random.below(rules.judger_fire_extra_seconds) as f64)),
                Judger::Firing => (Judger::Waiting, ticks(rules.judger_wait_seconds + random.below(rules.judger_wait_extra_seconds) as f64)),
            };
            self.judger_timer = 0;
        }
        self.judger_timer += 1;
    }

    pub(super) fn view(&self) -> EchidnaRuinsView {
        (self.lit, self.glow as f32, self.judger as u8)
    }
}

pub(super) fn apply(world: &mut World, (lit, glow, state): EchidnaRuinsView) {
    let ids: Vec<_> = world.ids_of(ObjectId::MarijunaCrystal).chain(world.ids_of(ObjectId::MarijunaJudger)).collect();
    for id in ids {
        let instance = &mut world.instances[id];
        match instance.vars {
            ObjectVars::MjCrystal { .. } => instance.vars = ObjectVars::MjCrystal { lit, glow: glow as f64 },
            // SERVER_MJJUDGER_STATE is read as a bool: firing looks like getting ready, and
            // the judgers never fire (obj_marijuna_judgerpor is never made).
            ObjectVars::Judger { .. } => {
                instance.vars = ObjectVars::Judger { state };
                instance.image_index = if state > 0 { 1.0 } else { 0.0 };
            }
            _ => {}
        }
    }
}
