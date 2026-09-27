//! Fart Zone's training dummy (the server's dummy): hits push it along its floor, and
//! it slides to a stop.

use crate::core::config::{step, FartZoneRules};
use crate::core::objects::ids::ObjectId;
use crate::core::world::{ObjectVars, World};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Dummy {
    /// None until the first tick finds the map's dummy.
    x: Option<f64>,
    speed: f64,
    /// Ticks it ignores shots and stomp waves after one (the clients' timer).
    rest: i32,
}

/// What clients are told of the dummy (SERVER_FART_STATE): where it is, and whether
/// it ignores hits.
pub type DummyView = (f32, bool);

impl Dummy {
    pub(super) fn tick(&mut self, rules: &FartZoneRules, world: &World) {
        let Some(start_x) = world.ids_of(ObjectId::FartDummy).find_map(|id| match world.instances[id].vars {
            ObjectVars::Dummy { start_x, .. } => Some(start_x),
            _ => None,
        }) else {
            return;
        };
        let x = self.x.get_or_insert(start_x);
        *x = (*x + self.speed * step()).clamp(rules.dummy_left, rules.dummy_right);
        let friction = self.speed.abs().min(rules.dummy_friction * step());
        self.speed -= friction * crate::core::player::gm_sign(self.speed);
        if self.rest > 0 {
            self.rest -= 1;
        }
    }

    /// CLIENT_FART_PUSH
    pub(super) fn push(&mut self, speed: f64) {
        self.speed = speed;
    }

    /// A shot or a stomp wave hit it; it ignores the next ones for `ticks`.
    pub(super) fn rest(&mut self, ticks: i32) {
        self.rest = ticks;
    }

    pub(super) fn view(&self) -> Option<DummyView> {
        self.x.map(|x| (x as f32, self.rest > 0))
    }
}

pub(super) fn apply(world: &mut World, (x, resting): DummyView) {
    let dummies: Vec<_> = world.ids_of(ObjectId::FartDummy).collect();
    for dummy in dummies {
        if let ObjectVars::Dummy { start_x, .. } = world.instances[dummy].vars {
            world.instances[dummy].vars = ObjectVars::Dummy { start_x, resting };
            world.instances[dummy].x = x as f64;
        }
    }
}
