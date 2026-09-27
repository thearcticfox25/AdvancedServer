//! Dark Tower as clients show it: the stalactites' fade, drips, notice and shards, the
//! fog and the darkness over the view, and the Tails Doll's jumpscare. The server moves
//! the balls, the stalactites and the doll (sim level/dark_tower.rs).

use crate::client::canvas::{Canvas, VIEW_HEIGHT, VIEW_WIDTH};
use crate::core::resources::names::{sound, sprite};
use crate::core::resources::SoundId;
use crate::core::collision::sprite_bbox;
use crate::core::objects::ids::ObjectId;
use crate::core::player::Player;
use crate::core::world::{InstanceId, ObjectVars, World, C_WHITE};
use macroquad::rand::gen_range;
use crate::core::config::step;
use crate::core::config::ticks;

/// obj_darktower_liquid: made at the stalactite's depth; obj_darktower_stalactitepart -20.
pub const DRIP_DEPTH: i32 = 101;
pub const SHARD_DEPTH: i32 = -20;

/// obj_darktower_stalactite End Step.
/// xx2 = random_range(-60, 60): how much the view may grow or shrink in a flicker.
const JUMPSCARE_VIEW_JOLT: f64 = 60.0;
const STALACTITE_FADE_IN: f64 = 0.016;
/// `current_time % 8 == 0`: about one step in eight makes a drip.
const DRIP_ONE_IN: i32 = 8;
const DRIP_FROM: (f64, f64) = (40.0, 40.0);
const DRIP_SPREAD: (i32, i32) = (14, 10);
const DRIP_SPEEDS: [f64; 4] = [5.0, 2.5, 4.0, 3.0];
/// Broken on the floor: the camera shakes for a player this close (the strength measured over 500 px).
const BREAK_SHAKE_REACH: f64 = 150.0;
const BREAK_SHAKE_SCALE: f64 = 500.0;
const SHARD_FROM_X: f64 = 40.0;
const SHARD_SPREAD: (i32, i32) = (4, 10);
const SHARD_THROWS: [f64; 4] = [4.0, 5.0, 6.0, 7.0];
const SHARD_GRAVITY: f64 = 0.2;
const SHARD_WIND: f64 = 0.05;

/// obj_darktower_darkness Draw GUI: clear for ten seconds, then dark for one, fading
/// by this much a step.
const LIGHT_SECONDS: f64 = 10.0;
const DARK_SECONDS: f64 = 1.0;
const DARKNESS_FADE: f64 = 0.016;
/// obj_darktower_fog: sways 200 px either way from half its width left, a radian every 4 s.
const FOG_LEFT: f64 = -495.0 / 2.0;
const FOG_SWAY: f64 = 200.0;
const FOG_MILLISECONDS_PER_RADIAN: f64 = 4000.0;
/// obj_darktower_jumpscare: shown a second; the red screen at 0.4, a frame every 200 ms.
const JUMPSCARE_SECONDS: f64 = 1.0;
const JUMPSCARE_RED_ALPHA: f64 = 0.4;
const JUMPSCARE_RED_MILLISECONDS: f64 = 200.0;

pub struct DarkTower {
    stalactites_seen: Vec<(bool, bool)>,
    drips: Vec<Drip>,
    shards: Vec<Shard>,
    darkness: Option<Darkness>,
    fog: bool,
    jumpscare_ticks: i32,
    /// How much wider and taller than the picture the view is while the face flickers
    /// (camera_set_view_size(480 + xx2, 270 + xx2)); 0 once the jumpscare is over.
    pub view_growth: f64,
}

struct Drip {
    x: f64,
    y: f64,
    speed: f64,
    frame: Option<f64>,
}

struct Shard {
    x: f64,
    y: f64,
    xspd: f64,
    yspd: f64,
    dir: f64,
    frame: f64,
}

#[derive(Default)]
struct Darkness {
    timer: i32,
    dark: bool,
    alpha: f64,
}

/// What a step asks of the level screen.
#[derive(Default)]
pub struct Heard {
    /// Sounds from these places, measured from the view's centre.
    pub sounds: Vec<(SoundId, f64, f64)>,
    /// scr_camera_shake(25, magnitude, 0.2)
    pub shake: Option<f64>,
}

impl DarkTower {
    pub fn open(world: &World) -> DarkTower {
        let placed = |object| world.ids_of(object).next().is_some();
        DarkTower {
            stalactites_seen: stalactites(world).iter().map(|&id| seen(world, id)).collect(),
            drips: Vec::new(),
            shards: Vec::new(),
            darkness: placed(ObjectId::DarktowerDarkness).then(Darkness::default),
            fog: placed(ObjectId::DarktowerFog),
            jumpscare_ticks: 0,
            view_growth: 0.0,
        }
    }

    /// SERVER_DTTAILSDOLL_STATE 1 on this player's client.
    pub fn scare(&mut self) {
        self.jumpscare_ticks = ticks(JUMPSCARE_SECONDS);
    }

    pub fn step(&mut self, world: &mut World, own: Option<&Player>) -> Heard {
        let mut heard = Heard::default();
        let floors: Vec<InstanceId> = world.ids_of(ObjectId::FloorParent).collect();
        for (&id, before) in stalactites(world).iter().zip(&mut self.stalactites_seen) {
            let now = seen(world, id);
            let instance = &mut world.instances[id];
            let (x, y) = (instance.x, instance.y);
            if now.1 && !before.1 {
                heard.sounds.push((sound::SND_STALACTITE_NOTICE, x, y));
            }
            if before.0 && !now.0 {
                // _destroy
                heard.sounds.push((sound::SND_SNOWBALL_BREAK, x, y));
                for (dir, index) in [(-1.0, 0), (-1.0, 1), (1.0, 0), (1.0, 1)] {
                    let throw = SHARD_THROWS[gen_range(0, SHARD_THROWS.len())];
                    self.shards.push(Shard {
                        x: x + SHARD_FROM_X + gen_range(-SHARD_SPREAD.0, SHARD_SPREAD.0 + 1) as f64,
                        y: y + gen_range(-SHARD_SPREAD.1, SHARD_SPREAD.1 + 1) as f64,
                        xspd: 1.0 + dir * index as f64 / 10.0,
                        yspd: -throw,
                        dir,
                        frame: gen_range(0, 4) as f64,
                    });
                }
                if let Some(own) = own {
                    let body = sprite_bbox(world.sprites.get(own.sprite_index), own.x, own.y, own.image_xscale, 1.0, 0.0);
                    let stalactite = sprite_bbox(world.sprites.get(sprite::SPR_DARKTOWER_STALACTITE), x, y, 1.0, 1.0, 0.0);
                    let distance = stalactite.distance(&body);
                    if distance < BREAK_SHAKE_REACH {
                        heard.shake = Some(1.0 - distance / BREAK_SHAKE_SCALE);
                    }
                }
            }
            *before = now;
            if !instance.visible {
                continue;
            }
            if instance.image_alpha < 1.0 {
                instance.image_alpha += STALACTITE_FADE_IN * step();
            }
            if gen_range(0, DRIP_ONE_IN) == 0 {
                self.drips.push(Drip {
                    x: x + DRIP_FROM.0 + gen_range(-DRIP_SPREAD.0, DRIP_SPREAD.0 + 1) as f64,
                    y: y + DRIP_FROM.1 + gen_range(-DRIP_SPREAD.1, DRIP_SPREAD.1 + 1) as f64,
                    speed: DRIP_SPEEDS[gen_range(0, DRIP_SPEEDS.len())],
                    frame: None,
                });
            }
        }

        let world = &*world;
        let drip_sprite = world.sprites.get(sprite::SPR_DARKTOWER_STALACTITE1);
        let on_floor = |x: f64, y: f64| {
            let body = sprite_bbox(drip_sprite, x, y, 1.0, 1.0, 0.0);
            floors.iter().any(|&floor| world.bbox(floor).is_some_and(|bbox| bbox.overlaps(&body)))
        };
        self.drips.retain_mut(|drip| match drip.frame.as_mut() {
            None => {
                drip.y += drip.speed * step();
                if on_floor(drip.x, drip.y) {
                    drip.y = drip.y.floor();
                    while on_floor(drip.x, drip.y) {
                        drip.y -= 1.0;
                    }
                    drip.frame = Some(0.0);
                }
                drip.y < world.room_height
            }
            Some(frame) => {
                *frame += drip_sprite.fps as f64 / crate::core::config::ticks_per_second();
                *frame < drip_sprite.frame_count as f64 - 1.0
            }
        });

        let shard_sprite = world.sprites.get(sprite::SPR_DARKTOWER_STALACTITE2);
        self.shards.retain_mut(|shard| {
            shard.yspd += SHARD_GRAVITY * step();
            shard.xspd += shard.dir * SHARD_WIND;
            shard.x += shard.xspd;
            shard.y += shard.yspd;
            let body = sprite_bbox(shard_sprite, shard.x, shard.y, 1.0, 1.0, 0.0);
            // instance_place finds one floor; an invisible one keeps the shard falling.
            let floor = floors.iter().copied().find(|&id| world.bbox(id).is_some_and(|bbox| bbox.overlaps(&body)));
            !floor.is_some_and(|floor| world.instances[floor].visible) && shard.y < world.room_height
        });

        if let Some(darkness) = self.darkness.as_mut() {
            darkness.timer += 1;
            if !darkness.dark && darkness.timer > ticks(LIGHT_SECONDS) {
                darkness.dark = true;
                darkness.timer = 0;
            }
            if darkness.dark && darkness.timer > ticks(DARK_SECONDS) {
                darkness.dark = false;
                darkness.timer = 0;
            }
            if darkness.dark && darkness.alpha < 1.0 {
                darkness.alpha += DARKNESS_FADE * step();
            } else if !darkness.dark && darkness.alpha > 0.0 {
                darkness.alpha -= DARKNESS_FADE * step();
            }
        }
        self.jumpscare_ticks = (self.jumpscare_ticks - 1).max(0);
        // Alarm_0: the view gets its own size back with the end of the jumpscare.
        if self.jumpscare_ticks == 0 {
            self.view_growth = 0.0;
        }
        heard
    }

    pub fn draw_drips(&self, canvas: &mut Canvas) {
        for drip in &self.drips {
            canvas.draw_sprite(sprite::SPR_DARKTOWER_STALACTITE1, drip.frame.unwrap_or(0.0), drip.x, drip.y);
        }
    }

    pub fn draw_shards(&self, canvas: &mut Canvas) {
        for shard in &self.shards {
            canvas.draw_sprite(sprite::SPR_DARKTOWER_STALACTITE2, shard.frame, shard.x, shard.y);
        }
    }

    /// Draw GUI by depth: the fog (0), the darkness (-100), the jumpscare (-105).
    pub fn draw_gui(&mut self, canvas: &mut Canvas, current_time_ms: f64) {
        if self.fog {
            canvas.draw_sprite(sprite::SPR_DARKTOWER_FOG, 0.0, FOG_LEFT + (current_time_ms / FOG_MILLISECONDS_PER_RADIAN).sin() * FOG_SWAY, 0.0);
        }
        if let Some(darkness) = &self.darkness {
            canvas.draw_sprite_ext(sprite::SPR_BLACK, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, C_WHITE, darkness.alpha.clamp(0.0, 1.0));
        }
        if self.jumpscare_ticks > 0 {
            canvas.draw_sprite_ext(sprite::SPR_REDRING_FORE, current_time_ms / JUMPSCARE_RED_MILLISECONDS, 0.0, 0.0, 1.0, 1.0, 0.0, C_WHITE, JUMPSCARE_RED_ALPHA);
            if current_time_ms % gen_range(20, 25) as f64 <= 10.0 {
                let offset = gen_range(-4.0, 4.0);
                // The view jolts too: the room is drawn from a larger or smaller view from
                // the next frame on, which the camera puts back in place but not in size.
                self.view_growth = gen_range(-JUMPSCARE_VIEW_JOLT, JUMPSCARE_VIEW_JOLT);
                let face = canvas.sprites.get(sprite::SPR_DARKTOWER_JUMPSCARE);
                let (width, height) = (face.width as f64, face.height as f64);
                // Its one frame, whatever frame number the original asks for.
                canvas.draw_sprite_ext(sprite::SPR_DARKTOWER_JUMPSCARE, 0.0, offset, 0.0, VIEW_WIDTH / width, VIEW_HEIGHT / height, 0.0, C_WHITE, 1.0);
            }
        }
    }
}

fn stalactites(world: &World) -> Vec<InstanceId> {
    world.ids_of(ObjectId::DarktowerStalactite).collect()
}

/// Whether a stalactite is there, and whether it falls.
fn seen(world: &World, id: InstanceId) -> (bool, bool) {
    let instance = &world.instances[id];
    (instance.visible, matches!(instance.vars, ObjectVars::Stalactite { falling: true, .. }))
}
