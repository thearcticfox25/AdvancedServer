//! What a client shows of rings: the rings of the simulation (obj_ring, obj_redring,
//! drawn only: whoever owns the level decides who takes them), the pieces of a broken
//! ring (obj_ringpart) and the red screen of a red ring (obj_redring_screen).

use crate::client::canvas::{Canvas, C_DKGRAY};
use macroquad::rand;
use crate::core::resources::names::sprite;
use crate::core::resources::SpriteId;
use crate::core::objects::ids::ObjectId;
use crate::core::world::{World, C_WHITE};
use crate::core::config::step;
use crate::core::config::ticks;

/// obj_ring and obj_redring: depth = 2.
pub const RING_DEPTH: i32 = 2;
/// obj_redring_screen: depth = -500.
pub const RED_SCREEN_DEPTH: i32 = -500;
/// obj_level Step: global.ringFrame += 1 / 10, back to 0 after 4.
const RING_FRAMES_PER_STEP: f64 = 0.1;
const RING_LAST_FRAME: f64 = 4.0;
const RED_SCREEN_ALPHA: f64 = 0.6;
/// obj_ringpart: gone after 7 to 9 seconds, fading out in the last one.
const PART_LIFETIME_SECONDS: (i32, i32) = (7, 9);
const PART_FADE_SECONDS: f64 = 1.0;
const PART_GRAVITY: f64 = 0.32;
const PART_FRICTION: f64 = 0.0512;
const PART_SPIN: f64 = 0.1;

/// One of the simulation's rings, drawn where it stands.
pub fn draw_ring(canvas: &mut Canvas, ring: &crate::core::rings::MapRing, ring_frame: f64, act9: bool) {
    let sprite = if ring.red { sprite::SPR_REDRING } else { sprite::SPR_RING };
    let blend = if act9 { C_DKGRAY } else { C_WHITE };
    canvas.draw_sprite_ext(sprite, ring_frame, ring.x, ring.y, 1.0, 1.0, 0.0, blend, ring.alpha);
}

/// global.ringFrame
pub fn next_ring_frame(frame: f64) -> f64 {
    let next = frame + RING_FRAMES_PER_STEP;
    if next > RING_LAST_FRAME {
        0.0
    } else {
        next
    }
}

pub struct RingPart {
    sprite: SpriteId,
    x: f64,
    y: f64,
    xspd: f64,
    yspd: f64,
    angle: f64,
    image_index: f64,
    ticks_left: i32,
    blend: u32,
}

/// CLIENT_RING_BROKE: four pieces fly off the ring with the breaker's speed.
pub fn ring_pieces(x: f64, y: f64, xspd: f64, act9: bool) -> Vec<RingPart> {
    let part = |sprite, x, y, xspd, yspd| RingPart {
        sprite,
        x,
        y,
        xspd,
        yspd,
        angle: 0.0,
        image_index: 0.0,
        ticks_left: rand::gen_range(PART_LIFETIME_SECONDS.0, PART_LIFETIME_SECONDS.1 + 1) * 60,
        blend: if act9 { C_DKGRAY } else { C_WHITE },
    };
    vec![
        part(sprite::SPR_RINGPART, x + 16.0 - 4.0, y + 4.0, 1.0 + xspd, rand::gen_range(-4, -1) as f64),
        part(sprite::SPR_RINGPART2, x, y + 16.0 - 4.0, -1.0 + xspd, rand::gen_range(1, 3) as f64),
        part(sprite::SPR_RINGPART3, x + 4.0, y + 4.0, -1.0 + xspd, rand::gen_range(-4, -1) as f64),
        part(sprite::SPR_RINGPART4, x + 16.0 - 4.0, y + 16.0 - 4.0, 1.0 + xspd, rand::gen_range(1, 3) as f64),
    ]
}

impl RingPart {
    /// End Step of obj_ringpart: moves, spins, stops at walls and lands on floors.
    /// False once alarm[0] destroyed it.
    pub fn step(&mut self, world: &World) -> bool {
        self.ticks_left -= 1;
        if self.ticks_left <= 0 {
            return false;
        }
        self.x += self.xspd;
        self.y += self.yspd;
        self.angle += self.xspd;
        self.image_index += (self.yspd + self.xspd) * PART_SPIN * step();
        let meta = world.sprites.get(self.sprite);
        let bbox = crate::core::collision::sprite_bbox(meta, self.x, self.y, 1.0, 1.0, self.angle);
        if world.position_meeting(bbox.right + self.xspd, self.y - 1.0, ObjectId::FloorParent) || world.position_meeting(bbox.left + self.xspd, self.y - 1.0, ObjectId::FloorParent) {
            self.xspd = 0.0;
        }
        while world.position_meeting(self.x, self.y + 4.0, ObjectId::FloorParent) {
            self.yspd = 0.0;
            self.y -= 1.0;
        }
        if world.position_meeting(self.x, self.y + 5.0, ObjectId::FloorParent) {
            self.xspd -= self.xspd.abs().min(PART_FRICTION) * crate::core::player::gm_sign(self.xspd);
            return true;
        }
        self.yspd += PART_GRAVITY * step();
        true
    }

    pub fn draw(&self, canvas: &mut Canvas) {
        let fade = ticks(PART_FADE_SECONDS);
        let alpha = if self.ticks_left < fade { self.ticks_left as f64 / fade as f64 } else { 1.0 };
        canvas.draw_sprite_ext(self.sprite, self.image_index, self.x, self.y, 1.0, 1.0, self.angle, self.blend, alpha);
    }
}

/// obj_redring_screen: the red foreground over the view while a red ring lasts.
pub fn draw_red_screen(canvas: &mut Canvas, frame: f64, view: (f64, f64)) {
    canvas.draw_sprite_ext(sprite::SPR_REDRING_FORE, frame, view.0, view.1, 1.0, 1.0, 0.0, C_WHITE, RED_SCREEN_ALPHA);
}
