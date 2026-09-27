//! Weed Zone as clients show it (obj_weed and friends): the lanterns' flicker and
//! light, the weed's arms creeping over a survivor who stays in the dark too long, the
//! ghosts drifting past, the fog outside the woods and the self-insert that appears with
//! the big ring. The server lights the lanterns (sim level/lanterns.rs).

use super::drawing::{lantern_end, LANTERN_HEIGHT};
use crate::client::canvas::{Canvas, VIEW_HEIGHT, VIEW_WIDTH};
use crate::client::room::{LayerContent, Room};
use crate::core::resources::names::{sound, sprite};
use crate::core::resources::SoundId;
use crate::core::collision::sprite_bbox;
use crate::core::config::ticks_per_second;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Character, Player};
use crate::core::world::{InstanceId, ObjectVars, World, C_WHITE};
use macroquad::rand::gen_range;
use crate::core::config::step;

/// The layers the ghosts fly on, and the fog's.
pub const GHOST1_DEPTH: i32 = 592;
pub const GHOST2_DEPTH: i32 = 292;
const FOG_LAYER: &str = "Foguis";
const FOG_SCROLL: f64 = 2.0;
const FOG_FADE: f64 = 0.016;
/// obj_weed_lantern: its frames go by this a step, looping from 1.
const LANTERN_FRAME_STEP: f64 = 0.16;
/// image_angle = ((sin(current_time / 400) + 1) / 2) * 4
const CHAIN_SWING_MILLISECONDS: f64 = 400.0;
const CHAIN_SWING: f64 = 4.0;
/// A lit lantern keeps the weed away within this.
const LANTERN_REACH: f64 = 80.0;
/// obj_weed_light: fades by this a step.
const LIGHT_FADE: f64 = 2.0 / 60.0;
/// obj_weed alarm[0]: the weed comes 21 to 22 seconds after the last light.
const WEED_AFTER_SECONDS: f64 = 20.0;
const WEED_AFTER_EXTRA: (f64, f64) = (1.0, 2.0);
/// Draw GUI: the arms grow by this a step, then the vignette over 30 seconds; both go back faster.
const ARMS_FRAME_STEP: f64 = 0.25;
const VIGNETTE_IN: f64 = 0.016 / 30.0;
const VIGNETTE_OUT: f64 = 0.016;
const ARMS_LEFT: f64 = -10.0;
const ARMS_SWAY: f64 = 8.0;
const ARMS_SWAY_MILLISECONDS: f64 = 300.0;
const ARMS_SHAKE: f64 = 2.0;
/// obj_weed alarm[1]: a ghost every 5 to 7 seconds (one time in five none).
const GHOST_EVERY_SECONDS: (i32, i32) = (5, 7);
const GHOST_TOP_RANGE: i32 = 200;
const GHOST_SPEED: (f64, f64) = (0.5, 1.0);
const GHOST_OUTSIDE: f64 = 64.0;
const GHOST_BOB: f64 = 10.0;
const GHOST_BOB_MILLISECONDS: f64 = 300.0;
const GHOST_PHASE_RANGE: f64 = 2000.0;
/// obj_weed_selfinsert: before the big ring only a smaller sprite, here.
const SELF_INSERT_BEFORE: (f64, f64) = (3968.0, 974.0);

pub struct WeedZone {
    lit_seen: Option<u8>,
    /// obj_weed_light of each lantern: where it shines and how much.
    lights: Vec<(InstanceId, f64, f64, f64)>,
    fog: f64,
    fog_scroll: f64,
    weed: bool,
    weed_alarm: i32,
    arms_frame: f64,
    vignette: f64,
    ghost_alarm: i32,
    ghosts: Vec<Ghost>,
    self_insert: Option<(InstanceId, f64, f64)>,
}

struct Ghost {
    second_kind: bool,
    x: f64,
    y: f64,
    top: f64,
    speed: f64,
    phase: f64,
}

impl WeedZone {
    /// Create of obj_weed, the lanterns and the ghosts the map places.
    pub fn open(world: &mut World, is_exe: bool) -> WeedZone {
        let lanterns: Vec<InstanceId> = world.ids_of(ObjectId::WeedLantern).collect();
        let lights = lanterns
            .iter()
            .map(|&id| {
                let instance = &world.instances[id];
                let height = instance.sprite_index.map_or(0.0, |sprite| world.sprites.get(sprite).height as f64);
                let below = if instance.object == ObjectId::WeedLantern { height / 2.0 } else { height + LANTERN_HEIGHT / 2.0 };
                (id, instance.x, instance.y + below, 0.0)
            })
            .collect();
        let placed_ghosts: Vec<InstanceId> = world.ids_of(ObjectId::WeedGhost).chain(world.ids_of(ObjectId::WeedGhost2)).collect();
        let ghosts = placed_ghosts
            .into_iter()
            .map(|id| {
                world.instances[id].visible = false;
                let top = world.instances[id].y;
                Ghost { second_kind: world.instances[id].object == ObjectId::WeedGhost2, x: 0.0, y: top, top, speed: 1.0, phase: gen_range(0.0, GHOST_PHASE_RANGE) }
            })
            .collect();
        let self_insert = world.ids_of(ObjectId::WeedSelfinsert).next().map(|id| (id, world.instances[id].x, world.instances[id].y));
        WeedZone {
            lit_seen: None,
            lights,
            fog: 0.0,
            fog_scroll: 0.0,
            weed: false,
            weed_alarm: if is_exe { -1 } else { weed_delay() },
            arms_frame: -1.0,
            vignette: 0.0,
            ghost_alarm: ghost_delay(),
            ghosts,
            self_insert,
        }
    }

    /// Returns the sounds this player hears, and where a lantern that just lit hangs.
    pub fn step(&mut self, room: &mut Room, world: &mut World, own: Option<&Player>, view: (f64, f64), big_ring_out: bool, current_time_ms: f64) -> (Vec<SoundId>, Option<(f64, f64)>) {
        let mut heard = Vec::new();
        let mut just_lit = None;
        // SERVER_WDLATERN_ACTIVATE
        let lit = world.ids_of(ObjectId::WeedLantern).find_map(|id| match world.instances[id].vars {
            ObjectVars::Lantern { nid, lit: true } => Some(nid),
            _ => None,
        });
        if lit.is_some() && lit != self.lit_seen {
            heard.push(sound::SND_WDLAMP);
            just_lit = lit;
        }
        self.lit_seen = lit;

        // obj_weed alarm[0]: the weed reaches a survivor who is still up.
        let can_be_weeded = own.is_some_and(|own| own.character != Character::Exe && own.hp > 0 && own.revival_times < 2);
        if self.weed_alarm > 0 {
            self.weed_alarm -= 1;
            if self.weed_alarm == 0 && can_be_weeded {
                heard.push(sound::SND_WDARMS);
                self.weed = true;
            }
        }
        if self.ghost_alarm > 0 {
            self.ghost_alarm -= 1;
            if self.ghost_alarm == 0 {
                self.spawn_ghost();
                self.ghost_alarm = ghost_delay();
            }
        }

        // obj_weed Step
        let view_middle = (view.0 + VIEW_WIDTH / 2.0, view.1 + VIEW_HEIGHT / 2.0);
        if world.position_meeting(view_middle.0, view_middle.1, ObjectId::WeedZone) {
            if self.fog > 0.0 {
                self.fog -= FOG_FADE * step();
            }
        } else if self.fog < 1.0 {
            self.fog += FOG_FADE * step();
        }
        if own.is_none_or(|own| own.is_dead || own.revival_times >= 2) {
            self.weed = false;
        }

        // The lanterns' Step.
        let lanterns: Vec<InstanceId> = world.ids_of(ObjectId::WeedLantern).collect();
        for &lantern in &lanterns {
            let active = matches!(world.instances[lantern].vars, ObjectVars::Lantern { lit: true, .. });
            let hanging = world.instances[lantern].object != ObjectId::WeedLantern;
            let (near_x, near_y) = if hanging {
                let (x, y) = lantern_end(world, lantern);
                (x, y + LANTERN_HEIGHT / 2.0)
            } else {
                (world.instances[lantern].x, world.instances[lantern].y)
            };
            let lantern_frames = world.sprites.get(sprite::SPR_WEED_LATERN).frame_count as f64;
            let instance = &mut world.instances[lantern];
            if active {
                instance.image_index += LANTERN_FRAME_STEP * step();
                if instance.image_index >= lantern_frames - 1.0 {
                    instance.image_index = 1.0;
                }
            } else {
                instance.image_index = 0.0;
            }
            if hanging {
                instance.image_angle = ((current_time_ms / CHAIN_SWING_MILLISECONDS).sin() + 1.0) / 2.0 * CHAIN_SWING;
            }
            let Some(own) = own.filter(|_| active) else { continue };
            let distance = if hanging {
                (near_x - own.x).hypot(near_y - own.y)
            } else {
                let body = sprite_bbox(world.sprites.get(own.sprite_index), own.x, own.y, own.image_xscale, 1.0, 0.0);
                world.bbox(lantern).map_or(f64::MAX, |bbox| bbox.distance(&body))
            };
            if distance < LANTERN_REACH {
                self.weed = false;
                self.weed_alarm = weed_delay();
            }
        }
        // SERVER_WDLATERN_ACTIVATE makes the survivors' arrow to the lantern that lit.
        let just_lit = just_lit.and_then(|nid| {
            let lantern = lanterns.iter().copied().find(|&id| matches!(world.instances[id].vars, ObjectVars::Lantern { nid: lit, .. } if lit == nid))?;
            let hanging = world.instances[lantern].object != ObjectId::WeedLantern;
            Some(if hanging { lantern_end(world, lantern) } else { (world.instances[lantern].x, world.instances[lantern].y) })
        });
        for light in &mut self.lights {
            let shows = matches!(world.instances[light.0].vars, ObjectVars::Lantern { lit: true, .. });
            light.3 = if shows { (light.3 + LIGHT_FADE).min(1.0) } else { (light.3 - LIGHT_FADE).max(0.0) };
        }

        // obj_weed Draw Begin: the fog follows the view and drifts.
        if let Some(LayerContent::Background(fog)) = room.layer_mut(FOG_LAYER).map(|layer| &mut layer.content) {
            (fog.parallax_x, fog.parallax_y, fog.x, fog.y, fog.hspeed) = (0.0, 0.0, self.fog_scroll, 0.0, 0.0);
            fog.alpha = self.fog;
        }
        self.fog_scroll += FOG_SCROLL * step();

        // obj_weed_ghost Draw Begin
        self.ghosts.retain_mut(|ghost| {
            ghost.x += ghost.speed * step();
            ghost.y = ghost.top + (current_time_ms / GHOST_BOB_MILLISECONDS + ghost.phase).sin() * GHOST_BOB;
            let width = world.sprites.get(ghost.sprite()).width as f64;
            !(ghost.speed > 0.0 && ghost.x >= VIEW_WIDTH + width + GHOST_OUTSIDE || ghost.speed < 0.0 && ghost.x <= -2.0 * GHOST_OUTSIDE)
        });

        // obj_weed Draw GUI: the arms and the dark grow while the weed holds on.
        if self.weed {
            let last_frame = world.sprites.get(sprite::SPR_WEED_ARMS).frame_count as f64 - 1.0;
            if self.arms_frame < last_frame {
                self.arms_frame += ARMS_FRAME_STEP * step();
            } else {
                self.arms_frame = last_frame;
                self.vignette = (self.vignette + VIGNETTE_IN).min(1.0);
            }
        } else {
            if self.arms_frame > -1.0 {
                self.arms_frame -= ARMS_FRAME_STEP * step();
            }
            self.vignette = (self.vignette - VIGNETTE_OUT).max(0.0);
        }

        // obj_weed_selfinsert Draw
        if let Some((id, x, y)) = self.self_insert {
            let instance = &mut world.instances[id];
            (instance.sprite_index, instance.x, instance.y) = if big_ring_out {
                (Some(sprite::SPR_SELFINSERT), x, y)
            } else {
                (Some(sprite::SPR_SELFINSERT2), SELF_INSERT_BEFORE.0, SELF_INSERT_BEFORE.1)
            };
        }
        (heard, just_lit)
    }

    fn spawn_ghost(&mut self) {
        let kind = gen_range(0, 5);
        if kind == 4 {
            return;
        }
        let top = gen_range(0, GHOST_TOP_RANGE + 1) as f64;
        let speed = gen_range(GHOST_SPEED.0, GHOST_SPEED.1);
        let (speed, x) = if kind < 2 { (speed, -GHOST_OUTSIDE) } else { (-speed, VIEW_WIDTH + GHOST_OUTSIDE) };
        self.ghosts.push(Ghost { second_kind: kind % 2 == 0, x, y: top, top, speed, phase: gen_range(0.0, GHOST_PHASE_RANGE) });
    }

    pub fn draw_ghosts(&self, canvas: &mut Canvas, view: (f64, f64), second_kind: bool, current_time_ms: f64) {
        for ghost in self.ghosts.iter().filter(|ghost| ghost.second_kind == second_kind) {
            let sprite = ghost.sprite();
            let frame = current_time_ms * canvas.sprites.get(sprite).fps as f64 / 1000.0;
            // obj_weed_ghost faces away from where it flies, obj_weed_ghost2 towards it.
            let facing = if second_kind { ghost.speed.signum() } else { -ghost.speed.signum() };
            canvas.draw_sprite_ext(sprite, frame, view.0 + ghost.x, view.1 + ghost.y, facing, 1.0, 0.0, C_WHITE, 1.0);
        }
    }

    /// Draw GUI of obj_weed_light, then of obj_weed.
    pub fn draw_gui(&self, canvas: &mut Canvas, view: (f64, f64), is_exe: bool, current_time_ms: f64) {
        for &(_, x, y, alpha) in &self.lights {
            canvas.draw_sprite_ext(sprite::SPR_WEED_LIGHT, 0.0, x - view.0, y - view.1, 1.0, 1.0, 0.0, C_WHITE, alpha);
        }
        if is_exe {
            return;
        }
        if self.arms_frame >= 0.0 {
            let angle = current_time_ms / ARMS_SWAY_MILLISECONDS;
            let x = ARMS_LEFT + angle.sin() * ARMS_SWAY + gen_range(-ARMS_SHAKE, ARMS_SHAKE);
            let y = ARMS_LEFT + angle.cos() * ARMS_SWAY + gen_range(-ARMS_SHAKE, ARMS_SHAKE);
            canvas.draw_sprite(sprite::SPR_WEED_ARMS, self.arms_frame, x, y);
        }
        if self.vignette > 0.0 {
            canvas.draw_sprite_ext(sprite::SPR_WEED_VIGNETTE, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, C_WHITE, self.vignette);
        }
    }
}

impl Ghost {
    fn sprite(&self) -> crate::core::resources::SpriteId {
        if self.second_kind { sprite::SPR_WEED_GHOSTS2 } else { sprite::SPR_WEED_GHOSTS }
    }
}

fn weed_delay() -> i32 {
    ((WEED_AFTER_SECONDS + gen_range(WEED_AFTER_EXTRA.0, WEED_AFTER_EXTRA.1)) * ticks_per_second()) as i32
}

fn ghost_delay() -> i32 {
    gen_range(GHOST_EVERY_SECONDS.0, GHOST_EVERY_SECONDS.1 + 1) * ticks_per_second() as i32
}
