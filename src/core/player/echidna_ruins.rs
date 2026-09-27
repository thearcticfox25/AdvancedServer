//! What Echidna Ruins does to a player: its lava platform burns whoever is in the lava
//! under it and carries whoever stands on it (obj_marijuna_lavaplatform Step and
//! SERVER_MJLAVA_STATE), and a lit crystal reverses the controls
//! (obj_marijuna_crystal Step, obj_marijuna_crystalcontroller swapControls).

use super::hurt::Hit;
use super::{gm_sign, Buttons, Player};
use crate::core::resources::names::sound;
use crate::core::config::{ticks, GameplayConfig};
use crate::core::events::SimEvent;
use crate::core::objects::ids::ObjectId;
use crate::core::world::{ObjectVars, World};

/// The lava fills this part of the platform's sprite, measured from its corner.
const LAVA_LEFT: f64 = 48.0;
const LAVA_RIGHT: f64 = 113.0;
const LAVA_TOP: f64 = 48.0;
const LAVA_BOTTOM: f64 = 672.0;
const LAVA_MIDDLE: f64 = 80.0;
const FEET: f64 = 16.0;
const BODY_RIGHT: f64 = 10.0;
/// A rider stands this far above the platform while it moves.
const RIDER_ABOVE: f64 = 17.0;

impl Player {
    pub(super) fn meet_lava_platform(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let rules = &cfg.levels.echidna_ruins.lava;
        for platform in world.ids_of(ObjectId::MarijunaLavaplatform) {
            let instance = &world.instances[platform];
            // SERVER_MJLAVA_STATE: while it rises or falls, a body on it moves with it.
            if let ObjectVars::LavaColumn { state: 2 | 4, .. } = instance.vars {
                let on_it = [self.sensor_bl, self.sensor_br].iter().any(|sensor| world.collides_point(platform, sensor.x, sensor.y));
                if on_it {
                    self.y = instance.y - RIDER_ABOVE;
                }
            }
            let (x, y) = (instance.x, instance.y);
            if self.y + FEET >= y + LAVA_TOP && self.y <= y + LAVA_BOTTOM && self.x + BODY_RIGHT >= x + LAVA_LEFT && self.x <= x + LAVA_RIGHT {
                let hit = Hit { xpw: gm_sign(self.x - (x + LAVA_MIDDLE)) * rules.knockback_x, ypw: rules.knockback_y, sound: sound::SND_LAVAHIT, ..Hit::damage(cfg, rules.damage) };
                self.hurt(world, cfg, hit, events);
            }
        }
    }

    pub(super) fn touch_crystals(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        if self.controls_reversed > 0 {
            return;
        }
        let body = crate::core::collision::sprite_bbox(world.sprites.get(self.sprite_index), self.x, self.y, self.image_xscale, 1.0, 0.0);
        let crystal = world.ids_of(ObjectId::MarijunaCrystal).find(|&id| {
            let glowing = matches!(world.instances[id].vars, ObjectVars::MjCrystal { lit: true, glow } if glow >= 1.0);
            glowing && world.bbox(id).is_some_and(|bbox| bbox.overlaps(&body))
        });
        if let Some(crystal) = crystal {
            let (x, y) = (world.instances[crystal].x, world.instances[crystal].y);
            self.controls_reversed = ticks(cfg.levels.echidna_ruins.reversed_seconds);
            events.push(SimEvent::Sound { sound: sound::SND_BOOHOO, x: self.x, y: self.y });
            events.push(SimEvent::ControlsReversed { x, y });
        }
    }

    /// global.KeyLeft and the others swapped while a crystal's curse lasts (alarm[0]).
    pub(super) fn reverse_controls(&mut self, buttons: Buttons) -> Buttons {
        if self.controls_reversed <= 0 {
            return buttons;
        }
        self.controls_reversed -= 1;
        let swap = |held: u16, a: u16, b: u16| {
            let mut result = held & !(a | b);
            if held & a != 0 {
                result |= b;
            }
            if held & b != 0 {
                result |= a;
            }
            result
        };
        Buttons(swap(swap(buttons.0, Buttons::LEFT, Buttons::RIGHT), Buttons::UP, Buttons::DOWN))
    }
}
