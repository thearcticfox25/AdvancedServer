//! Not Perfect's stages (the server's npctrl): the level is four copies of one stage,
//! and now and then, after a warning, everyone is moved to the next copy.

use crate::core::config::{ticks, GameplayConfig, NotPerfectRules};
use crate::core::events::SimEvent;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Character, Player};
use crate::core::world::{ObjectVars, World};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NotPerfect {
    timer: i32,
    warning: bool,
    /// Counts every switch; the stage is its last two bits.
    switches: u8,
    chased: bool,
    switched: bool,
}

/// What clients are told: the stage, whether the warning shows, whether it just switched.
pub type NotPerfectView = (u8, bool, bool);

impl NotPerfect {
    pub(super) fn tick(&mut self, cfg: &GameplayConfig, world: &World, players: &mut [Player], events: &mut Vec<SimEvent>) {
        let rules = &cfg.levels.not_perfect;
        self.switched = false;
        let seconds_left = world.timer_ticks / crate::core::config::ticks_per_second() as i64;
        let chase = (seconds_left as f64) < rules.chase_seconds_left;
        // The chase brings a switch at once.
        if seconds_left as f64 <= rules.chase_seconds_left && !self.chased {
            self.timer = ticks(rules.switch_after_seconds);
            self.warning = true;
            self.chased = true;
        }
        if !self.warning {
            let after = if chase { rules.warning_after_seconds_chase } else { rules.warning_after_seconds };
            if self.timer >= ticks(after) {
                self.warning = true;
                self.timer = 0;
            }
        } else {
            let after = if chase { rules.switch_after_seconds_chase } else { rules.switch_after_seconds };
            if self.timer >= ticks(after) {
                let from = stage_place(rules, self.switches % 4);
                self.switches = self.switches.wrapping_add(1);
                let to = stage_place(rules, self.switches % 4);
                let (dx, dy) = (to.0 - from.0, to.1 - from.1);
                for player in players.iter_mut().filter(|player| !player.removed) {
                    move_to_next_stage(cfg, world, player, dx, dy);
                }
                events.push(SimEvent::StageSwitched { dx, dy });
                self.switched = true;
                self.warning = false;
                self.timer = 0;
            }
        }
        self.timer += 1;
    }

    pub(super) fn view(&self) -> NotPerfectView {
        (self.switches % 4, self.warning, self.switched)
    }
}

/// Where a stage's copy is from stage 0's.
fn stage_place(rules: &NotPerfectRules, stage: u8) -> (f64, f64) {
    let [x, y] = rules.stage_offset;
    (if stage & 1 != 0 { x } else { 0.0 }, if stage & 2 != 0 { y } else { 0.0 })
}

/// SERVER_NPCONTROLLER_STATE 1 on a client: the same place on the next stage, a
/// stuck Knuckles let go, and out of the walls that are not in the same place there.
fn move_to_next_stage(cfg: &GameplayConfig, world: &World, player: &mut Player, dx: f64, dy: f64) {
    if player.character == Character::Knux && player.is_stuck {
        player.is_stuck = false;
        player.is_gliding = false;
        player.glide_timer = ticks(cfg.knuckles.glide_recharge_seconds);
    }
    player.x += dx;
    player.y += dy;
    push_out_of_walls(world, player);
}

/// obj_left_pusher and obj_right_pusher while active: a body inside one is
/// put beside it and stopped.
pub fn push_out_of_walls(world: &World, player: &mut Player) {
    let body = crate::core::collision::Bbox { left: player.x - 6.0, top: player.y - 20.0, right: player.x + 6.0, bottom: player.y + 18.0 };
    for (object, left) in [(ObjectId::LeftPusher, true), (ObjectId::RightPusher, false)] {
        for wall in world.ids_of(object) {
            let Some(bbox) = world.bbox(wall) else { continue };
            // collision_rectangle takes both corners in.
            if body.left < bbox.right && body.right >= bbox.left && body.top < bbox.bottom && body.bottom >= bbox.top {
                // bbox_right is the last pixel inside.
                player.x = if left { bbox.left - 20.0 } else { bbox.right - 1.0 + 14.0 };
                player.xspd = 0.0;
                player.yspd = 0.0;
            }
        }
    }
}

pub(super) fn apply(world: &mut World, (stage, warning, switched): NotPerfectView) {
    let controllers: Vec<_> = world.ids_of(ObjectId::NpController).collect();
    for controller in controllers {
        world.instances[controller].vars = ObjectVars::NotPerfectController { stage, warning, switched };
    }
}
