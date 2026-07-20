pub mod hide_and_seek2;
pub mod ravine_mist;
pub mod dot_dot_dot;
pub mod desert_town;
pub mod you_cant_run;
pub mod limp_city;
pub mod not_perfect;
pub mod kind_and_fair;
pub mod act9;
pub mod nasty_paradise;
pub mod priceless_freedom;
pub mod volcano_valley;
pub mod hill;
pub mod majin_forest;
pub mod hide_and_seek;
pub mod torture_cave;
pub mod dark_tower;
pub mod haunting_dream;
pub mod mystic_wood;
pub mod echidna_ruins;
pub mod fart_zone;

use crate::packet::Packet;
use crate::server::{BigRingState, OutboxMsg, Server};

pub struct MapDef {
    pub name: &'static str,
    pub ring_count: u8,
    pub init: fn(&mut Server, &mut Vec<OutboxMsg>),
    pub tick: fn(&mut Server, &mut Vec<OutboxMsg>),
    pub tcp_msg: fn(u16, &mut Packet, &mut Server, &mut Vec<OutboxMsg>),
    pub left: fn(u16, &mut Server, &mut Vec<OutboxMsg>),
}

pub const MAP_COUNT: usize = 21;

pub static MAP_LIST: [MapDef; MAP_COUNT] = [
    MapDef { name: "Hide and Seek 2",   ring_count: 22, init: hide_and_seek2::hs2_init,        tick: map_tick, tcp_msg: map_tcpmsg,                    left: map_left },
    MapDef { name: "Ravine Mist",        ring_count: 27, init: ravine_mist::rmz_init,           tick: ravine_mist::rmz_tick,  tcp_msg: ravine_mist::rmz_tcpmsg,  left: ravine_mist::rmz_left },
    MapDef { name: "...",                ring_count: 25, init: dot_dot_dot::dot_init,           tick: map_tick, tcp_msg: map_tcpmsg,                    left: map_left },
    MapDef { name: "Desert Town",        ring_count: 25, init: map_init,                        tick: map_tick, tcp_msg: map_tcpmsg,                    left: map_left },
    MapDef { name: "You Can't Run",      ring_count: 27, init: you_cant_run::ycr_init,          tick: map_tick, tcp_msg: map_tcpmsg,                    left: map_left },
    MapDef { name: "Limp City",          ring_count: 23, init: limp_city::lc_init,              tick: map_tick, tcp_msg: limp_city::lc_tcpmsg,          left: map_left },
    MapDef { name: "Not Perfect",        ring_count: 59, init: not_perfect::np_init,            tick: map_tick, tcp_msg: map_tcpmsg,                    left: map_left },
    MapDef { name: "Kind and Fair",      ring_count: 31, init: kind_and_fair::kaf_init,         tick: map_tick, tcp_msg: kind_and_fair::kaf_tcpmsg,     left: map_left },
    MapDef { name: "Act 9",              ring_count: 38, init: act9::act9_init,                 tick: map_tick, tcp_msg: map_tcpmsg,                    left: map_left },
    MapDef { name: "Nasty Paradise",     ring_count: 26, init: nasty_paradise::nap_init,        tick: map_tick, tcp_msg: nasty_paradise::nap_tcpmsg,    left: map_left },
    MapDef { name: "Priceless Freedom",  ring_count: 38, init: priceless_freedom::pf_init,     tick: map_tick, tcp_msg: priceless_freedom::pf_tcpmsg,  left: map_left },
    MapDef { name: "Volcano Valley",     ring_count: 27, init: volcano_valley::vv_init,         tick: map_tick, tcp_msg: volcano_valley::vv_tcpmsg,     left: map_left },
    MapDef { name: "Hill",               ring_count: 26, init: hill::hill_init,                 tick: map_tick, tcp_msg: map_tcpmsg,                    left: map_left },
    MapDef { name: "Majin Forest",       ring_count: 20, init: majin_forest::maj_init,          tick: map_tick, tcp_msg: map_tcpmsg,                    left: map_left },
    MapDef { name: "Hide and Seek",      ring_count: 21, init: map_init,                        tick: map_tick, tcp_msg: map_tcpmsg,                    left: map_left },
    MapDef { name: "Torture Cave",       ring_count: 27, init: torture_cave::tc_init,           tick: map_tick, tcp_msg: map_tcpmsg,                    left: map_left },
    MapDef { name: "Dark Tower",         ring_count: 31, init: dark_tower::dt_init,             tick: map_tick, tcp_msg: dark_tower::dt_tcpmsg,         left: map_left },
    MapDef { name: "Haunting Dream",     ring_count: 31, init: haunting_dream::hd_init,         tick: map_tick, tcp_msg: haunting_dream::hd_tcpmsg,     left: map_left },
    MapDef { name: "Mystic Wood",        ring_count: 26, init: mystic_wood::wd_init,            tick: map_tick, tcp_msg: map_tcpmsg,                    left: map_left },
    MapDef { name: "Echidna Ruins",      ring_count: 33, init: echidna_ruins::mj_init,         tick: map_tick, tcp_msg: map_tcpmsg,                    left: map_left },
    MapDef { name: "Fart Zone",          ring_count: 15, init: fart_zone::ft_init,              tick: map_tick, tcp_msg: fart_zone::ft_tcpmsg,          left: map_left },
];

pub fn map_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    map_time_ex(server, 180, 20);
    map_ring(server, 5);
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
    map_time(server, time_sec);
}

pub fn map_tick(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {

}

pub fn map_tcpmsg(_peer_id: u16, _packet: &mut Packet, _server: &mut Server, _outbox: &mut Vec<OutboxMsg>) {

}

pub fn map_left(_peer_id: u16, _server: &mut Server, _outbox: &mut Vec<OutboxMsg>) {

}

pub fn map_time(server: &mut Server, time_sec: u16) {
    server.game.time_sec = time_sec;
    server.game.time     = time_sec as f64 * 60.0;
}

pub fn map_ring(server: &mut Server, ring_coff: u8) {
    let ingame = server.ingame_count();
    let coff = if ingame > 3 && ring_coff > 1 { ring_coff - 1 } else { ring_coff };
    server.game.ring_coff = coff.max(1);
}
