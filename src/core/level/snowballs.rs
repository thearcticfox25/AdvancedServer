//! Nasty Paradise's snowballs (the server's nap_snowball): now and then each one rolls
//! down its path of waypoints, speeding up, and breaks at the path's last leg.

use crate::core::config::{step, ticks, NastyParadiseRules};
use crate::core::objects::ids::ObjectId;
use crate::core::world::{InstanceId, ObjectVars, World, SNOWBALL_UNDER_WAYPOINT};
use serde::{Deserialize, Serialize};

/// spr_nap_snowball: frames up to 16 start rolling, the ones after loop.
const LOOP_END_FRAME: f64 = 32.0;
const LOOP_START_FRAME: f64 = 16.0;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Snowballs {
    /// Every snowball's timer; they all start at once.
    timer: i32,
    /// By path nid.
    balls: Vec<Snowball>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct Snowball {
    rolling: bool,
    /// The waypoint it rolls from, and how far to the next one (0..1).
    leg: usize,
    progress: f64,
    frame: f64,
    speed: f64,
}

/// What clients are told of a rolling snowball (SERVER_NAPBALL_STATE 1).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SnowballView {
    pub leg: u8,
    pub progress: f32,
    pub frame: u8,
}

impl Snowballs {
    pub(super) fn tick(&mut self, rules: &NastyParadiseRules, world: &World) {
        let paths = paths(world);
        self.balls.resize(paths.len(), Snowball::default());
        if self.timer < ticks(rules.snowball_every_seconds) {
            self.timer += 1;
        } else {
            self.timer = 0;
            for ball in self.balls.iter_mut().filter(|ball| !ball.rolling) {
                *ball = Snowball { rolling: true, ..Snowball::default() };
            }
        }
        for (ball, path) in self.balls.iter_mut().zip(&paths) {
            if !ball.rolling {
                continue;
            }
            if ball.speed > 1.0 {
                let (roll_speed, roll_frames) = match path.get(ball.leg).map(|&id| &world.instances[id].vars) {
                    Some(&ObjectVars::SnowballWaypoint { roll_speed, roll_frames, .. }) => {
                        (roll_speed.unwrap_or(rules.snowball_roll_speed), roll_frames.unwrap_or(rules.snowball_roll_frames))
                    }
                    _ => (rules.snowball_roll_speed, rules.snowball_roll_frames),
                };
                ball.frame += roll_frames * step();
                ball.progress += roll_speed * step();
                if ball.frame >= LOOP_END_FRAME {
                    ball.frame = LOOP_START_FRAME;
                }
            } else {
                ball.speed += rules.snowball_start_acceleration * step();
                ball.frame += ball.speed * rules.snowball_start_frames * step();
                ball.progress += ball.speed * rules.snowball_start_progress * step();
            }
            if ball.progress > 1.0 {
                ball.progress = 0.0;
                ball.leg += 1;
                // SERVER_NAPBALL_STATE 2: it breaks on the last leg.
                if ball.leg >= path.len().saturating_sub(1) {
                    *ball = Snowball::default();
                }
            }
        }
    }

    pub(super) fn view(&self) -> Vec<Option<SnowballView>> {
        self.balls
            .iter()
            .map(|ball| ball.rolling.then(|| SnowballView { leg: ball.leg as u8, progress: ball.progress as f32, frame: ball.frame as u8 }))
            .collect()
    }
}

/// Each path's waypoints in order, by path nid.
fn paths(world: &World) -> Vec<Vec<InstanceId>> {
    let mut paths: Vec<Vec<(u8, InstanceId)>> = Vec::new();
    for id in world.ids_of(ObjectId::NapSnowballWaypoint) {
        if let ObjectVars::SnowballWaypoint { nid, wid, .. } = world.instances[id].vars {
            if paths.len() <= nid as usize {
                paths.resize(nid as usize + 1, Vec::new());
            }
            paths[nid as usize].push((wid, id));
        }
    }
    paths
        .into_iter()
        .map(|mut path| {
            path.sort_by_key(|&(wid, _)| wid);
            path.into_iter().map(|(_, id)| id).collect()
        })
        .collect()
}

/// SERVER_NAPBALL_STATE on clients: a rolling snowball stands between the waypoints of
/// its leg, facing the way its path goes; the others are hidden.
pub(super) fn apply(world: &mut World, snowballs: &[Option<SnowballView>]) {
    let paths = paths(world);
    let balls: Vec<_> = world.ids_of(ObjectId::NapSnowball).collect();
    for ball in balls {
        let ObjectVars::Snowball { nid } = world.instances[ball].vars else { continue };
        let path = paths.get(nid as usize).map_or(&[][..], Vec::as_slice);
        let place = |index: usize| path.get(index).map(|&id| (world.instances[id].x, world.instances[id].y));
        let view = snowballs.get(nid as usize).copied().flatten();
        let (Some(view), Some(start), Some(second)) = (view, place(0), place(1)) else {
            world.instances[ball].visible = false;
            continue;
        };
        let (Some(first), Some(next)) = (place(view.leg as usize), place(view.leg as usize + 1)) else { continue };
        let progress = view.progress as f64;
        let instance = &mut world.instances[ball];
        if !instance.visible {
            // obj_nap_snowball Create: it fades in.
            instance.visible = true;
            instance.image_alpha = 0.0;
            instance.image_xscale = if second.0 < start.0 { -1.0 } else { 1.0 };
        }
        instance.x = first.0 + (next.0 - first.0) * progress + SNOWBALL_UNDER_WAYPOINT.0;
        instance.y = first.1 + (next.1 - first.1) * progress + SNOWBALL_UNDER_WAYPOINT.1;
        instance.image_index = view.frame as f64;
    }
}
