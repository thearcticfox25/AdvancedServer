pub mod ravine_mist;
pub mod desert_town;
pub mod hide_and_seek;

use crate::server::{BigRingState, OutboxMsg, Server};

/// The round most maps have: 180 s plus 20 s for every player after the first, ring
/// coefficient 5 (the original's map_init).
const DEFAULT_TIME: (usize, usize) = (180, 20);
const DEFAULT_RINGS: u8 = 5;

pub struct MapDef {
    pub name: &'static str,
    /// Round time: base seconds, plus seconds for every in-game player after the first.
    pub time: (usize, usize),
    /// Ring coefficient (map_ring).
    pub rings: u8,
    pub tick: fn(&mut Server, &mut Vec<OutboxMsg>),
}

pub static MAP_LIST: [MapDef; 21] = [
    MapDef { name: "Hide and Seek 2",   time: DEFAULT_TIME, rings: DEFAULT_RINGS, tick: map_tick },
    MapDef { name: "Ravine Mist",       time: (180, 20),    rings: 5,             tick: ravine_mist::rmz_tick },
    MapDef { name: "...",               time: (205, 20),    rings: 5,             tick: map_tick },
    MapDef { name: "Desert Town",       time: DEFAULT_TIME, rings: DEFAULT_RINGS, tick: map_tick },
    MapDef { name: "You Can't Run",     time: (180, 20),    rings: 5,             tick: map_tick },
    MapDef { name: "Limp City",         time: (155, 20),    rings: 5,             tick: map_tick },
    MapDef { name: "Not Perfect",       time: DEFAULT_TIME, rings: DEFAULT_RINGS, tick: map_tick },
    MapDef { name: "Kind and Fair",     time: (180, 20),    rings: 5,             tick: map_tick },
    MapDef { name: "Act 9",             time: (130, 10),    rings: 3,             tick: map_tick },
    MapDef { name: "Nasty Paradise",    time: (155, 20),    rings: 5,             tick: map_tick },
    MapDef { name: "Priceless Freedom", time: (155, 10),    rings: 5,             tick: map_tick },
    MapDef { name: "Volcano Valley",    time: (180, 20),    rings: 5,             tick: map_tick },
    MapDef { name: "Hill",              time: DEFAULT_TIME, rings: DEFAULT_RINGS, tick: map_tick },
    MapDef { name: "Majin Forest",      time: (155, 10),    rings: 3,             tick: map_tick },
    MapDef { name: "Hide and Seek",     time: DEFAULT_TIME, rings: DEFAULT_RINGS, tick: map_tick },
    MapDef { name: "Torture Cave",      time: DEFAULT_TIME, rings: DEFAULT_RINGS, tick: map_tick },
    MapDef { name: "Dark Tower",        time: (205, 20),    rings: 5,             tick: map_tick },
    MapDef { name: "Haunting Dream",    time: (205, 20),    rings: 5,             tick: map_tick },
    MapDef { name: "Mystic Wood",       time: (155, 20),    rings: 5,             tick: map_tick },
    MapDef { name: "Echidna Ruins",     time: (205, 20),    rings: 5,             tick: map_tick },
    MapDef { name: "Fart Zone",         time: (256, 20),    rings: 1,             tick: map_tick },
];

/// Sets up the round of map `map_idx`, or the default round for an unknown index.
pub fn map_init(server: &mut Server, map_idx: usize) {
    let (time, rings) = MAP_LIST.get(map_idx).map_or((DEFAULT_TIME, DEFAULT_RINGS), |map| (map.time, map.rings));
    map_time_ex(server, time.0, time.1);
    map_ring(server, rings);
    server.game.bring_state = BigRingState::None;
}

pub fn map_time_ex(server: &mut Server, base_sec: usize, mul_sec: usize) {
    let cfg = crate::config::cfg();
    let time_sec = if cfg.states.gameplay.banana.disable_timer {
        0u16
    } else {
        let ingame = server.ingame_count();
        (base_sec + ingame.saturating_sub(1) * mul_sec).min(9999) as u16
    };
    server.game.time_sec = time_sec;
    server.game.time = crate::states::seconds(time_sec as f64);
}

pub fn map_tick(_server: &mut Server, _outbox: &mut Vec<OutboxMsg>) {
}

pub fn map_ring(server: &mut Server, ring_coff: u8) {
    let ingame = server.ingame_count();
    let adjusted = if ingame > 3 && ring_coff > 1 { ring_coff - 1 } else { ring_coff };
    server.game.ring_coff = adjusted.max(1);
}
