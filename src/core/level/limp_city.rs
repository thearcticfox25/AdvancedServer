//! Limp City's electric chains (the server's lc_chain) and eyes (lc_eye): the chains
//! warn, then shock whoever touches them; looking down or up at an eye shows what another
//! eye sees for as long as its charge lasts. The eyes hang on swinging chains.

use crate::core::resources::names::sprite;
use crate::core::config::{ticks, LimpCityRules, ticks_per_second};
use crate::core::player::Player;
use crate::core::objects::ids::ObjectId;
use crate::core::world::{InstanceId, ObjectVars, World};
use serde::{Deserialize, Serialize};

/// obj_limpcity_eyechain: it swings a radian every 200 ms, `range` px and 8 degrees a range.
const SWING_MILLISECONDS: f64 = 200.0;
const SWING_DEGREES: f64 = 8.0;
/// An eye hangs 14 px below its chain's end (above, on a chain hanging upwards), sways
/// 2 px, and is 2 px off the end's side.
const EYE_BELOW_CHAIN: f64 = 14.0;
const EYE_SWAY: f64 = 2.0;
const EYE_DELAY: f64 = 40.0;
const CHAIN_END_OFFSET: f64 = 2.0;
/// obj_limpcity_eyeA: the frames showing its charge.
const EYE_CHARGE_FRAMES: f64 = 5.0;
/// How deep chains hang on chains; enough passes to place them all.
const CHAIN_DEPTH: usize = 4;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LimpCity {
    chain: ChainState,
    chain_timer: i32,
    eyes: Vec<Eye>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum ChainState {
    #[default]
    Resting = 0,
    Warning = 1,
    Shocking = 2,
}

/// global.cameraMode 4: where the camera looks while this player watches through an eye.
/// The client moves its camera there; the server lets the player see what is there
/// (states/round.rs seen_by).
pub fn watched_eye(world: &World, own: &Player) -> Option<(f64, f64)> {
    let watching = own.watching_eye?;
    let (_, target) = world.ids_of(ObjectId::LimpcityEyea).find_map(|id| match world.instances[id].vars {
        ObjectVars::Eye { nid, targets: Some(targets), .. } if nid == watching => Some(((), targets)),
        _ => None,
    })?;
    let wanted = if own.is_looking_down { target.0 } else { target.1 };
    world.ids_of(ObjectId::LimpcityEyeb).find_map(|id| match world.instances[id].vars {
        ObjectVars::Eye { nid, used: true, .. } if nid == wanted => Some((world.instances[id].x, world.instances[id].y)),
        _ => None,
    })
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Eye {
    used: bool,
    user: Option<usize>,
    target: u8,
    charge: u8,
    rest: i32,
    timer: i32,
}

/// What clients are told: the chains' state and each eye's use (SERVER_LCEYE_STATE;
/// its user knows by Player::watching_eye).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LimpCityView {
    pub chains: u8,
    pub eyes: Vec<EyeView>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EyeView {
    pub used: bool,
    pub target: u8,
    pub charge: u8,
}

/// lc_init makes two eyes, for the eyeAs with nid 0 and 1.
const EYES: usize = 2;

impl LimpCity {
    pub(super) fn new() -> LimpCity {
        let eye = Eye { used: false, user: None, target: 0, charge: 100, rest: 0, timer: 0 };
        LimpCity { eyes: vec![eye; EYES], ..LimpCity::default() }
    }

    pub(super) fn tick(&mut self, rules: &LimpCityRules, players: &mut [Player]) {
        let wait = match self.chain {
            ChainState::Resting => rules.chain_rest_seconds,
            ChainState::Warning => rules.chain_warning_seconds,
            ChainState::Shocking => rules.chain_shock_seconds,
        };
        if self.chain_timer >= ticks(wait) {
            self.chain = match self.chain {
                ChainState::Resting => ChainState::Warning,
                ChainState::Warning => ChainState::Shocking,
                ChainState::Shocking => ChainState::Resting,
            };
            self.chain_timer = 0;
        }
        self.chain_timer += 1;

        for eye in &mut self.eyes {
            if eye.rest > 0 {
                eye.rest -= 1;
                continue;
            }
            if eye.timer >= ticks_per_second() as i32 {
                if eye.used && eye.charge > 0 {
                    eye.charge = eye.charge.saturating_sub(rules.eye_use_cost);
                    if eye.charge < rules.eye_min_charge {
                        eye.rest = ticks(rules.eye_rest_seconds);
                        eye.used = false;
                        if let Some(user) = eye.user.take().and_then(|user| players.get_mut(user)) {
                            user.watching_eye = None;
                        }
                    }
                } else if !eye.used && eye.charge < 100 {
                    eye.charge = eye.charge.saturating_add(rules.eye_recharge).min(100);
                }
                eye.timer = 0;
            }
            eye.timer += 1;
        }
    }

    /// CLIENT_LCEYE_REQUEST_ACTIVATE
    pub(super) fn request_eye(&mut self, rules: &LimpCityRules, index: usize, player: &mut Player, nid: u8, target: u8, on: bool) {
        let Some(eye) = self.eyes.get_mut(nid as usize) else { return };
        if on && (eye.used || eye.charge < rules.eye_min_charge) {
            return;
        }
        (eye.used, eye.user, eye.target, eye.timer) = if on { (true, Some(index), target, 0) } else { (false, None, 0, 0) };
        player.watching_eye = on.then_some(nid);
    }

    pub(super) fn view(&self) -> LimpCityView {
        LimpCityView {
            chains: self.chain as u8,
            eyes: self.eyes.iter().map(|eye| EyeView { used: eye.used, target: eye.target, charge: eye.charge }).collect(),
        }
    }
}

pub(super) fn apply(world: &mut World, view: &LimpCityView) {
    let ids: Vec<InstanceId> = world.ids_of(ObjectId::LimpcityEchain1).chain(world.ids_of(ObjectId::LimpcityEyea)).chain(world.ids_of(ObjectId::LimpcityEyeb)).collect();
    for id in ids {
        let instance = &mut world.instances[id];
        match &mut instance.vars {
            ObjectVars::ElectricChain { state } => *state = view.chains,
            ObjectVars::Eye { nid, targets: Some(_), used, charge, .. } => {
                if let Some(eye) = view.eyes.get(*nid as usize) {
                    (*used, *charge) = (eye.used, eye.charge);
                    // obj_limpcity_eyeA Step: its frame shows its charge.
                    instance.image_index = (eye.charge as f64 / 100.0 * EYE_CHARGE_FRAMES).floor();
                }
            }
            // The eye shown by an eyeA in use.
            ObjectVars::Eye { nid, targets: None, used, .. } => {
                *used = view.eyes.iter().any(|eye| eye.used && eye.target == *nid);
                let sprite = if *used { sprite::SPR_LIMPCITY_EYE } else { sprite::SPR_LIMPCITY_EYE_RECHARGE };
                instance.sprite_index = Some(sprite);
            }
            _ => {}
        }
    }
}

/// The Step and End Step of the eye chains and eyes: the chains swing, each hangs on
/// the end of the chain it names, and the eyes hang below theirs. `time_ms` is
/// current_time; the simulation uses 0, clients the clock.
pub fn hang_eyes(world: &mut World, time_ms: f64) {
    let chains: Vec<InstanceId> = world.ids_of(ObjectId::LimpcityEyechain).chain(world.ids_of(ObjectId::LimpcityEyechain2)).collect();
    if chains.is_empty() {
        return;
    }
    let phase = |delay: f64| (time_ms / SWING_MILLISECONDS + delay).sin_cos();
    // The end of a chain: where what hangs on it starts (before its own side offset).
    let end = |world: &World, chain: InstanceId| {
        let instance = &world.instances[chain];
        let height = instance.sprite_index.map_or(0.0, |sprite| world.sprites.get(sprite).height as f64) * instance.image_yscale;
        let width = instance.sprite_index.map_or(0.0, |sprite| world.sprites.get(sprite).width as f64) * instance.image_xscale;
        let angle = instance.image_angle.to_radians();
        (instance.x + height * angle.sin(), instance.y + height * angle.cos(), width / 2.0 + CHAIN_END_OFFSET)
    };
    let sid_of = |world: &World, chain: InstanceId| match world.instances[chain].vars {
        ObjectVars::EyeChain { sid, .. } => sid,
        _ => -1,
    };
    for _ in 0..CHAIN_DEPTH {
        for &chain in &chains {
            let ObjectVars::EyeChain { tid, range, map_x, map_y, .. } = world.instances[chain].vars else { continue };
            let upward = world.instances[chain].object == ObjectId::LimpcityEyechain2;
            // Only the first kind hangs on another chain (its End Step).
            let parent = (!upward && tid != -1).then(|| chains.iter().copied().find(|&other| world.instances[other].object == ObjectId::LimpcityEyechain && sid_of(world, other) == tid)).flatten();
            let (origin_x, origin_y) = match parent {
                Some(parent) => {
                    let (x, y, side) = end(world, parent);
                    (x - side, y - CHAIN_END_OFFSET)
                }
                None => (map_x, map_y),
            };
            let (sin, cos) = phase(0.0);
            let instance = &mut world.instances[chain];
            instance.image_angle = if upward { 180.0 } else { 0.0 } + sin * range * SWING_DEGREES;
            instance.x = origin_x + sin * range;
            instance.y = origin_y + cos * range;
        }
    }
    let eyes: Vec<InstanceId> = world.ids_of(ObjectId::LimpcityEyea).chain(world.ids_of(ObjectId::LimpcityEyeb)).collect();
    for eye in eyes {
        let ObjectVars::Eye { tid, map_x, map_y, .. } = world.instances[eye].vars else { continue };
        let holder = chains.iter().copied().filter(|&chain| sid_of(world, chain) == tid).last();
        let (origin_x, origin_y, below) = match holder {
            Some(chain) if world.instances[chain].object == ObjectId::LimpcityEyechain2 => {
                let (x, y, side) = end(world, chain);
                (x + side, y + CHAIN_END_OFFSET, -EYE_BELOW_CHAIN)
            }
            Some(chain) => {
                let (x, y, side) = end(world, chain);
                (x - side, y - CHAIN_END_OFFSET, EYE_BELOW_CHAIN)
            }
            None => (map_x, map_y - EYE_BELOW_CHAIN, EYE_BELOW_CHAIN),
        };
        let (sin, _) = phase(EYE_DELAY);
        let instance = &mut world.instances[eye];
        instance.x = origin_x + sin * EYE_SWAY;
        instance.y = origin_y + below;
    }
}
