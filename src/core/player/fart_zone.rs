//! What Fart Zone does to a player: hits on the training dummy (obj_fart_dummy Step)
//! and the statues' curse (obj_fart_mermer, obj_fart_controller), which turns its
//! holder into a statue that must be passed on by touch before it runs out.

use super::amy::AMY_HJUMP;
use super::animation::{FALL, JUMP, SPIN};
use super::{Character, ExeCharacter, Player};
use crate::core::resources::names::{sound, sprite};
use crate::core::collision::sprite_bbox;
use crate::core::config::{ticks, GameplayConfig, ticks_per_second};
use crate::core::events::SimEvent;
use crate::core::objects::ids::ObjectId;
use crate::core::world::{ObjectVars, World};

/// Amy's hammer jump hits the dummy in its first frames.
const HAMMER_JUMP_HITTING_FRAMES: f64 = 3.0;
/// CLIENT_FART_PUSH carries the push times this, as a whole number.
const PUSH_SCALE: f64 = 3.0;

impl Player {
    pub(super) fn hit_dummy(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let Some(dummy) = world.ids_of(ObjectId::FartDummy).next() else { return };
        if self.dummy_rest > 0 {
            self.dummy_rest -= 1;
            return;
        }
        let resting = matches!(world.instances[dummy].vars, ObjectVars::Dummy { resting: true, .. });
        let body = sprite_bbox(world.sprites.get(self.sprite_index), self.x, self.y, self.image_xscale, 1.0, 0.0);
        if resting || !world.bbox(dummy).is_some_and(|bbox| bbox.overlaps(&body)) {
            return;
        }
        let (x, y) = (world.instances[dummy].x, world.instances[dummy].y);
        let demon = self.character == Character::Exe || self.revival_times >= 2;
        let facing = self.image_xscale;
        let rolling = matches!(self.state, JUMP | FALL | SPIN);
        let (worth, push) = if demon && rolling && !matches!(self.character, Character::Eggman | Character::Amy | Character::Sally) {
            (-1, facing * PUSH_SCALE)
        } else {
            let attacking = self.is_attacking || (self.character == Character::Amy && self.state == AMY_HJUMP && self.image_index < HAMMER_JUMP_HITTING_FRAMES);
            if !attacking {
                return;
            }
            let (worth, push) = match (self.character, self.exe_character, self.revival_times >= 2) {
                (Character::Exe, ExeCharacter::Chaos, _) => (-1, 1.0),
                (Character::Exe, _, _) => (-2, 2.0),
                (Character::Knux | Character::Amy, _, false) => (3, 1.5),
                (Character::Eggman | Character::Sally, _, false) => (2, 1.0),
                (Character::Knux | Character::Eggman | Character::Amy | Character::Sally, _, true) => (-1, 1.0),
                _ => (0, 0.0),
            };
            (worth, facing * push * PUSH_SCALE)
        };
        events.push(SimEvent::DummyPushed { speed: push.trunc() });
        events.push(SimEvent::DummyHit { worth, x, y, everyone: false });
        self.dummy_rest = crate::core::config::original_ticks(cfg.levels.fart_zone.dummy_rest_ticks);
    }

    /// obj_fart_mermer Step: touching a statue curses the player.
    pub(super) fn touch_statue(&mut self, world: &World, cfg: &GameplayConfig) {
        if self.potato_ticks <= 0 && self.meeting_at(world, self.x, self.y, ObjectId::FartMermer) {
            self.potato_ticks = ticks(cfg.levels.fart_zone.potato_seconds);
        }
    }

    /// obj_fart_controller Draw Begin: the curse counts down, roaring every second, and
    /// ends the holder's game when it runs out; the holder looks like the statue.
    pub(super) fn hold_the_curse(&mut self, events: &mut Vec<SimEvent>) {
        if self.potato_ticks <= 0 {
            return;
        }
        self.potato_ticks -= 1;
        self.sprite_index = sprite::SPR_MERFURMU;
        if self.potato_ticks % ticks_per_second() as i32 == 0 {
            events.push(SimEvent::Sound { sound: sound::SND_ROAR, x: self.x, y: self.y });
        }
        if self.potato_ticks <= 0 {
            events.push(SimEvent::PotatoBoom);
        }
    }

    /// obj_fart_controller Step: a holder whose curse has been on long enough gives it to
    /// the first player it touches.
    pub fn can_pass_the_curse(&self, cfg: &GameplayConfig) -> bool {
        let rules = &cfg.levels.fart_zone;
        self.potato_ticks > 0 && self.potato_ticks <= ticks(rules.potato_seconds - rules.potato_passes_after_seconds)
    }
}
