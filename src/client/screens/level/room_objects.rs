//! Events of the room's instances that only change how they look to this player.

use crate::core::collision::sprite_bbox;
use crate::core::objects::ids::ObjectId;
use crate::core::player::Player;
use crate::core::world::World;

/// obj_warning: fully shown when touching the player, gone 48 px away from it.
const WARNING_FADE_DISTANCE: f64 = 48.0;

/// Step events. Without a player they return early and every look stays as it was.
pub fn step(world: &mut World, own: Option<&Player>) {
    let Some(own) = own else { return };
    let body = sprite_bbox(world.sprites.get(own.sprite_index), own.x, own.y, own.image_xscale, 1.0, 0.0);
    let warnings: Vec<_> = world.ids_of(ObjectId::Warning).collect();
    for warning in warnings {
        let Some(bbox) = world.bbox(warning) else { continue };
        let distance = bbox.distance(&body).clamp(0.0, WARNING_FADE_DISTANCE);
        world.instances[warning].image_alpha = (WARNING_FADE_DISTANCE - distance) / WARNING_FADE_DISTANCE;
    }
}

/// obj_corpse: the body of a character nobody plays lies on the map, once someone
/// else is in the round. `characters` are the numbers everyone in the round plays.
pub fn show_corpses(world: &mut World, characters: &[i32]) {
    let corpses: Vec<_> = world.ids_of(ObjectId::Corpse).collect();
    for corpse in corpses {
        let character = match world.instances[corpse].object {
            ObjectId::CorpseTails => 1,
            ObjectId::CorpseKnux => 2,
            ObjectId::CorpseEggman => 3,
            ObjectId::CorpseAmy => 4,
            ObjectId::CorpseCream => 5,
            _ => continue,
        };
        world.instances[corpse].visible = !characters.contains(&character);
    }
}

