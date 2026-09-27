//! Angel Island's ziplines (obj_aiz_zipline). Every client of the original had its
//! own copies, so each player moves its own handles: Player::ziplines lists the
//! handles this player took along; a handle not listed hangs at its start.

use super::{Buttons, Character, ExeCharacter, Player};
use crate::core::resources::names::sound;
use crate::core::collision::sprite_bbox;
use crate::core::config::{step, ticks, GameplayConfig};
use crate::core::events::SimEvent;
use crate::core::objects::ids::ObjectId;
use crate::core::world::{InstanceId, ObjectVars, World};
use serde::{Deserialize, Serialize};

/// Where a rider hangs: this far right of the handle, and per character this far
/// below it plus the handle's height.
const RIDER_OFFSET_X: f64 = 12.0;
/// The rider snaps here on grabbing, before the first move along the line.
const GRAB_OFFSET_Y: f64 = 16.0;
/// A falling handle goes back to its start this far below the camera's top.
const HANDLE_RETURNS_BELOW_VIEW: f64 = 480.0;
/// The camera's top is half a view above the player (obj_camera follows it).
const HALF_VIEW_HEIGHT: f64 = 135.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ZiplineHandle {
    pub zipline: InstanceId,
    pub x: f64,
    pub y: f64,
    has_player: bool,
    progress: f64,
    zspeed: f64,
    zvspeed: f64,
    timeout: i32,
    /// camY: the camera's top when the ride ended.
    cam_y: f64,
}

impl Player {
    /// obj_aiz_zipline Step: a rider is held still before its own Step.
    pub(super) fn hold_on_ziplines(&mut self, world: &World) {
        let riding: Vec<InstanceId> = self.ziplines.iter().filter(|handle| handle.has_player).map(|handle| handle.zipline).collect();
        for zipline in riding {
            if let ObjectVars::Zipline { start, end } = world.instances[zipline].vars {
                self.hold_on_zipline(world, start, end);
            }
        }
    }

    /// obj_aiz_zipline End Step, for every zipline of the room in turn.
    pub(super) fn zipline_end_steps(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        for zipline in world.ids_of(ObjectId::AizZipline) {
            let ObjectVars::Zipline { start, end } = world.instances[zipline].vars else { continue };
            let resting = ZiplineHandle {
                zipline,
                x: world.instances[zipline].x,
                y: world.instances[zipline].y,
                has_player: false,
                progress: 0.0,
                zspeed: cfg.levels.angel_island.zipline_start_speed,
                zvspeed: 0.0,
                timeout: 0,
                cam_y: 0.0,
            };
            let index = self.ziplines.iter().position(|handle| handle.zipline == zipline);
            let mut handle = index.map_or(resting, |index| self.ziplines[index]);
            self.zipline_end_step(world, cfg, &mut handle, start, end, events);
            let at_rest = !handle.has_player && handle.progress <= 0.0 && handle.timeout <= 0 && (handle.x, handle.y) == (resting.x, resting.y);
            match (index, at_rest) {
                (Some(index), true) => {
                    self.ziplines.remove(index);
                }
                (Some(index), false) => self.ziplines[index] = handle,
                (None, false) => self.ziplines.push(handle),
                (None, true) => {}
            }
        }
    }

    fn zipline_end_step(&mut self, world: &World, cfg: &GameplayConfig, handle: &mut ZiplineHandle, start: InstanceId, end: InstanceId, events: &mut Vec<SimEvent>) {
        let rules = &cfg.levels.angel_island;
        if handle.timeout > 0 {
            handle.timeout -= 1;
        }
        let (start_x, start_y) = (world.instances[start].x, world.instances[start].y);
        let (end_x, end_y) = (world.instances[end].x, world.instances[end].y);
        let instance = &world.instances[handle.zipline];
        let meta = world.sprites.get(instance.sprite_index.unwrap_or(crate::core::resources::names::sprite::SPR_AIZ_ZIPLINE));
        let handle_box = sprite_bbox(meta, handle.x, handle.y, instance.image_xscale, instance.image_yscale, instance.image_angle);
        let body = sprite_bbox(world.sprites.get(self.sprite_index), self.x, self.y, self.image_xscale, 1.0, 0.0);

        let can_grab = self.hp > 0 && handle_box.overlaps(&body) && !self.is_grounded && !handle.has_player && handle.progress <= 0.0 && handle.timeout <= 0;
        if can_grab && self.state != self.zipline_state() {
            events.push(SimEvent::Sound { sound: sound::SND_SNAP, x: self.x, y: self.y });
            handle.has_player = true;
            handle.progress = 0.0;
            handle.zspeed = rules.zipline_start_speed;
            handle.zvspeed = 0.0;
            self.x = handle.x + RIDER_OFFSET_X;
            self.y = handle.y + GRAB_OFFSET_Y;
            events.push(SimEvent::CameraOnPlayer);
        }

        if handle.has_player && handle.progress < 1.0 {
            handle.x = start_x + ((end_x - start_x) * handle.progress).floor();
            handle.y = start_y + ((end_y - start_y) * handle.progress).floor();
            self.x = handle.x + RIDER_OFFSET_X;
            self.y = handle.y + self.hang_below_zipline() + meta.height as f64 * instance.image_yscale;
            let lets_go = self.is_dead || self.hp <= 0 || (handle.progress >= rules.zipline_release_after_progress && self.pressed_raw(Buttons::A));
            if lets_go {
                // keyboard_key_release: the press that let go does not jump as well.
                self.ignore_jump_until_released = true;
                self.is_zipline = false;
                handle.progress = 1.0;
            } else {
                self.hold_on_zipline(world, start, end);
            }
            if handle.zspeed < rules.zipline_max_speed {
                handle.zspeed += rules.zipline_acceleration * step();
            }
            handle.progress += handle.zspeed * step();
            if handle.progress >= 1.0 {
                handle.cam_y = self.y.floor() - HALF_VIEW_HEIGHT;
                self.xspd = super::gm_sign(end_x - start_x) * handle.zspeed * rules.zipline_launch_speed_per_progress;
            }
        }

        if handle.progress >= 1.0 {
            handle.zvspeed += rules.zipline_handle_gravity * step();
            handle.timeout = ticks(rules.zipline_regrab_seconds);
            handle.y += handle.zvspeed * step();
            if handle.y >= handle.cam_y + HANDLE_RETURNS_BELOW_VIEW {
                handle.x = start_x;
                handle.y = start_y;
                handle.progress = 0.0;
                handle.zspeed = rules.zipline_start_speed;
                handle.zvspeed = 0.0;
            }
            if !world.game_ends {
                self.controls_enabled = true;
            }
            handle.has_player = false;
        }
    }

    /// _holdPlayer of obj_aiz_zipline.
    fn hold_on_zipline(&mut self, world: &World, start: InstanceId, end: InstanceId) {
        self.xspd = 0.0;
        self.yspd = 0.0;
        self.image_xscale = super::gm_sign(world.instances[end].x - world.instances[start].x);
        self.is_spinning = false;
        self.is_jumping = false;
        self.is_grounded = false;
        self.is_hurt = false;
        self.is_attacking = false;
        self.is_zipline = true;
        self.angle = 0.0;
        self.controls_enabled = false;
    }

    /// How far below the handle this character hangs; some abilities stop on the line.
    fn hang_below_zipline(&mut self) -> f64 {
        match self.character {
            Character::Tails | Character::Cream => self.is_flying = false,
            Character::Knux => {
                self.is_gliding = false;
                if self.is_stuck {
                    self.can_move = true;
                    self.is_stuck = false;
                }
            }
            Character::Amy => self.is_hj = false,
            _ => {}
        }
        hang_below_zipline(self.character)
    }

    /// The state of riding a zipline, which others draw the handle over (obj_player_puppet Draw).
    pub fn zipline_state(&self) -> usize {
        match (self.character, self.exe_character) {
            (Character::Exe, ExeCharacter::Chaos) => super::chaos::CHAOS_ZIPLINE,
            (Character::Exe, _) => super::exe::EXE_ZIPLINE,
            (Character::Tails, _) => super::tails::TAILS_ZIPLINE,
            (Character::Knux, _) => super::knux::KNUX_ZIPLINE,
            (Character::Eggman, _) => super::egg::EGG_ZIPLINE,
            (Character::Amy, _) => super::amy::AMY_ZIPLINE,
            (Character::Cream, _) => super::cream::CREAM_ZIPLINE,
            (Character::Sally, _) => super::sally::SALLY_ZIPLINE,
        }
    }
}

/// _holdPlayer of obj_aiz_zipline: how far below the handle each character hangs.
pub fn hang_below_zipline(character: Character) -> f64 {
    match character {
        Character::Exe | Character::Knux => 22.0,
        Character::Tails => 11.0,
        Character::Eggman => 25.0,
        Character::Amy => 21.0,
        Character::Cream => 8.0,
        Character::Sally => 18.0,
    }
}
