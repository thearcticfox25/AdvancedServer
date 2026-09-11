use serde::Deserialize;
use std::collections::HashMap;

/// One object's box, exactly as MapPhysics.json spells it: `w`/`h` are the
/// on-disk key names, renamed here so the code reads in full words.
#[derive(Debug, Clone, Deserialize)]
pub struct Hitbox {
    pub x: f32,
    pub y: f32,
    #[serde(rename = "w")]
    pub width: f32,
    #[serde(rename = "h")]
    pub height: f32,
}

/// One object as MapPhysics.json describes it. `id`, `object`, `x` and `y` are
/// not used by any check yet; they are kept because they document the file the
/// extractor (tools/extract_gm_physics.py) writes, and because a violation report
/// is far easier to chase with the offending object's own name in it.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct PhysicsInstance {
    pub id:       String,
    pub object:   String,
    pub category: String,
    pub x:        f32,
    pub y:        f32,
    pub hitbox:   Hitbox,
    #[serde(default)]
    pub extra:    Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RoomPhysics {
    pub width:     f32,
    pub height:    f32,
    pub instances: Vec<PhysicsInstance>,
}

#[derive(Debug, Deserialize)]
struct PhysicsFile {
    map_index: HashMap<String, String>,
    rooms:     HashMap<String, RoomPhysics>,
}

#[derive(Debug, Clone, Copy)]
pub struct Aabb {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Aabb {
    /// Not used by a check yet, unlike `overlaps`: kept as the point-sized
    /// counterpart for the kill-box work MapPhysics.kills is being extracted for.
    #[allow(dead_code)]
    #[inline]
    pub fn contains_point(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width &&
        py >= self.y && py <= self.y + self.height
    }

    #[inline]
    pub fn overlaps(&self, other: &Aabb) -> bool {
        self.x < other.x + other.width  &&
        self.x + self.width  > other.x   &&
        self.y < other.y + other.height &&
        self.y + self.height > other.y
    }

    #[inline]
    pub fn shrink(&self, margin: f32) -> Aabb {
        Aabb {
            x: self.x + margin,
            y: self.y + margin,
            width:  (self.width  - margin * 2.0).max(0.0),
            height: (self.height - margin * 2.0).max(0.0),
        }
    }
}

#[derive(Debug, Clone)]
/// The parts of a map the server can reason about. `solids` backs the current
/// solid check; `kills` and `springs` are extracted and parked, waiting for the
/// death-validation and launch-envelope checks they were extracted for.
pub struct MapPhysics {
    pub width:  f32,
    pub height: f32,
    pub solids:   Vec<Aabb>,
    #[allow(dead_code)]
    pub kills:    Vec<Aabb>,
    #[allow(dead_code)]
    pub springs:  Vec<(Aabb, SpringDir)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpringDir { Up, Left, Right }

fn to_aabb(hitbox: &Hitbox) -> Aabb {
    Aabb { x: hitbox.x, y: hitbox.y, width: hitbox.width, height: hitbox.height }
}

impl MapPhysics {
    fn from_room(room: &RoomPhysics) -> Self {
        let mut solids  = Vec::new();
        let mut kills   = Vec::new();
        let mut springs = Vec::new();

        for inst in &room.instances {
            let aabb = to_aabb(&inst.hitbox);
            match inst.category.as_str() {
                "solid" | "conveyor" | "lift" => {
                    solids.push(aabb);
                }
                "kill" | "kill_platform" => {
                    kills.push(aabb);
                }
                "spring" => {
                    let dir = match inst.extra.as_deref() {
                        Some("left")  => SpringDir::Left,
                        Some("right") => SpringDir::Right,
                        _             => SpringDir::Up,
                    };
                    springs.push((aabb, dir));
                }
                _ => {}
            }
        }

        MapPhysics {
            width:   room.width,
            height:  room.height,
            solids,
            kills,
            springs,
        }
    }

    pub fn player_aabb(px: f32, py: f32) -> Aabb {
        Aabb { x: px - 12.0, y: py - 18.0, width: 24.0, height: 32.0 }
    }

    pub fn solid_violation(&self, px: f32, py: f32) -> Option<Aabb> {
        let plr = Self::player_aabb(px, py);
        self.solids.iter().find(|solid| plr.overlaps(&solid.shrink(4.0))).copied()
    }

    /// Parked alongside MapPhysics.kills: the server cannot yet tell an honest
    /// death from a faked one, so nothing calls this.
    #[allow(dead_code)]
    pub fn kill_violation(&self, px: f32, py: f32) -> Option<Aabb> {
        let plr = Self::player_aabb(px, py);
        self.kills.iter().find(|kill_box| plr.overlaps(kill_box)).copied()
    }

    pub fn out_of_bounds(&self, px: f32, py: f32) -> bool {
        let margin = 512.0;
        px < -margin || py < -margin ||
        px > self.width  + margin ||
        py > self.height + margin
    }
}

pub fn f16_to_f32(half: u16) -> f32 {
    let sign = (half & 0x8000) as u32;
    let exponent_bits  = (half & 0x7C00) as u32 >> 10;
    let mantissa_bits = (half & 0x03FF) as u32;
    let bits: u32 = if exponent_bits == 0 {
        if mantissa_bits == 0 {
            sign << 16
        } else {
            // Subnormal: shift the mantissa up until its implicit bit appears,
            // paying one exponent step per shift.
            let mut mantissa = mantissa_bits;
            let mut exponent = 127u32.wrapping_sub(14);
            while mantissa & 0x400 == 0 {
                mantissa <<= 1;
                exponent = exponent.wrapping_sub(1);
            }
            mantissa &= 0x3FF;
            (sign << 16) | (exponent << 23) | (mantissa << 13)
        }
    } else if exponent_bits == 31 {
        (sign << 16) | 0x7F80_0000 | (mantissa_bits << 13)
    } else {
        (sign << 16) | ((exponent_bits + 127 - 15) << 23) | (mantissa_bits << 13)
    };
    f32::from_bits(bits)
}

pub const MAX_H_SPEED: f32 = 12.0;
pub const MAX_Y_SPEED: f32 = 12.0;
pub const GRAVITY:     f32 = 0.21875;

/// Per-character horizontal speed cap. check_correction deliberately uses the
/// single global MAX_H_SPEED instead, so this stays unused until the envelope is
/// tightened per character.
#[allow(dead_code)]
pub fn char_max_hspeed(surv_char: i8) -> f32 {
    match surv_char {
        2 => 9.0,
        _ => 11.0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrectionVerdict {
    Ok,
    VelocityOverflow,
    PositionMismatch,
}

pub const MS_PER_TICK: f32 = 16.667;
/// Per-axis slack: a diagonal spring launches (+-12, -12) in a single tick, plus u16
/// position / f16 velocity quantization. Keeps legit spring/edge cases inside the box.
pub const POS_MARGIN:  f32 = 24.0;
/// f16 rounding of the exact 12 px/tick clamp value.
pub const VEL_EPS:     f32 = 0.75;
/// Cap on how many ticks a single inter-packet gap may represent. Bounds how far a
/// fake-lag can stretch the envelope (40 * 12 = 480 px, below the 700 px hard cap),
/// while still tolerating ~0.66 s lag spikes without rubber-banding a legit player.
pub const MAX_TICKS:   f32 = 40.0;

/// Reachable-set validator. The game hard-clamps `xspd`/`yspd` to
/// `[-maxHSpeed, maxHSpeed]` / `[-maxYSpeed, maxYSpeed]` (springs, boost and every
/// character special included - all <= 12), then adds gravity to `yspd`, *every* tick
/// before applying movement. So the union of reachable next-positions over ALL inputs
/// collapses to a per-axis box; no button combination, slope or ability can leave it.
/// We therefore never enumerate inputs - we test whether the reported state lies in the
/// box. Warps (abyss/teleport/lift/zipline) escape it and are handled by the caller's
/// per-map skip + the hard distance cap. `_prev_vel` is unused: the box is anchored on
/// the last accepted position, not on an extrapolation of the last velocity.
pub fn check_correction(
    prev_pos:   (f32, f32),
    _prev_vel:  (f32, f32),
    new_pos:    (f32, f32),
    new_vel:    (f32, f32),
    elapsed_ms: f32,
) -> (CorrectionVerdict, f32) {
    // Velocity envelope - map-independent (lifts/ziplines zero the velocity).
    let vx_cap = MAX_H_SPEED + VEL_EPS;
    let vy_cap = MAX_Y_SPEED + GRAVITY + VEL_EPS;
    if new_vel.0.abs() > vx_cap || new_vel.1.abs() > vy_cap {
        let overflow = new_vel.0.abs().max(new_vel.1.abs());
        return (CorrectionVerdict::VelocityOverflow, overflow);
    }

    // Position envelope - reachable box from the last accepted position.
    let ticks_elapsed = (elapsed_ms / MS_PER_TICK).round().clamp(1.0, MAX_TICKS);
    let allow_x = MAX_H_SPEED * ticks_elapsed + POS_MARGIN;
    let allow_y = (MAX_Y_SPEED + GRAVITY) * ticks_elapsed + POS_MARGIN;

    let dx = (new_pos.0 - prev_pos.0).abs();
    let dy = (new_pos.1 - prev_pos.1).abs();
    if dx > allow_x || dy > allow_y {
        return (CorrectionVerdict::PositionMismatch, (dx - allow_x).max(dy - allow_y));
    }
    (CorrectionVerdict::Ok, dx.max(dy))
}

pub struct PhysicsTable(Vec<Option<MapPhysics>>);

impl PhysicsTable {
    pub fn get(&self, map_id: i8) -> Option<&MapPhysics> {
        self.0.get(map_id as usize)?.as_ref()
    }
}

static PHYSICS: std::sync::OnceLock<PhysicsTable> = std::sync::OnceLock::new();

pub fn load(path: &str) -> usize {
    let raw = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            log::warn!("[physics] Cannot read {path}: {error} - physics AC disabled");
            PHYSICS.set(PhysicsTable(Vec::new())).ok();
            return 0;
        }
    };
    let file: PhysicsFile = match serde_json::from_str(&raw) {
        Ok(parsed) => parsed,
        Err(error) => {
            log::warn!("[physics] Parse error in {path}: {error} - physics AC disabled");
            PHYSICS.set(PhysicsTable(Vec::new())).ok();
            return 0;
        }
    };

    let max_id = file.map_index.keys()
        .filter_map(|key| key.parse::<usize>().ok())
        .max()
        .unwrap_or(0);

    let mut table: Vec<Option<MapPhysics>> = vec![None; max_id + 1];
    let mut count = 0;

    for (id_str, room_name) in &file.map_index {
        let id: usize = match id_str.parse() {
            Ok(parsed) => parsed,
            Err(_) => continue,
        };
        if let Some(room) = file.rooms.get(room_name) {
            if let Some(slot) = table.get_mut(id) {
                *slot = Some(MapPhysics::from_room(room));
                count += 1;
            }
        }
    }

    log::info!("[physics] Loaded {count} map(s) from {path}");
    PHYSICS.set(PhysicsTable(table)).ok();
    count
}

pub fn table() -> &'static PhysicsTable {
    PHYSICS.get_or_init(|| PhysicsTable(Vec::new()))
}
