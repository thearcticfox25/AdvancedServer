//! Not Perfect as clients show it: the next stage's shadow during the warning, the white
//! flash of a switch, obj_np_background's scrolling sky and floating shapes, and the
//! decoy big rings (obj_npring). The server switches the stages (sim level/not_perfect.rs).

use crate::client::canvas::Canvas;
use crate::client::room::{LayerContent, Room};
use crate::core::resources::names::sprite;
use crate::core::collision::sprite_bbox;
use crate::core::objects::ids::ObjectId;
use crate::core::world::{ObjectVars, World, C_WHITE};
use macroquad::rand::gen_range;
use crate::core::config::step;

/// obj_np_background: depth 250; obj_np_white over everything.
pub const PARTICLE_DEPTH: i32 = 250;
/// The shadow of the next stage: obj_tile at depth 90, fading in over five seconds.
const SHADOW_DEPTH: i32 = 90;
const SHADOW_FADE_IN: f64 = 0.01666 / 5.0;
/// obj_np_white: its alpha drops by this a step.
const WHITE_FADE: f64 = 0.1;
/// obj_np_background: the sky's frames up to 107 drift sideways, the ones after rise.
const RISING_FROM_FRAME: f64 = 107.0;
const RISING_TO_FRAME: f64 = 217.0;
const SKY_SPEED_STEP: f64 = 0.5;
const SIDEWAYS_SHAPES: usize = 15;
const RISING_SHAPES: usize = 10;
/// The red the sky turns once the big ring is out.
const BIG_RING_SKY_TINT: u32 = 0x0000FF;
/// obj_npring: fades in by this a step once the big ring is out.
const DECOY_FADE_IN: f64 = 0.032;

pub struct NotPerfect {
    warning: bool,
    shadow_alpha: f64,
    pub white: f64,
    sky_speed: f64,
    rising: bool,
    big_ring_out: bool,
    sideways: Vec<(f64, f64)>,
    rising_shapes: Vec<(f64, f64, f64)>,
}

impl NotPerfect {
    /// Room creation code and obj_np_background Create.
    pub fn open(room: &mut Room) -> NotPerfect {
        let not_perfect = NotPerfect {
            warning: false,
            shadow_alpha: 0.0,
            white: 0.0,
            sky_speed: 0.0,
            rising: false,
            big_ring_out: false,
            sideways: (0..SIDEWAYS_SHAPES).map(|row| (gen_range(0, 25) as f64 * 20.0, row as f64 * 24.0)).collect(),
            rising_shapes: (0..RISING_SHAPES).map(|column| (column as f64 * 48.0, column as f64 * 48.0, gen_range(0, 28) as f64 * 10.0)).collect(),
        };
        not_perfect.show_shadow(room);
        not_perfect
    }

    /// SERVER_NPCONTROLLER_STATE 1.
    pub fn switched(&mut self) {
        self.white = 1.0;
    }

    /// `big_ring`: where the big ring is, once it is out; `current_time_ms` sways the rising shapes.
    pub fn step(&mut self, room: &mut Room, world: &mut World, big_ring: Option<(f64, f64)>, current_time_ms: f64) {
        let warning = world.ids_of(ObjectId::NpController).any(|id| matches!(world.instances[id].vars, ObjectVars::NotPerfectController { warning: true, .. }));
        if warning != self.warning {
            self.warning = warning;
            self.shadow_alpha = 0.0;
        } else if warning && self.shadow_alpha < 1.0 {
            self.shadow_alpha += SHADOW_FADE_IN * step();
        }
        self.show_shadow(room);
        self.white = (self.white - WHITE_FADE * step()).max(0.0);

        // obj_np_background Draw Begin: once the big ring is out the sky turns red; the sky
        // follows the view and drifts.
        if let (Some(ring), false) = (big_ring, self.big_ring_out) {
            self.big_ring_out = true;
            // obj_np_background Draw Begin: the stage tiles turn grey too (_filter_greyscale).
            for name in ["Tiles", "ShadowTiles"] {
                if let Some(layer) = room.layer_mut(name) {
                    layer.greyscale = true;
                }
            }
            if let Some(LayerContent::Background(colour)) = room.layer_mut("Colour").map(|layer| &mut layer.content) {
                colour.colour = 0x000000;
            }
            self.white = 1.0;
            // SERVER_GAME_SPAWN_RING shows the decoys; obj_npring Step destroys the one on the real ring.
            let real = sprite_bbox(world.sprites.get(sprite::SPR_BIGRING), ring.0, ring.1, 1.0, 1.0, 0.0);
            let decoys: Vec<_> = world.ids_of(ObjectId::Npring).collect();
            for decoy in decoys {
                let on_the_ring = world.bbox(decoy).is_some_and(|bbox| bbox.overlaps(&real));
                world.instances[decoy].visible = !on_the_ring;
            }
        }
        if let Some(LayerContent::Background(sky)) = room.layer_mut("Background").map(|layer| &mut layer.content) {
            if sky.image_index >= RISING_FROM_FRAME {
                self.rising = true;
            }
            if sky.image_index >= RISING_TO_FRAME {
                self.rising = false;
            }
            self.sky_speed += if self.rising { -SKY_SPEED_STEP } else { SKY_SPEED_STEP };
            (sky.parallax_x, sky.parallax_y, sky.x, sky.y) = (0.0, 0.0, self.sky_speed, 0.0);
            sky.colour = if self.big_ring_out { BIG_RING_SKY_TINT } else { C_WHITE };
        }
        for instance in world.instances.iter_mut().filter(|instance| instance.object == ObjectId::Npring && instance.visible) {
            if instance.image_alpha < 1.0 {
                instance.image_alpha += DECOY_FADE_IN * step();
            }
        }

        // obj_np_background Draw moves its shapes.
        if self.rising {
            for (index, shape) in self.rising_shapes.iter_mut().enumerate() {
                shape.0 = shape.1 + (current_time_ms / 400.0 + index as f64).sin() * 2.0;
                shape.2 -= crate::core::config::step();
                if shape.2 <= -16.0 {
                    shape.2 = 286.0 + gen_range(0, 33) as f64;
                }
            }
        } else {
            for shape in &mut self.sideways {
                shape.0 -= 1.0;
                if shape.0 <= -64.0 {
                    shape.0 = 500.0;
                }
            }
        }
    }

    fn show_shadow(&self, room: &mut Room) {
        for layer in room.layers.iter_mut().filter(|layer| layer.depth == SHADOW_DEPTH) {
            if let LayerContent::Decorations(decorations) = &mut layer.content {
                decorations.iter_mut().for_each(|decoration| decoration.alpha = if self.warning { self.shadow_alpha } else { 0.0 });
            }
        }
    }

    /// obj_np_background Draw: shapes drift left over the view, or rise while the sky rises.
    pub fn draw_particles(&self, canvas: &mut Canvas, view: (f64, f64)) {
        let frame = if self.big_ring_out { 1.0 } else { 0.0 };
        if self.rising {
            for shape in &self.rising_shapes {
                canvas.draw_sprite(sprite::BACKGROUND_NOTPERF2, frame, shape.0 + view.0, shape.2 + view.1);
            }
        } else {
            for shape in &self.sideways {
                canvas.draw_sprite(sprite::BACKGROUND_NOTPERF3, frame, shape.0 + view.0, shape.1 + view.1);
            }
        }
    }
}
