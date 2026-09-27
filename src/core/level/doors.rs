//! Haunting Dream's doors (the server's hd_door): using a crystal opens every door, or
//! closes them, and the crystals rest for a while. A door slides a pixel a tick.

use crate::core::config::{ticks, HauntingDreamRules};
use crate::core::objects::ids::ObjectId;
use crate::core::world::{InstanceId, ObjectVars, World};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Doors {
    open: bool,
    /// Ticks until the crystals can be used again.
    rest: i32,
    /// How far each door (obj_hd_door, then obj_hd_door2, in instance order) slid open.
    slid: Vec<u16>,
}

impl Doors {
    pub(super) fn tick(&mut self, world: &World) {
        if self.rest > 0 {
            self.rest -= 1;
        }
        let doors = door_ids(world);
        self.slid.resize(doors.len(), 0);
        for (slid, &door) in self.slid.iter_mut().zip(&doors) {
            let full = door_size(world, door);
            if self.open && *slid < full {
                *slid += 1;
            } else if !self.open && *slid > 0 {
                *slid -= 1;
            }
        }
    }

    /// CLIENT_HDDOOR_TOGGLE: nothing while the crystals rest.
    pub(super) fn toggle(&mut self, rules: &HauntingDreamRules) {
        if self.rest > 0 {
            return;
        }
        self.open = !self.open;
        self.rest = ticks(rules.door_toggle_rest_seconds);
    }

    pub(super) fn view(&self) -> (bool, bool, Vec<u16>) {
        (self.open, self.rest <= 0, self.slid.clone())
    }
}

fn door_ids(world: &World) -> Vec<InstanceId> {
    world.ids_of(ObjectId::HdDoor).chain(world.ids_of(ObjectId::HdDoor2)).collect()
}

/// A door opens by its own height (obj_hd_door) or width (obj_hd_door2).
fn door_size(world: &World, door: InstanceId) -> u16 {
    let instance = &world.instances[door];
    let meta = world.sprites.get(instance.sprite_index.unwrap_or(crate::core::resources::names::sprite::SPR_HD_DOOR));
    let size = if instance.object == ObjectId::HdDoor { meta.height as f64 * instance.image_yscale } else { meta.width as f64 * instance.image_xscale };
    size.abs() as u16
}

/// SERVER_HDDOOR_STATE on clients: the doors' state and place, the crystals' light.
pub(super) fn apply(world: &mut World, open: bool, crystals_usable: bool, slid: &[u16]) {
    for (&door, &slid) in door_ids(world).iter().zip(slid) {
        let instance = &mut world.instances[door];
        let ObjectVars::Door { closed_x, closed_y, .. } = instance.vars else { continue };
        let was_slid = if instance.object == ObjectId::HdDoor { closed_y - instance.y } else { closed_x - instance.x };
        let closing = !open && (slid as f64) < was_slid;
        instance.vars = ObjectVars::Door { closed_x, closed_y, open, closing };
        if instance.object == ObjectId::HdDoor {
            instance.y = closed_y - slid as f64;
        } else {
            instance.x = closed_x - slid as f64;
        }
    }
    let crystals: Vec<_> = world.ids_of(ObjectId::HdCrystal).collect();
    for crystal in crystals {
        world.instances[crystal].vars = ObjectVars::Crystal { lit: open, usable: crystals_usable };
        world.instances[crystal].image_index = if open { 1.0 } else { 0.0 };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crystals_rest_after_a_toggle() {
        let rules = HauntingDreamRules::default();
        let mut doors = Doors::default();
        doors.toggle(&rules);
        assert!(doors.open);
        doors.toggle(&rules);
        assert!(doors.open, "resting crystals do nothing");
        doors.rest = 0;
        doors.toggle(&rules);
        assert!(!doors.open);
    }
}
