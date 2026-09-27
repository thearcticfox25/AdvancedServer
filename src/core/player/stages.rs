//! Not Perfect's stages as a player lives them: obj_np_teleporn puts whoever is outside
//! the current stage's area back into it, and right after a switch the pushers
//! push bodies out of walls (their Step and End Step).

use super::Player;
use crate::core::collision::sprite_bbox;
use crate::core::level::push_out_of_walls;
use crate::core::objects::ids::ObjectId;
use crate::core::world::{ObjectVars, World};

impl Player {
    pub(super) fn stay_on_stage(&mut self, world: &World) {
        let Some(ObjectVars::NotPerfectController { stage, switched, .. }) = world.ids_of(ObjectId::NpController).next().map(|id| &world.instances[id].vars) else { return };
        let (stage, switched) = (*stage, *switched);
        if switched {
            push_out_of_walls(world, self);
        }
        for area in world.ids_of(ObjectId::NpTeleporn) {
            let ObjectVars::StageArea { stage: area_stage, back_x, back_y } = world.instances[area].vars else { continue };
            let body = sprite_bbox(world.sprites.get(self.sprite_index), self.x, self.y, self.image_xscale, 1.0, 0.0);
            if area_stage == stage && !world.bbox(area).is_some_and(|bbox| bbox.overlaps(&body)) {
                self.x = back_x;
                self.y = back_y;
            }
        }
    }

    /// End Step of the pushers.
    pub(super) fn leave_walls_after_switch(&mut self, world: &World) {
        let switched = world.ids_of(ObjectId::NpController).any(|id| matches!(world.instances[id].vars, ObjectVars::NotPerfectController { switched: true, .. }));
        if switched {
            push_out_of_walls(world, self);
        }
    }
}
