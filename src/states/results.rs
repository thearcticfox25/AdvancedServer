use crate::config::cfg;
use crate::packet::{Packet, PacketType};
use crate::player::flags as plrflags;
use crate::server::{GameState, OutboxMsg, PeerData, Server};
use crate::states::lobby::{lobby_broadcast_init, lobby_init};

const RES_EXE: u8      = 0;
const RES_DEMONIZED: u8 = 1;
const RES_DEAD: u8     = 2;
const RES_ALIVE: u8    = 3;
const RES_ESCAPED: u8  = 4;

const SHAMES_1: &[&str] = &[
    "(my skill issue makes me allergic to moving)",
    "(i'm afraid to leave my camp spot)",
    "(how am i not bored of camping)",
    "(i'm too bad to move around the map)",
    "(i just like being a brick)",
];

const SHAMES_2: &[&str] = &[
    "(i can't win without camping bodies)",
    "(i'm afraid revived players will make me lose)",
    "(i camp bodies cuz im bad)",
];

pub fn results_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();
    server.state = GameState::Results;
    server.results.countdown = cfg.states.results_misc.timer as f64 * 60.0;

    let mut pkt = Packet::new(PacketType::SERVER_RESULTS);
    let _ = pkt.write_u8(server.game.map as u8);
    outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));

    log::info!("Server is now in Results");
}

pub fn results_state_left(v_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let remaining = server.peers.iter().filter(|p| p.in_game && p.id != v_id).count();
    let min_to_continue = cfg().states.lobby_misc.min_players_required.max(1) as usize;
    if remaining < min_to_continue {
        results_uninit(server, outbox);
    }
}

fn results_uninit(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    server.game.entities.clear();
    server.game.left.clear();
    lobby_init(server);
    lobby_broadcast_init(server, outbox);
}

pub fn results_state_tick(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    server.results.countdown -= 1.0;
    if server.results.countdown <= 0.0 {
        results_uninit(server, outbox);
    }
}

pub fn results_state_handle(
    v_id: u16,
    packet: &mut Packet,
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
) {
    let ptype = match packet.packet_type() {
        Some(t) => t,
        None => return,
    };

    match ptype {
        PacketType::CLIENT_RESULTS_REQUEST => {
            send_results_to(v_id, server, outbox);
        }

        PacketType::CLIENT_CHAT_MESSAGE => {
            if !server.chat_rate_allow(v_id) { return; } // SEC-L3: anti-flood
            let in_game = server.find_peer(v_id).map(|p| p.in_game).unwrap_or(true);
            if in_game {
                return;
            }
            packet.pos = 2;
            let _pid = packet.read_u16();
            let msg = match packet.read_str() {
                Some(s) => s,
                None => return,
            };
            if let Some(pd) = server.find_peer_mut(v_id) {
                pd.timeout = 0.0;
            }

            if let Some(cmd) = crate::terminal::cmd::parse_cmd(&msg) {
                let op = server.find_peer(v_id).map(|p| p.op).unwrap_or(0);
                if crate::states::lobby::exec_cmd_pub(&cmd, v_id, op, server, outbox) {
                    return;
                }
            }
        }

        PacketType::CLIENT_PING => {
            let mut pkt = Packet::new(PacketType::SERVER_PONG);
            outbox.push(OutboxMsg::SendTo(v_id, pkt.data().to_vec(), false));
        }

        _ => {}
    }
}

fn send_results_to(v_id: u16, server: &Server, outbox: &mut Vec<OutboxMsg>) {

    let mut entries: Vec<ResEntry> = Vec::new();

    for pd in server.peers.iter() {
        if !pd.in_game {
            continue;
        }
        entries.push(ResEntry::from_peer(pd, server.game.exe));
    }
    for pd in server.game.left.iter() {
        if !pd.in_game {
            continue;
        }
        entries.push(ResEntry::from_peer(pd, server.game.exe));
    }

    entries.sort_by(compare_primary);
    entries.sort_by(compare_secondary);

    let viewer_op = server.peers.iter().find(|p| p.id == v_id).map(|p| p.op).unwrap_or(0);
    let cfg = cfg();
    for e in &entries {
        let pkt = build_result_packet(e, server, cfg, viewer_op);
        outbox.push(OutboxMsg::SendTo(v_id, pkt.data().to_vec(), true));
    }
}

struct ResEntry {
    nickname: String,
    flags: u8,
    exe_char: u8,
    surv_char: u8,
    is_exe: bool,
    has_quit: bool,
    danger_time: f64,
    camp_time: f64,
    brain_damage: bool,
    rings: u16,
    kills: u16,
    damage: u16,
    damage_taken: u16,
    stun_time: u16,
    stuns: u16,
    hp_restored: u16,
    survive_time: f64,
}

impl ResEntry {
    fn from_peer(pd: &PeerData, exe_id: i32) -> Self {
        Self {
            nickname: pd.nickname.clone(),
            flags: pd.plr.flags,
            exe_char: pd.exe_char as u8,
            surv_char: pd.surv_char as u8,
            is_exe: pd.id as i32 == exe_id,
            has_quit: pd.plr.flags & plrflags::LEFT != 0,
            danger_time: pd.plr.stats.danger_time,
            camp_time: pd.plr.stats.camp_time,
            brain_damage: pd.plr.stats.brain_damage,
            rings: pd.plr.stats.rings,
            kills: pd.plr.stats.kills,
            damage: pd.plr.stats.damage,
            damage_taken: pd.plr.stats.damage_taken,
            stun_time: pd.plr.stats.stun_time,
            stuns: pd.plr.stats.stuns,
            hp_restored: pd.plr.stats.hp_restored,
            survive_time: pd.plr.stats.survive_time,
        }
    }
}

fn compare_primary(a: &ResEntry, b: &ResEntry) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;

    let ak = a.flags & plrflags::KILLER != 0;
    let bk = b.flags & plrflags::KILLER != 0;
    if ak || bk {

        let c = (bk as i8) - (ak as i8);
        if c != 0 { return c.cmp(&0); }
    }

    let al = a.flags & plrflags::LEFT != 0;
    let bl = b.flags & plrflags::LEFT != 0;
    if al || bl {

        let c = (al as i8) - (bl as i8);
        if c != 0 { return c.cmp(&0); }
    }

    let ad = a.flags & plrflags::DEMONIZED != 0;
    let bd = b.flags & plrflags::DEMONIZED != 0;
    if ad || bd {
        let c = (ad as i8) - (bd as i8);
        if c != 0 { return c.cmp(&0); }
    }

    let adead = a.flags & plrflags::DEAD != 0;
    let bdead = b.flags & plrflags::DEAD != 0;
    if adead || bdead {
        let c = (adead as i8) - (bdead as i8);
        if c != 0 { return c.cmp(&0); }
    }

    Equal
}

fn compare_secondary(a: &ResEntry, b: &ResEntry) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;

    let ak = a.flags & plrflags::KILLER != 0;
    let bk = b.flags & plrflags::KILLER != 0;
    if ak || bk {
        let c = (bk as i8) - (ak as i8);
        if c != 0 { return c.cmp(&0); }
    }

    let al = a.flags & plrflags::LEFT != 0;
    let bl = b.flags & plrflags::LEFT != 0;
    if al || bl { return Equal; }

    let ad = a.flags & plrflags::DEMONIZED != 0;
    let bd = b.flags & plrflags::DEMONIZED != 0;
    if ad || bd { return Equal; }

    let adead = a.flags & plrflags::DEAD != 0;
    let bdead = b.flags & plrflags::DEAD != 0;
    if adead == bdead {
        let diff = b.danger_time - a.danger_time;
        if diff > 0.0 { return Less; }
        if diff < 0.0 { return Greater; }
    }

    Equal
}

fn build_result_packet(e: &ResEntry, server: &Server, cfg: &crate::config::Config, viewer_op: u8) -> Packet {

    let mut plr_type = RES_ALIVE;
    if e.flags & plrflags::ESCAPED != 0 { plr_type = RES_ESCAPED; }
    if e.flags & plrflags::DEMONIZED != 0 { plr_type = RES_DEMONIZED; }
    else if e.flags & plrflags::DEAD != 0 { plr_type = RES_DEAD; }
    else if e.is_exe { plr_type = RES_EXE; }

    let nickname = if cfg.states.lobby_misc.anonymous_mode && viewer_op < 1 {
        "anonymous".to_string()
    } else {
        let postfix = if cfg.states.results_misc.pride {
            if e.brain_damage && (e.flags & plrflags::ESCAPED != 0) {
                let idx = (e.nickname.len() + e.rings as usize) % SHAMES_1.len();
                SHAMES_1[idx]
            } else if e.camp_time >= 30.0 * 60.0 {
                let idx = (e.nickname.len() + e.kills as usize) % SHAMES_2.len();
                SHAMES_2[idx]
            } else {
                ""
            }
        } else {
            ""
        };
        if postfix.is_empty() {
            e.nickname.clone()
        } else {
            format!("{} {}", e.nickname, postfix)
        }
    };


    let char_byte = if e.exe_char != 255 { e.exe_char } else { e.surv_char };

    let mut pkt = Packet::new(PacketType::SERVER_RESULTS_DATA);
    let _ = pkt.write_str(&nickname);
    let _ = pkt.write_u8(char_byte);
    let _ = pkt.write_u8(server.game.ending as u8);
    let _ = pkt.write_u16(server.game.time_sec);
    let _ = pkt.write_u8(if e.has_quit { 1 } else { 0 });
    let _ = pkt.write_u8(plr_type);
    let _ = pkt.write_u16(e.rings);
    let _ = pkt.write_u16(e.kills);
    let _ = pkt.write_u16(e.damage);
    let _ = pkt.write_u16(e.damage_taken);
    let _ = pkt.write_u16(e.stun_time);
    let _ = pkt.write_u16(e.stuns);
    let _ = pkt.write_u16(e.hp_restored);
    write_double_compat(&mut pkt, e.survive_time);
    write_double_compat(&mut pkt, e.danger_time);
    pkt
}

fn write_double_compat(pkt: &mut Packet, v: f64) {
    let _ = pkt.write_f32(v as f32);
    let _ = pkt.write_u32(0);
}
