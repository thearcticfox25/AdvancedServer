//! Desert Town's buildings (obj_deserttown_i): inside one, its outer walls (the art at
//! depth -400 and the TilesBalls fog) fade out and its insides (depth -300) fade in.

use crate::client::canvas::{VIEW_HEIGHT, VIEW_WIDTH};
use crate::client::room::{LayerContent, Room};
use crate::core::objects::ids::ObjectId;
use crate::core::player::Player;
use crate::core::world::World;
use crate::core::config::step;

const INSIDE_DEPTH: i32 = -300;
const OUTSIDE_DEPTH: i32 = -400;
const FADE_PER_STEP: f64 = 0.1;

#[derive(Default)]
pub struct DesertTown {
    /// value: how far the insides show.
    shown: f64,
    /// coll: whether this player (or the camera, without one) is inside.
    inside: bool,
}

impl DesertTown {
    /// Step of obj_deserttown_i.
    pub fn step(&mut self, room: &mut Room, world: &World, own: Option<&Player>, view: (f64, f64)) {
        let (x, y) = match own.filter(|own| (own.hp > 0 || own.is_demonized()) && !world.game_ends) {
            Some(own) => (own.x, own.y),
            None => (view.0 + VIEW_WIDTH / 2.0, view.1 + VIEW_HEIGHT / 2.0),
        };
        self.inside = world.position_meeting(x, y, ObjectId::DeserttownTrigger);
        if self.inside {
            if self.shown < 1.0 {
                self.shown += FADE_PER_STEP * step();
            }
        } else if self.shown > 0.0 {
            self.shown -= FADE_PER_STEP * step();
        }
        for layer in &mut room.layers {
            let alpha = match layer.depth {
                INSIDE_DEPTH => self.shown,
                OUTSIDE_DEPTH => 1.0 - self.shown,
                _ => continue,
            };
            match &mut layer.content {
                LayerContent::Decorations(decorations) => decorations.iter_mut().for_each(|decoration| decoration.alpha = alpha),
                LayerContent::Background(background) => background.alpha = alpha,
                LayerContent::Instances(_) => {}
            }
        }
    }

    /// obj_player_puppet and obj_exeller_clone Draw GUI: no name plate for someone on
    /// the other side of a building's wall.
    pub fn shows_plate_at(&self, world: &World, x: f64, y: f64) -> bool {
        world.position_meeting(x, y, ObjectId::DeserttownTrigger) == self.inside
    }
}
