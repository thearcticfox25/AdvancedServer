//! Nasty Paradise as clients show it: obj_nap_controller's snow, the shards of a
//! breaking ice block, and the snowballs' fade, rumble, shake and pieces. The server
//! decides the blocks and the snowballs (sim level/ice.rs, level/snowballs.rs).

use crate::client::canvas::Canvas;
use crate::core::resources::names::{sound, sprite};
use crate::core::resources::SoundId;
use crate::core::collision::sprite_bbox;
use crate::core::objects::ids::ObjectId;
use crate::core::player::Player;
use crate::core::world::{World, C_WHITE};
use macroquad::rand::gen_range;
use crate::core::config::step;
use crate::core::config::ticks;

/// obj_nap_snow: depth -200; obj_nap_iceblock_part -20; obj_nap_snowball_part -4.
pub const SNOW_DEPTH: i32 = -200;
pub const ICE_SHARD_DEPTH: i32 = -20;
pub const SNOWBALL_PIECE_DEPTH: i32 = -4;

/// obj_nap_controller Step: the flakes made each step, from above the view and from its right side.
const FLAKES_FROM_ABOVE: usize = 3;
const FLAKES_FROM_THE_RIGHT: usize = 2;
const ABOVE_SPAWN_X: (f64, f64) = (-120.0, 560.0);
const RIGHT_SPAWN_X: f64 = 481.0;
const RIGHT_SPAWN_Y: (f64, f64) = (-120.0, 350.0);
/// obj_nap_snow: y += 2 + spd, x -= 3 + spd, gone after two seconds.
const FLAKE_FALL: f64 = 2.0;
const FLAKE_DRIFT: f64 = 3.0;
const FLAKE_SECONDS: f64 = 2.0;

/// obj_nap_iceblock_part: made at the block's right half, falling and blown sideways
/// until it hits a visible floor.
const SHARD_OFFSET_X: f64 = 20.0;
const SHARD_ROW: f64 = 20.0;
const SHARD_THROW: f64 = -3.0;
const SHARD_GRAVITY: f64 = 0.2;
const SHARD_WIND: f64 = 0.05;

/// obj_nap_snowball: fade in per step, rumble every half second, shake within 500 px.
const SNOWBALL_FADE_IN: f64 = 0.016;
const ROLL_SOUND_SECONDS: f64 = 0.5;
const SHAKE_REACH: f64 = 500.0;
const FIRST_ROLLING_FRAME: f64 = 8.0;
/// SERVER_NAPBALL_STATE 2: pieces fly from 48 px above the snowball's bottom.
const PIECES_ABOVE: f64 = 48.0;
const PIECE_SPREAD: f64 = 24.0;
/// obj_nap_snowball_part: gravity, drag, spin; fades after 1 to 1.5 seconds.
const PIECE_GRAVITY: f64 = 0.12;
const PIECE_DRAG: f64 = 0.04;
const PIECE_FADE: f64 = 0.05;

pub struct NastyParadise {
    flakes: Vec<Flake>,
    shards: Vec<Shard>,
    pieces: Vec<Piece>,
    /// Whether each ice block and each snowball was visible at the last step.
    blocks_seen: Vec<bool>,
    balls_seen: Vec<bool>,
    roll_timers: Vec<i32>,
}

struct Flake {
    x: f64,
    y: f64,
    speed: f64,
    ticks_left: i32,
}

struct Shard {
    x: f64,
    y: f64,
    xspd: f64,
    yspd: f64,
    dir: f64,
}

struct Piece {
    x: f64,
    y: f64,
    xspd: f64,
    yspd: f64,
    frame: f64,
    angle: f64,
    alpha: f64,
    ticks_to_fade: i32,
}

/// What a step asks of the level screen.
#[derive(Default)]
pub struct Heard {
    /// Sounds from these places (emitters measured from the left ear).
    pub sounds: Vec<(SoundId, f64, f64)>,
    /// scr_camera_shake(25, magnitude, 0.2)
    pub shake: Option<f64>,
}

impl NastyParadise {
    pub fn open(world: &World) -> NastyParadise {
        let visible = |object| world.ids_of(object).map(|id| world.instances[id].visible).collect::<Vec<_>>();
        let balls_seen = visible(ObjectId::NapSnowball);
        NastyParadise {
            flakes: Vec::new(),
            shards: Vec::new(),
            pieces: Vec::new(),
            blocks_seen: visible(ObjectId::NapIceblock),
            roll_timers: vec![0; balls_seen.len()],
            balls_seen,
        }
    }

    pub fn step(&mut self, world: &mut World, own: Option<&Player>, view: (f64, f64)) -> Heard {
        let mut heard = Heard::default();
        self.step_snow(world, own, view);
        self.step_ice(world, &mut heard);
        self.step_snowballs(world, own, &mut heard);
        self.pieces.retain_mut(Piece::step);
        heard
    }

    /// obj_nap_snow Step and Alarm, then obj_nap_controller Step: no new snow without a
    /// player or while the player is in a sheltered place (obj_nap_trigger).
    fn step_snow(&mut self, world: &World, own: Option<&Player>, view: (f64, f64)) {
        for flake in &mut self.flakes {
            flake.ticks_left -= 1;
            flake.y += (FLAKE_FALL + flake.speed) * step();
            flake.x -= (FLAKE_DRIFT + flake.speed) * step();
        }
        self.flakes.retain(|flake| flake.ticks_left > 0);
        if own.is_none_or(|own| own.meeting_at(world, own.x, own.y, ObjectId::NapTrigger)) {
            return;
        }
        let flake = |x, y| Flake { x, y, speed: gen_range(-1.0, 1.0), ticks_left: ticks(FLAKE_SECONDS) };
        self.flakes.extend((0..FLAKES_FROM_ABOVE).map(|_| flake(view.0 + gen_range(ABOVE_SPAWN_X.0, ABOVE_SPAWN_X.1), view.1 - 1.0)));
        self.flakes.extend((0..FLAKES_FROM_THE_RIGHT).map(|_| flake(view.0 + RIGHT_SPAWN_X, view.1 + gen_range(RIGHT_SPAWN_Y.0, RIGHT_SPAWN_Y.1))));
    }

    /// SERVER_NAPICE_STATE as the world now shows it, and the shards' Step.
    fn step_ice(&mut self, world: &World, heard: &mut Heard) {
        let blocks: Vec<_> = world.ids_of(ObjectId::NapIceblock).collect();
        for (&block, seen) in blocks.iter().zip(&mut self.blocks_seen) {
            let instance = &world.instances[block];
            if instance.visible == *seen {
                continue;
            }
            *seen = instance.visible;
            if instance.visible {
                heard.sounds.push((sound::SND_ICE_SPAWN, instance.x, instance.y));
                continue;
            }
            heard.sounds.push((sound::SND_ICE_BREAK, instance.x, instance.y));
            for row in 0..2 {
                let (x, y) = (instance.x + SHARD_OFFSET_X, instance.y + row as f64 * SHARD_ROW);
                self.shards.push(Shard { x, y, xspd: 1.0 - row as f64, yspd: SHARD_THROW, dir: -1.0 });
            }
            for row in 0..2 {
                let (x, y) = (instance.x + SHARD_OFFSET_X, instance.y + row as f64 * SHARD_ROW);
                self.shards.push(Shard { x, y, xspd: 1.0 + row as f64, yspd: SHARD_THROW, dir: 1.0 });
            }
        }
        let shard_sprite = world.sprites.get(sprite::SPR_NAP_ICEBLOCK_PART);
        self.shards.retain_mut(|shard| {
            shard.yspd += SHARD_GRAVITY * step();
            shard.xspd += shard.dir * SHARD_WIND;
            shard.x += shard.xspd;
            shard.y += shard.yspd;
            let body = sprite_bbox(shard_sprite, shard.x, shard.y, 1.0, 1.0, 0.0);
            // instance_place finds one floor; an invisible one keeps the shard falling.
            let floor = world.ids_of(ObjectId::FloorParent).find(|&id| world.bbox(id).is_some_and(|bbox| bbox.overlaps(&body)));
            let landed = floor.is_some_and(|floor| world.instances[floor].visible);
            !landed && shard.y < world.room_height
        });
    }

    /// obj_nap_snowball Step, and SERVER_NAPBALL_STATE 2 when one is gone.
    fn step_snowballs(&mut self, world: &mut World, own: Option<&Player>, heard: &mut Heard) {
        let balls: Vec<_> = world.ids_of(ObjectId::NapSnowball).collect();
        for ((&ball, seen), timer) in balls.iter().zip(&mut self.balls_seen).zip(&mut self.roll_timers) {
            let instance = &mut world.instances[ball];
            let appeared = instance.visible && !*seen;
            let broke = !instance.visible && *seen;
            *seen = instance.visible;
            if broke {
                heard.sounds.push((sound::SND_SNOWBALL_BREAK, instance.x, instance.y));
                self.pieces.extend(pieces(instance.x, instance.y, -instance.image_xscale));
                continue;
            }
            if !instance.visible {
                continue;
            }
            if appeared {
                *timer = 0;
            }
            if instance.image_alpha < 1.0 {
                instance.image_alpha += SNOWBALL_FADE_IN * step();
            }
            let Some(own) = own else { continue };
            let (x, y, frame) = (instance.x, instance.y, instance.image_index);
            if frame >= FIRST_ROLLING_FRAME {
                let body = sprite_bbox(world.sprites.get(own.sprite_index), own.x, own.y, own.image_xscale, 1.0, 0.0);
                let distance = world.bbox(ball).map_or(f64::MAX, |bbox| bbox.distance(&body));
                if distance < SHAKE_REACH {
                    heard.shake = Some(1.0 - distance / SHAKE_REACH);
                }
            }
            *timer += 1;
            if *timer >= ticks(ROLL_SOUND_SECONDS) {
                heard.sounds.push((sound::SND_SNOWBALL_ROLL, x, y));
                *timer = 0;
            }
        }
    }

    pub fn draw_snow(&self, canvas: &mut Canvas) {
        for flake in &self.flakes {
            canvas.draw_sprite(sprite::SPR_NAP_SNOW, 0.0, flake.x, flake.y);
        }
    }

    pub fn draw_shards(&self, canvas: &mut Canvas) {
        for shard in &self.shards {
            canvas.draw_sprite(sprite::SPR_NAP_ICEBLOCK_PART, 0.0, shard.x, shard.y);
        }
    }

    pub fn draw_pieces(&self, canvas: &mut Canvas) {
        for piece in &self.pieces {
            canvas.draw_sprite_ext(sprite::SPR_NAP_SNOWBALL_PART, piece.frame, piece.x, piece.y, 1.0, 1.0, piece.angle, C_WHITE, piece.alpha);
        }
    }
}

/// SERVER_NAPBALL_STATE 2: two big pieces in a row of frames 0 to 3, then three to six
/// small ones (the loop draws its bound again each time round), all flying `dir`.
fn pieces(x: f64, y: f64, dir: f64) -> Vec<Piece> {
    let piece = |spread: f64, frame: i32, push: f64| Piece {
        x,
        y: y - PIECES_ABOVE + gen_range(-spread, spread) * PIECE_SPREAD,
        xspd: (push + gen_range(-2.0, 2.0)) * dir,
        yspd: -4.0 + gen_range(-2.0, 2.0),
        frame: frame as f64,
        angle: 0.0,
        alpha: 1.0,
        ticks_to_fade: crate::core::config::ticks(1.0 + gen_range(0.0, 0.5)),
    };
    let first = gen_range(0, 3);
    let mut pieces: Vec<Piece> = (first..first + 2).map(|frame| piece(1.5, frame, 7.0)).collect();
    let mut small = 0;
    while small < gen_range(3, 7) {
        pieces.push(piece(1.0, gen_range(4, 10), 4.0));
        small += 1;
    }
    pieces
}

impl Piece {
    /// Step and Alarm of obj_nap_snowball_part. False once faded away.
    fn step(&mut self) -> bool {
        self.ticks_to_fade -= 1;
        self.yspd += PIECE_GRAVITY * step();
        // sign(0) is 0 in GameMaker.
        if self.xspd != 0.0 {
            self.xspd -= self.xspd.signum() * PIECE_DRAG * step();
        }
        self.x += self.xspd * step();
        self.y += self.yspd * step();
        self.angle += step();
        if self.ticks_to_fade <= 0 {
            self.alpha -= PIECE_FADE * step();
            return self.alpha > 0.0;
        }
        true
    }
}
