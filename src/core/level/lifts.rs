//! Priceless Freedom's lifts (the server's pf_lift): the first to touch one rides it up
//! to its top and is let off with a hop; the lift fades away, waits and comes back.

use crate::core::config::{step, ticks, PricelessFreedomRules};
use crate::core::objects::ids::ObjectId;
use crate::core::player::Player;
use crate::core::world::{InstanceId, ObjectVars, World};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Lifts {
    /// By the map's lifts in instance order.
    lifts: Vec<Lift>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct Lift {
    /// How far above its start the lift is.
    raised: f64,
    speed: f64,
    rider: Option<usize>,
    /// Ticks until it is back at its start after a ride.
    rest: i32,
    /// image_alpha: a lift can be taken only once it has faded in completely.
    alpha: f64,
    fading_out: bool,
}

/// What clients are told of one lift.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LiftView {
    pub raised: f32,
    pub alpha: f32,
    pub carrying: bool,
}

impl Lifts {
    pub(super) fn tick(&mut self, rules: &PricelessFreedomRules, world: &World, players: &mut [Player]) {
        let ids = lift_ids(world);
        self.lifts.resize(ids.len(), Lift::default());
        for (lift, &id) in self.lifts.iter_mut().zip(&ids) {
            // obj_pf_lift Step: each client faded its copy, the server's timer does not wait for it.
            if lift.fading_out {
                lift.alpha = (lift.alpha - rules.lift_fade_per_step * step()).max(0.0);
            } else if lift.alpha < 1.0 {
                lift.alpha += rules.lift_fade_per_step * step();
            }
            let ObjectVars::Lift { start_y, top_y, .. } = world.instances[id].vars else { continue };
            let Some(rider) = lift.rider else {
                if lift.rest > 0 {
                    lift.rest -= 1;
                    if lift.rest <= 0 {
                        lift.raised = 0.0;
                        lift.fading_out = false;
                    }
                }
                continue;
            };
            if start_y - lift.raised > top_y {
                if lift.speed < rules.lift_max_speed {
                    lift.speed += rules.lift_acceleration * step();
                }
                lift.raised += lift.speed * step();
                continue;
            }
            // SERVER_PFLIFT_STATE 2: the rider is let off with a hop.
            if let Some(player) = players.get_mut(rider) {
                if !world.game_ends {
                    player.controls_enabled = true;
                }
                player.x = world.instances[id].x;
                player.y = start_y - lift.raised;
                player.yspd = rules.lift_leave_speed;
                player.riding_lift = None;
            }
            lift.rider = None;
            lift.rest = ticks(rules.lift_rest_seconds);
            lift.fading_out = true;
        }
    }

    /// CLIENT_PFLIT_ACTIVATE: a free lift at its start takes the player.
    pub(super) fn board(&mut self, world: &World, lift_id: InstanceId, index: usize, player: &mut Player) {
        let ids = lift_ids(world);
        let Some(position) = ids.iter().position(|&id| id == lift_id) else { return };
        let Some(lift) = self.lifts.get_mut(position) else { return };
        if lift.rider.is_some() || lift.rest > 0 {
            return;
        }
        let ObjectVars::Lift { start_y, .. } = world.instances[lift_id].vars else { return };
        *lift = Lift { raised: 0.0, speed: 0.0, rider: Some(index), rest: 0, alpha: lift.alpha, fading_out: false };
        player.controls_enabled = false;
        player.x = world.instances[lift_id].x;
        player.y = start_y;
        player.riding_lift = Some(lift_id);
    }

    pub(super) fn view(&self) -> Vec<LiftView> {
        self.lifts.iter().map(|lift| LiftView { raised: lift.raised as f32, alpha: lift.alpha as f32, carrying: lift.rider.is_some() }).collect()
    }
}

fn lift_ids(world: &World) -> Vec<InstanceId> {
    world.ids_of(ObjectId::PfLift).collect()
}

/// SERVER_PFLIFT_STATE on clients: where each lift is and how clearly it shows.
pub(super) fn apply(world: &mut World, lifts: &[LiftView]) {
    for (id, lift) in lift_ids(world).into_iter().zip(lifts) {
        let instance = &mut world.instances[id];
        let ObjectVars::Lift { start_y, top_y, .. } = instance.vars else { continue };
        instance.vars = ObjectVars::Lift { start_y, top_y, carrying: lift.carrying };
        instance.y = start_y - lift.raised as f64;
        instance.image_alpha = lift.alpha as f64;
    }
}
