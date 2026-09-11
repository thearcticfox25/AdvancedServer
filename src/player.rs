use std::time::Instant;

pub mod flags {
    pub const ESCAPED: u8 = 0x01;
    pub const DEAD: u8 = 0x02;
    pub const DEMONIZED: u8 = 0x04;
    pub const REVIVED: u8 = 0x08;
    pub const CANTREVIVE: u8 = 0x10;
    pub const LEFT: u8 = 0x20;
    pub const KILLER: u8 = 0x40;
}

#[derive(Debug, Clone, Default)]
pub struct PlayerStats {
    pub survive_time: f64,
    pub danger_time: f64,
    pub camp_time: f64,
    pub brain_damage: bool,
    pub stun_time: u16,
    pub stuns: u16,
    pub hp_restored: u16,
    pub rings: u16,
    pub damage: u16,
    pub damage_taken: u16,
    pub kills: u16,
}

#[derive(Debug, Clone)]
pub struct Player {
    pub errors: u16,
    pub ex_teleport: u8,
    pub timeout: f64,

    pub mod_tool: bool,
    pub mod_tool_timer: u32,
    pub last_packet: Option<Instant>,

    pub is_attacking: bool,
    pub attack_timer: f64,
    pub last_attack: Option<Instant>,
    pub cooldown: f64,
    pub tails_last_proj: Option<Instant>,

    pub ping_last: u16,
    pub ping_total: f64,
    pub ping_timer: f64,
    pub rings: i16,
    pub last_rings: Option<Instant>,
    pub heal_rings: u16,

    pub state: u8,
    pub flags: u8,
    pub death_timer_sec: u8,
    pub death_timer: f64,
    pub revival: f64,
    pub revival_init: [i32; 5],

    pub start_pos: (f32, f32),
    pub pos: (f32, f32),
    pub vel: (f32, f32),
    pub good_pos: (f32, f32),

    pub hp: i8,
    pub revival_times: u8,
    pub hp_credit: i8,
    pub tails_charge: u8,
    pub red_ring: bool,

    pub data: [u8; 4],

    pub stats: PlayerStats,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            errors: 0,
            ex_teleport: 0,
            timeout: 0.0,
            mod_tool: false,
            mod_tool_timer: 0,
            last_packet: None,
            is_attacking: false,
            attack_timer: 0.0,
            last_attack: None,
            cooldown: 0.0,
            tails_last_proj: None,
            ping_last: 0,
            ping_total: 0.0,
            ping_timer: 0.0,
            rings: 0,
            last_rings: None,
            heal_rings: 0,
            state: 0,
            flags: 0,
            death_timer_sec: 0,
            death_timer: 0.0,
            revival: 0.0,
            revival_init: [-1; 5],
            start_pos: (0.0, 0.0),
            pos: (0.0, 0.0),
            vel: (0.0, 0.0),
            good_pos: (0.0, 0.0),
            hp: -1,
            revival_times: 0,
            hp_credit: 0,
            tails_charge: 0,
            red_ring: false,
            data: [0; 4],
            stats: PlayerStats::default(),
        }
    }
}

pub fn vec2_dist(from: (f32, f32), to: (f32, f32)) -> f32 {
    let dx = from.0 - to.0;
    let dy = from.1 - to.1;
    (dx * dx + dy * dy).sqrt()
}
