//! Dark Tower's objects the server decides: the that fall on whoever walks under them (dt_stalactits) and the Tails Doll
//! that waits, notices a survivor, chases and scares them (dt_tails_doll).

use super::Random;
use crate::core::config::{step, ticks, DarkTowerRules, GameplayConfig};
use crate::core::events::SimEvent;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{gm_sign, Character, Player};
use crate::core::world::{ObjectVars, World};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DarkTower {
    /// By the map's stalactites in instance order.
    stalactites: Vec<Stalactite>,
    doll: Doll,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Stalactite {
    shown: bool,
    falling: bool,
    timer: i32,
    fallen: f64,
    speed: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Doll {
    x: f64,
    y: f64,
    state: DollState,
    /// The chased survivor's place among the players.
    target: Option<usize>,
    timer: i32,
    speed: [f64; 2],
}

/// The frame of spr_darktower_tailsol is the state's number.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
enum DollState {
    Waiting = 0,
    Ready = 1,
    Chasing = 2,
    Moving = 3,
}

/// What clients are told of Dark Tower.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DarkTowerView {
    /// How far each stalactite has fallen, None while it is gone.
    pub stalactites: Vec<Option<StalactiteView>>,
    pub doll: (f32, f32, u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StalactiteView {
    pub fallen: f32,
    pub falling: bool,
}

impl DarkTower {
    pub(super) fn new(world: &World) -> DarkTower {
        let stalactite = Stalactite { shown: true, falling: false, timer: 0, fallen: 0.0, speed: 0.0 };
        // Its first spot is chosen away from the players on the first tick.
        let doll = Doll { x: 0.0, y: 0.0, state: DollState::Moving, target: None, timer: 0, speed: [0.0; 2] };
        DarkTower { stalactites: vec![stalactite; world.ids_of(ObjectId::DarktowerStalactite).count()], doll }
    }

    pub(super) fn tick(&mut self, cfg: &GameplayConfig, world: &World, players: &mut [Player], random: &mut Random, events: &mut Vec<SimEvent>) {
        let rules = &cfg.levels.dark_tower;
        let ids: Vec<_> = world.ids_of(ObjectId::DarktowerStalactite).collect();
        for (stalactite, &id) in self.stalactites.iter_mut().zip(&ids) {
            let ObjectVars::Stalactite { start_y, .. } = world.instances[id].vars else { continue };
            stalactite.tick(rules, (world.instances[id].x, start_y), world, players, random);
        }
        self.doll.tick(cfg, players, random, events);
    }

    pub(super) fn view(&self) -> DarkTowerView {
        DarkTowerView {
            stalactites: self.stalactites.iter().map(|stalactite| stalactite.shown.then_some(StalactiteView { fallen: stalactite.fallen as f32, falling: stalactite.falling })).collect(),
            doll: (self.doll.x as f32, self.doll.y as f32, self.doll.state as u8),
        }
    }
}

impl Stalactite {
    fn tick(&mut self, rules: &DarkTowerRules, (x, start_y): (f64, f64), world: &World, players: &[Player], random: &mut Random) {
        if self.falling {
            self.speed += rules.stalactite_acceleration * step();
            self.fallen += self.speed * step();
            // obj_darktower_stalactite End Step: it breaks on the floor (CLIENT_DTASS_ACTIVATE).
            let (left, top) = (x + STALACTITE_TIP.0, start_y + self.fallen + STALACTITE_TIP.1);
            if world.collision_rectangle(left, top, left + STALACTITE_TIP.2, top + STALACTITE_TIP.3, ObjectId::FloorParent).is_some() {
                self.shown = false;
                self.falling = false;
                self.timer = ticks(rules.stalactite_back_seconds + random.below(rules.stalactite_back_extra_seconds) as f64);
                self.fallen = 0.0;
                self.speed = 0.0;
            }
            return;
        }
        let armed_after = ticks(rules.stalactite_armed_after_seconds);
        if !self.shown {
            if self.timer > armed_after {
                self.timer -= 1;
            }
            if self.timer <= armed_after {
                self.shown = true;
            }
        } else if self.timer > 0 {
            self.timer -= 1;
        }
        if self.timer > 0 {
            return;
        }
        let under = players.iter().filter(|player| !player.removed && !player.is_dead).any(|player| {
            let below = player.y - start_y;
            below > 0.0 && below <= rules.stalactite_notice_below && player.x >= x && player.x <= x + rules.stalactite_notice_width
        });
        if under {
            self.speed = 0.0;
            self.falling = true;
        }
    }
}

/// The floor check under a stalactite: left and top from its corner, width and height.
const STALACTITE_TIP: (f64, f64, f64, f64) = (19.0, 18.0, 40.0, 2.0);

impl Doll {
    /// A random spot far from everyone, or any spot when none is.
    fn find_spot(&mut self, rules: &DarkTowerRules, players: &[Player], random: &mut Random) {
        let far = |spot: &&[f64; 2]| players.iter().filter(|player| !player.removed).all(|player| (spot[0] - player.x).hypot(spot[1] - player.y) >= rules.doll_spot_away);
        let valid: Vec<&[f64; 2]> = rules.doll_spots.iter().filter(far).collect();
        let spot = if valid.is_empty() {
            rules.doll_spots.get(random.below(rules.doll_spots.len() as u32) as usize)
        } else {
            Some(valid[random.below(valid.len() as u32) as usize])
        };
        if let Some(&[x, y]) = spot {
            (self.x, self.y) = (x, y);
        }
    }

    fn huntable(player: &Player) -> bool {
        !player.removed && player.character != Character::Exe && !player.is_dead && !player.is_demonized()
    }

    /// The first survivor close enough becomes the target.
    fn find_target(&mut self, rules: &DarkTowerRules, players: &[Player]) -> bool {
        let near = players.iter().position(|player| Doll::huntable(player) && (self.x - player.x).hypot(self.y - player.y) < rules.doll_notice);
        if near.is_some() {
            self.target = near;
        }
        near.is_some()
    }

    fn tick(&mut self, cfg: &GameplayConfig, players: &mut [Player], random: &mut Random, events: &mut Vec<SimEvent>) {
        let rules = &cfg.levels.dark_tower;
        match self.state {
            DollState::Waiting => {
                if self.find_target(rules, players) {
                    self.timer = ticks(rules.doll_ready_seconds + random.below(2) as f64 * rules.doll_ready_extra_seconds);
                    self.state = DollState::Ready;
                    events.extend(self.target.map(|target| SimEvent::TailsDollNoticed { target }));
                }
            }
            DollState::Ready => {
                self.timer -= 1;
                if self.timer <= 0 {
                    events.extend(self.target.map(|target| SimEvent::TailsDollChases { target }));
                    self.state = DollState::Chasing;
                }
            }
            DollState::Chasing => {
                self.find_target(rules, players);
                let Some(target) = self.target.filter(|&target| players.get(target).is_some_and(Doll::huntable)) else {
                    self.state = DollState::Moving;
                    return;
                };
                let player = &mut players[target];
                // The server measures in whole pixels.
                let distance = [(player.x - self.x).trunc(), (player.y - self.y).trunc()];
                for axis in 0..2 {
                    if distance[axis].abs() >= rules.doll_dead_zone[axis] {
                        self.speed[axis] = (self.speed[axis] + gm_sign(distance[axis]) * rules.doll_acceleration[axis] * step()).clamp(-rules.doll_max_speed, rules.doll_max_speed);
                    }
                }
                self.x += self.speed[0] * step();
                self.y += self.speed[1] * step();
                if (self.x - player.x).hypot(self.y - player.y) < rules.doll_catch {
                    // SERVER_DTTAILSDOLL_STATE 1 on the target's client.
                    if player.hp > 0 {
                        events.push(SimEvent::TailsDollCaught { victim: target, x: player.x, y: player.y });
                        player.slow_down(cfg, rules.doll_slow_percent, rules.doll_slow_seconds);
                    }
                    self.state = DollState::Moving;
                }
            }
            DollState::Moving => {
                self.find_spot(rules, players, random);
                self.state = DollState::Waiting;
            }
        }
    }
}

/// Sets what the server's packets set on clients: the stalactites' fall, and the doll's place and frame (facing the way it moved).
pub(super) fn apply(world: &mut World, view: &DarkTowerView) {
    let stalactites: Vec<_> = world.ids_of(ObjectId::DarktowerStalactite).collect();
    for (id, stalactite) in stalactites.into_iter().zip(&view.stalactites) {
        let instance = &mut world.instances[id];
        let ObjectVars::Stalactite { nid, start_y, .. } = instance.vars else { continue };
        match stalactite {
            Some(stalactite) => {
                if !instance.visible {
                    // SERVER_DTASS_STATE 0 made a new one, fading in.
                    instance.visible = true;
                    instance.image_alpha = 0.0;
                }
                instance.y = start_y + stalactite.fallen as f64;
                instance.vars = ObjectVars::Stalactite { nid, start_y, falling: stalactite.falling };
            }
            None => {
                instance.visible = false;
                instance.vars = ObjectVars::Stalactite { nid, start_y, falling: false };
            }
        }
    }
    let doll = world.ids_of(ObjectId::DarktowerTailsdoll).next();
    if let Some(doll) = doll {
        let instance = &mut world.instances[doll];
        let (x, y, frame) = view.doll;
        let moved = gm_sign(x as f64 - instance.x);
        if moved != 0.0 {
            instance.image_xscale = moved;
        }
        (instance.x, instance.y, instance.image_index) = (x as f64, y as f64, frame as f64);
        instance.visible = true;
    }
}
