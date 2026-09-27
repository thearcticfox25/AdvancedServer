//! Torture Cave's acid (the server's tc_acid): every few seconds the acid either stops
//! or rises as clouds in one random group of places.

use super::Random;
use crate::core::config::{ticks, TortureCaveRules, ticks_per_second};
use crate::core::objects::ids::ObjectId;
use crate::core::world::{ObjectVars, World};
use serde::{Deserialize, Serialize};

/// tc_acid picks one of this many groups.
const GROUPS: usize = 7;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TortureCave {
    timer: i32,
    burning: bool,
    group: u8,
    /// Ticks since each group's clouds last rose.
    since: Vec<Option<u32>>,
}

/// What clients are told: the group that burns now, and how long ago each group rose.
pub type TortureCaveView = (Option<u8>, Vec<Option<u32>>);

impl TortureCave {
    pub(super) fn new() -> TortureCave {
        TortureCave { timer: 0, burning: false, group: 0, since: vec![None; GROUPS] }
    }

    pub(super) fn tick(&mut self, rules: &TortureCaveRules, random: &mut Random) {
        for since in self.since.iter_mut().flatten() {
            *since += 1;
        }
        if self.timer >= ticks(rules.acid_seconds) {
            self.timer = 0;
            self.burning = !self.burning;
            if self.burning {
                self.group = random.below(GROUPS as u32) as u8;
                self.since[self.group as usize] = Some(0);
            }
        }
        self.timer += 1;
    }

    pub(super) fn view(&self) -> TortureCaveView {
        (self.burning.then_some(self.group), self.since.clone())
    }
}

/// SERVER_TCGOM_STATE and obj_abadon_cloud Step: a group's clouds play once from when
/// they rose and then hide; they burn while their group does.
pub(super) fn apply(world: &mut World, (burning, since): &TortureCaveView) {
    let clouds: Vec<_> = world.ids_of(ObjectId::AbadonCloud).collect();
    for cloud in clouds {
        let ObjectVars::AcidCloud { nid, .. } = world.instances[cloud].vars else { continue };
        let meta = world.instances[cloud].sprite_index.map(|sprite| world.sprites.get(sprite));
        let (fps, last_frame) = meta.map_or((0.0, 0.0), |meta| (meta.fps as f64, meta.frame_count as f64 - 1.0));
        let instance = &mut world.instances[cloud];
        instance.vars = ObjectVars::AcidCloud { nid, burning: *burning == Some(nid) };
        match since.get(nid as usize).copied().flatten() {
            Some(ticks) => {
                let frame = ticks as f64 * fps / ticks_per_second();
                instance.image_index = frame.min(last_frame);
                instance.visible = frame < last_frame;
            }
            None => instance.visible = false,
        }
    }
}
