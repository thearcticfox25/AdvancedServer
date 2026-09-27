//! Volcano Valley as clients show it: the lava columns' hum and roar, and the shards of
//! broken vases. The server moves the lava and breaks the vases (sim level/lava.rs,
//! level/vases.rs); Echidna Ruins' lava platform sounds the same; the barrels are map data (the Bochki layer their Create moved them to).

use crate::client::canvas::Canvas;
use crate::core::resources::names::{sound, sprite};
use crate::core::resources::SoundId;
use crate::core::collision::sprite_bbox;
use crate::core::objects::ids::ObjectId;
use crate::core::world::{InstanceId, ObjectVars, World, C_WHITE};
use macroquad::rand::gen_range;
use crate::core::config::step;
use crate::core::config::ticks;

/// obj_vv_vasepiece: depth 249.
pub const SHARD_DEPTH: i32 = 249;
/// obj_vv_lavacolumn Step: `timer++ >= 18` roars every 19 steps while it rises or is up.
const ROAR_EVERY_SECONDS: f64 = 19.0 / 60.0;
/// obj_vv_vasepiece: gravity; on the floor it slows by this a step.
const SHARD_GRAVITY: f64 = 0.32;
const SHARD_FRICTION: f64 = 0.1;
/// It checks the floor this far under it and walls a pixel above it.
const SHARD_FLOOR_BELOW: f64 = 4.0;

pub struct VolcanoValley {
    roar_timers: Vec<i32>,
    vases_seen: Vec<bool>,
    shards: Vec<Shard>,
}

struct Shard {
    x: f64,
    y: f64,
    xspd: f64,
    yspd: f64,
    frame: f64,
    angle: f64,
    stopped: bool,
}

/// What a step asks of the level screen.
#[derive(Default)]
pub struct Heard {
    /// Sounds from these places (emitters measured from the left ear).
    pub sounds: Vec<(SoundId, f64, f64)>,
    /// Each column's looping snd_lava: its number and place.
    pub hums: Vec<(u32, f64, f64)>,
}

impl VolcanoValley {
    pub fn open(world: &World) -> VolcanoValley {
        VolcanoValley {
            roar_timers: vec![0; lava(world).len()],
            vases_seen: world.ids_of(ObjectId::VvVase).map(|id| world.instances[id].visible).collect(),
            shards: Vec::new(),
        }
    }

    pub fn step(&mut self, world: &World) -> Heard {
        let mut heard = Heard::default();
        let columns = lava(world);
        for (&column, timer) in columns.iter().zip(&mut self.roar_timers) {
            let instance = &world.instances[column];
            let ObjectVars::LavaColumn { nid, state, .. } = instance.vars else { continue };
            heard.hums.push((nid as u32, instance.x, instance.y));
            *timer += 1;
            if *timer >= ticks(ROAR_EVERY_SECONDS) {
                // obj_vv_lavacolumn: states 2 and 3 roar.
                if matches!(state, 2 | 3) {
                    heard.sounds.push((sound::SND_LAVAAPPEAR, instance.x, instance.y));
                }
                *timer = 0;
            }
        }

        // SERVER_VVVASE_STATE: six to eight shards fly off (the loop draws its bound again each time round).
        let vases: Vec<InstanceId> = world.ids_of(ObjectId::VvVase).collect();
        for (&vase, seen) in vases.iter().zip(&mut self.vases_seen) {
            let instance = &world.instances[vase];
            if *seen && !instance.visible {
                let mut made = 0;
                while made < gen_range(6, 9) {
                    let away = if gen_range(0, 3) >= 1 { -1.0 } else { 1.0 };
                    let frame = gen_range(0, world.sprites.get(sprite::SPR_VV_VASEPIECE).frame_count as i32 + 1) as f64;
                    let (xspd, yspd) = (gen_range(2, 5) as f64 * away, gen_range(-4, -1) as f64);
                    self.shards.push(Shard { x: instance.x, y: instance.y, xspd, yspd, frame, angle: 0.0, stopped: false });
                    made += 1;
                }
                heard.sounds.push((sound::SND_VASEBREAK, instance.x, instance.y));
            }
            *seen = instance.visible;
        }

        if self.shards.iter().any(|shard| !shard.stopped) {
            let floors: Vec<InstanceId> = world.ids_of(ObjectId::FloorParent).collect();
            for shard in &mut self.shards {
                shard.step(world, &floors);
            }
        }
        heard
    }

    pub fn draw_shards(&self, canvas: &mut Canvas) {
        for shard in &self.shards {
            canvas.draw_sprite_ext(sprite::SPR_VV_VASEPIECE, shard.frame, shard.x, shard.y, 1.0, 1.0, shard.angle, C_WHITE, 1.0);
        }
    }
}

/// The lava columns, or Echidna Ruins' lava platform, which hums and roars the same.
fn lava(world: &World) -> Vec<InstanceId> {
    world.ids_of(ObjectId::VvLavacolumn).chain(world.ids_of(ObjectId::MarijunaLavaplatform)).collect()
}

impl Shard {
    /// obj_vv_vasepiece Step. It rarely stops for good: its speed shrinks by tenths past zero.
    fn step(&mut self, world: &World, floors: &[InstanceId]) {
        if self.stopped {
            return;
        }
        self.x += self.xspd;
        self.y += self.yspd;
        let bbox = sprite_bbox(world.sprites.get(sprite::SPR_VV_VASEPIECE), self.x, self.y, 1.0, 1.0, self.angle);
        // bbox_right is the last pixel inside.
        if world.position_meeting_any(bbox.right - 1.0 + self.xspd, self.y - 1.0, floors) {
            self.xspd = 0.0;
        }
        if world.position_meeting_any(bbox.left + self.xspd, self.y - 1.0, floors) {
            self.xspd = 0.0;
        }
        if world.position_meeting_any(self.x, self.y + SHARD_FLOOR_BELOW, floors) {
            self.xspd -= crate::core::player::gm_sign(self.xspd) * SHARD_FRICTION;
            self.yspd = 0.0;
            if self.xspd.abs() <= 0.0 {
                self.stopped = true;
            }
            return;
        }
        self.yspd += SHARD_GRAVITY * step();
        self.angle += self.xspd;
    }
}
