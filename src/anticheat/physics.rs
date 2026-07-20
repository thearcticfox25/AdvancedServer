use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct Hitbox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

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
    pub w: f32,
    pub h: f32,
}

impl Aabb {
    #[inline]
    pub fn contains_point(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w &&
        py >= self.y && py <= self.y + self.h
    }

    #[inline]
    pub fn overlaps(&self, other: &Aabb) -> bool {
        self.x < other.x + other.w &&
        self.x + self.w > other.x &&
        self.y < other.y + other.h &&
        self.y + self.h > other.y
    }

    #[inline]
    pub fn shrink(&self, margin: f32) -> Aabb {
        Aabb {
            x: self.x + margin,
            y: self.y + margin,
            w: (self.w - margin * 2.0).max(0.0),
            h: (self.h - margin * 2.0).max(0.0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MapPhysics {
    pub width:  f32,
    pub height: f32,
    pub solids:   Vec<Aabb>,
    pub kills:    Vec<Aabb>,
    pub springs:  Vec<(Aabb, SpringDir)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpringDir { Up, Left, Right }

fn to_aabb(h: &Hitbox) -> Aabb {
    Aabb { x: h.x, y: h.y, w: h.w, h: h.h }
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
        Aabb { x: px - 12.0, y: py - 18.0, w: 24.0, h: 32.0 }
    }

    pub fn solid_violation(&self, px: f32, py: f32) -> Option<Aabb> {
        let plr = Self::player_aabb(px, py);
        self.solids.iter().find(|s| plr.overlaps(&s.shrink(4.0))).copied()
    }

    pub fn kill_violation(&self, px: f32, py: f32) -> Option<Aabb> {
        let plr = Self::player_aabb(px, py);
        self.kills.iter().find(|k| plr.overlaps(k)).copied()
    }

    pub fn out_of_bounds(&self, px: f32, py: f32) -> bool {
        let margin = 512.0;
        px < -margin || py < -margin ||
        px > self.width  + margin ||
        py > self.height + margin
    }
}

pub fn f16_to_f32(h: u16) -> f32 {
    let sign  = (h & 0x8000) as u32;
    let exp   = (h & 0x7C00) as u32 >> 10;
    let mant  = (h & 0x03FF) as u32;
    let bits: u32 = if exp == 0 {
        if mant == 0 {
            sign << 16
        } else {
            let mut m = mant;
            let mut e = 127u32.wrapping_sub(14);
            while m & 0x400 == 0 { m <<= 1; e = e.wrapping_sub(1); }
            m &= 0x3FF;
            (sign << 16) | (e << 23) | (m << 13)
        }
    } else if exp == 31 {
        (sign << 16) | 0x7F80_0000 | (mant << 13)
    } else {
        (sign << 16) | ((exp + 127 - 15) << 23) | (mant << 13)
    };
    f32::from_bits(bits)
}

pub const MAX_H_SPEED: f32 = 12.0;
pub const MAX_Y_SPEED: f32 = 12.0;
pub const GRAVITY:     f32 = 0.21875;

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
/// Per-axis slack: a diagonal spring launches (±12, −12) in a single tick, plus u16
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
/// character special included — all ≤ 12), then adds gravity to `yspd`, *every* tick
/// before applying movement. So the union of reachable next-positions over ALL inputs
/// collapses to a per-axis box; no button combination, slope or ability can leave it.
/// We therefore never enumerate inputs — we test whether the reported state lies in the
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
    // Velocity envelope — map-independent (lifts/ziplines zero the velocity).
    let vx_cap = MAX_H_SPEED + VEL_EPS;
    let vy_cap = MAX_Y_SPEED + GRAVITY + VEL_EPS;
    if new_vel.0.abs() > vx_cap || new_vel.1.abs() > vy_cap {
        let overflow = new_vel.0.abs().max(new_vel.1.abs());
        return (CorrectionVerdict::VelocityOverflow, overflow);
    }

    // Position envelope — reachable box from the last accepted position.
    let n = (elapsed_ms / MS_PER_TICK).round().clamp(1.0, MAX_TICKS);
    let allow_x = MAX_H_SPEED * n + POS_MARGIN;
    let allow_y = (MAX_Y_SPEED + GRAVITY) * n + POS_MARGIN;

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
        Ok(r)  => r,
        Err(e) => {
            log::warn!("[physics] Cannot read {path}: {e} — physics AC disabled");
            PHYSICS.set(PhysicsTable(Vec::new())).ok();
            return 0;
        }
    };
    let file: PhysicsFile = match serde_json::from_str(&raw) {
        Ok(f)  => f,
        Err(e) => {
            log::warn!("[physics] Parse error in {path}: {e} — physics AC disabled");
            PHYSICS.set(PhysicsTable(Vec::new())).ok();
            return 0;
        }
    };

    let max_id = file.map_index.keys()
        .filter_map(|k| k.parse::<usize>().ok())
        .max()
        .unwrap_or(0);

    let mut table: Vec<Option<MapPhysics>> = vec![None; max_id + 1];
    let mut count = 0;

    for (id_str, room_name) in &file.map_index {
        let id: usize = match id_str.parse() {
            Ok(n)  => n,
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
