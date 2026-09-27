//! Exeller's clone (obj_exeller_clone + AdvancedServer entities/exeller_clone.rs):
//! stands where it was placed, is a teleport destination, and shows its owner the
//! nearest survivor within reach.

use crate::core::resources::names::sprite;
use crate::core::resources::SpriteId;
use crate::core::collision::{sprite_bbox, Bbox};
use crate::core::config::GameplayConfig;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Character, Player};
use crate::core::rooms::ids::RoomId;
use crate::core::world::{ObjectVars, World};

#[derive(Clone, Debug)]
pub struct ExellerClone {
    /// Matches the value stored in Player::clones.
    pub id: i32,
    /// Index of the Exeller player who placed it.
    pub owner: usize,
    pub x: f64,
    pub y: f64,
    pub image_xscale: f64,
    pub sprite_index: SpriteId,
    /// Survivor this clone currently reveals to its owner (only the owner may be sent this).
    pub revealed: Option<usize>,
}

impl ExellerClone {
    /// SERVER_NPCONTROLLER_STATE 1, and the pushers pushing a clone out of their walls.
    pub fn move_to_next_stage(&mut self, world: &World, dx: f64, dy: f64) {
        self.x += dx;
        self.y += dy;
        const HALF: f64 = 22.0;
        for (object, step) in [(ObjectId::LeftPusher, -1.0), (ObjectId::RightPusher, 1.0)] {
            for wall in world.ids_of(object) {
                let Some(bbox) = world.bbox(wall) else { continue };
                while self.x - HALF < bbox.right && self.x + HALF >= bbox.left && self.y - HALF < bbox.bottom && self.y + HALF >= bbox.top {
                    self.x += step;
                }
            }
        }
    }

    /// obj_np_teleporn Step: a clone outside the current stage's area is put back into it.
    fn stay_on_stage(&mut self, world: &World) {
        let Some(&ObjectVars::NotPerfectController { stage, .. }) = world.ids_of(ObjectId::NpController).next().map(|id| &world.instances[id].vars) else { return };
        let body = crate::core::collision::sprite_bbox(world.sprites.get(self.sprite_index), self.x, self.y, self.image_xscale, 1.0, 0.0);
        for area in world.ids_of(ObjectId::NpTeleporn) {
            let ObjectVars::StageArea { stage: area_stage, back_x, back_y } = world.instances[area].vars else { continue };
            if area_stage == stage && !world.bbox(area).is_some_and(|bbox| bbox.overlaps(&body)) {
                self.x = back_x;
                self.y = back_y;
            }
        }
    }

    pub fn new(id: i32, owner: usize, x: f64, y: f64, dir: f64) -> ExellerClone {
        ExellerClone { id, owner, x, y, image_xscale: dir, sprite_index: sprite::SPR_EXELLER_CLONE, revealed: None }
    }

    pub fn tick(&mut self, world: &World, players: &[Player]) {
        let cfg = &world.config;
        let exeller = &cfg.exeller;
        let standing = world.position_meeting(self.x, self.y + exeller.clone_floor_check_below, ObjectId::FloorParent);
        self.sprite_index = if standing { sprite::SPR_EXELLER_CLONE } else { sprite::SPR_EXELLER_CLONE2 };
        self.push_out_of_doors(world);
        self.stay_on_stage(world);
        if players.get(self.owner).is_some_and(|owner| owner.state == crate::core::player::IDLE) && self.sprite_index == sprite::SPR_EXELLER_CLONE {
            // The standing clone copies the idle pose while its owner idles.
            self.sprite_index = sprite::SPR_EXELLER_IDLE;
        }
        self.revealed = self.nearest_survivor(world, cfg, players);
        self.push_out_of_rotating_floor(world, cfg);
    }

    /// Haunting Dream doors can close on a clone: it slides right until free.
    fn push_out_of_doors(&mut self, world: &World) {
        let doors: Vec<Bbox> = world.ids_of(ObjectId::HdDoor).filter_map(|id| world.bbox(id)).collect();
        // ponytail: bounding boxes only; a pathological door stack could need many steps
        while doors.iter().any(|door| door.overlaps(&self.bbox(world))) {
            self.x += 1.0;
        }
    }

    /// Not Perfect rotates the map: a clone caught in the floor rises until it is out
    /// (obj_exeller_clone Draw_76, which the simulation runs every tick).
    fn push_out_of_rotating_floor(&mut self, world: &World, cfg: &GameplayConfig) {
        if world.room != Some(RoomId::Notperfect) {
            return;
        }
        let [left, top, right, bottom] = cfg.exeller.clone_floor_push_rect;
        while world.collision_rectangle(self.x + left, self.y + top, self.x + right, self.y + bottom, ObjectId::FloorParent).is_some() {
            self.y -= 1.0;
        }
    }

    fn bbox(&self, world: &World) -> Bbox {
        sprite_bbox(world.sprites.get(self.sprite_index), self.x, self.y, self.image_xscale, 1.0, 0.0)
    }

    /// distance_to_object: the gap between bounding boxes, 0 when they overlap.
    fn nearest_survivor(&self, world: &World, cfg: &GameplayConfig, players: &[Player]) -> Option<usize> {
        let clone_box = self.bbox(world);
        players
            .iter()
            .enumerate()
            .filter(|(_, player)| player.character != Character::Exe && player.hp > 0 && !player.is_demonized())
            .map(|(index, player)| {
                let body = sprite_bbox(world.sprites.get(player.sprite_index), player.x, player.y, player.image_xscale, 1.0, 0.0);
                (index, box_distance(&clone_box, &body))
            })
            .filter(|(_, distance)| *distance < cfg.exeller.clone_reveal_distance)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| index)
    }
}

fn box_distance(a: &Bbox, b: &Bbox) -> f64 {
    let dx = (a.left - b.right).max(b.left - a.right).max(0.0);
    let dy = (a.top - b.bottom).max(b.top - a.bottom).max(0.0);
    dx.hypot(dy)
}
