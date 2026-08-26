use crate::config::cfg;
use crate::packet::{Packet, PacketType};
use crate::player::flags as plrflags;
use crate::server::{DisconnectReason, GameState, OutboxMsg, PeerData, Server};
use crate::states::lobby::{lobby_broadcast_init, lobby_init, send_countdown};

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
    log::debug!("Entering Results state...");
    let cfg = cfg();
    server.state = GameState::Results;
    server.results.countdown = cfg.states.results_misc.timer as f64 * 60.0;

    let mode = cfg.states.gameplay.tournament_mode;
    server.results.open_lobby_after = mode == 0 || tournament_ending(mode, server);

    let mut pkt = Packet::new(PacketType::SERVER_RESULTS);
    let _ = pkt.write_u8(server.game.map as u8);
    if server.results.open_lobby_after {
        outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));
    } else {
        // Spectators aren't getting into Lobby after this Results screen (the
        // tournament keeps going), and the client only leaves Results once it's
        // told to enter Lobby -- so showing it to them here would strand them
        // on the Results screen with no such packet ever coming. Skip it for
        // spectators; they keep watching the tournament unfold normally
        // through the ordinary in-progress-round broadcasts instead.
        for p in server.peers.iter().filter(|p| p.in_game) {
            outbox.push(OutboxMsg::SendTo(p.id, pkt.data().to_vec(), true));
        }
    }

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

    let mode = cfg().states.gameplay.tournament_mode;
    if mode > 0 {
        tournament_advance(mode, server, outbox);
    } else {
        lobby_init(server, outbox);
        lobby_broadcast_init(server, outbox);
    }
}

/// Non-exe in-game survivor(s) tournament_mode would eliminate this round (see
/// Gameplay::tournament_mode for the mode rules), ranked worst-first. If every
/// survivor escaped, there's no bad performance to rank, so the exe is
/// eliminated instead.
fn tournament_eliminate_ids(mode: u8, server: &Server) -> Vec<u16> {
    let exe_id = server.game.exe;
    let ranked = ranked_survivor_ids(server);

    let all_escaped = !ranked.is_empty() && ranked.iter().all(|&id| {
        server.find_peer(id).map(|p| p.plr.flags & plrflags::ESCAPED != 0).unwrap_or(false)
    });

    if ranked.is_empty() || all_escaped {
        return server.find_peer(exe_id as u16).map(|p| vec![p.id]).unwrap_or_default();
    }

    let n = server.peers.iter().filter(|p| p.in_game).count();
    let count = match mode {
        2 if n % 2 == 0 => n / 2,
        3 if n % 2 == 0 => n / 2,
        3 if n % 3 == 0 => n * 2 / 3,
        _ => 1,
    };
    let count = count.min(ranked.len());
    ranked[ranked.len() - count..].to_vec()
}

/// Whether this round's elimination would leave too few in-game players for
/// the tournament to keep going. Floored at 2 regardless of
/// lobby_misc.min_players_required -- a match is exe vs. at least one
/// survivor, so 1 player left is always the tournament's end even if that
/// config is set lower for other reasons (e.g. easier small-scale testing).
fn tournament_ending(mode: u8, server: &Server) -> bool {
    let n = server.peers.iter().filter(|p| p.in_game).count();
    let eliminated = tournament_eliminate_ids(mode, server).len();
    let min_to_continue = cfg().states.lobby_misc.min_players_required.max(2) as usize;
    n.saturating_sub(eliminated) < min_to_continue
}

/// Elimination-tournament progression (see Gameplay::tournament_mode). Kicks
/// the worst-ranked survivor(s) for this round first, always. What happens
/// after that depends on server.results.open_lobby_after (decided back in
/// results_init):
///
/// - Still running (false): a real Lobby entry with spectator promotion
///   suppressed -- anyone waiting in the spectator room stays there instead of
///   joining the running tournament -- followed by a fixed ~1s stub (real time
///   for clients to finish loading the Lobby room) before handing straight to
///   mapvote_init for the next round. No waiting on readiness.
/// - Over (true): an ordinary Lobby entry, promoting everyone same as always,
///   and nothing more -- it's a real Lobby stay from here, not a stub, so the
///   usual ready-triggered countdown (or ?start, or whatever else) takes over
///   exactly like the non-tournament path in results_uninit.
fn tournament_advance(mode: u8, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let eliminate = tournament_eliminate_ids(mode, server);
    log::info!(
        "[Server {}] Tournament: eliminating {} of {} in-game, open_lobby_after={}",
        server.id, eliminate.len(),
        server.peers.iter().filter(|p| p.in_game).count(),
        server.results.open_lobby_after,
    );

    for &id in &eliminate {
        let nick = server.find_peer(id).map(|p| p.nickname.clone()).unwrap_or_default();
        log::info!("[Server {}] Tournament: {} (id {}) eliminated", server.id, crate::colors::colorize(&nick), id);
        // Removing them from server.peers below happens synchronously, before
        // the real ENet disconnect event (and the SERVER_PLAYER_LEFT broadcast
        // that normally goes out when that event's handler finds them) ever
        // arrives -- by then they're already gone and that lookup finds
        // nothing. Send it here instead, so everyone still playing actually
        // drops them from their roster instead of carrying a phantom entry
        // into the next round's CharSelect.
        let mut left_pkt = Packet::new(PacketType::SERVER_PLAYER_LEFT);
        let _ = left_pkt.write_u16(id);
        outbox.push(OutboxMsg::BroadcastEx(left_pkt.data().to_vec(), true, id));
        outbox.push(OutboxMsg::Disconnect(id, DisconnectReason::KickedByHost as u32));
    }
    server.peers.retain(|p| !eliminate.contains(&p.id));

    if server.results.open_lobby_after {
        lobby_init(server, outbox);
        lobby_broadcast_init(server, outbox);
        return;
    }

    // lobby_init's promote-only loop only touches peers whose in_game is
    // currently false -- flip spectators to true first so it leaves them
    // (and the SERVER_IDENTITY_RESPONSE it would send them) alone, then
    // flip them back right after. Every other per-peer reset it does
    // still runs for spectators too, same as any other Lobby entry.
    let spectators: Vec<u16> = server.peers.iter().filter(|p| !p.in_game).map(|p| p.id).collect();
    for &id in &spectators {
        if let Some(pd) = server.find_peer_mut(id) { pd.in_game = true; }
    }
    lobby_init(server, outbox);
    for &id in &spectators {
        if let Some(pd) = server.find_peer_mut(id) { pd.in_game = false; }
    }

    // Only actual participants get the "you're in Lobby now" signal and
    // exe_chance update -- a spectator who stays a spectator never really
    // enters this Lobby stay, and telling their client otherwise desyncs
    // it the same way it used to happen from ?stop into ?start, back
    // before lobby_init became promote-only.
    let pkt = Packet::new(PacketType::SERVER_GAME_BACK_TO_LOBBY);
    for p in server.peers.iter().filter(|p| p.in_game) {
        outbox.push(OutboxMsg::SendTo(p.id, pkt.data().to_vec(), true));
        let mut pkt2 = Packet::new(PacketType::SERVER_LOBBY_EXE_CHANCE);
        let _ = pkt2.write_u8(p.exe_chance);
        outbox.push(OutboxMsg::SendTo(p.id, pkt2.data().to_vec(), true));
    }

    // Hand off to lobby_state_tick's own countdown instead of jumping straight
    // to mapvote_init: that gives every client's freshly-entered Lobby room
    // (player list, banner, etc.) real time to finish loading before the next
    // round's packets arrive. Fixed at 1 second regardless of
    // lobby_misc.lobby_start_timer -- that config is for the ready-triggered
    // wait in a real Lobby stay, not this stub between tournament rounds.
    const TOURNAMENT_LOBBY_STUB_SECS: u8 = 1;
    server.lobby.countdown_sec = TOURNAMENT_LOBBY_STUB_SECS;
    server.lobby.countdown = TOURNAMENT_LOBBY_STUB_SECS as f64 * 60.0;
    server.lobby.tournament_pending = true;
    send_countdown(TOURNAMENT_LOBBY_STUB_SECS, server, outbox);
}

/// Non-exe in-game survivors, ranked best-to-worst by the same criteria as the
/// results screen (see compare_primary/compare_secondary).
fn ranked_survivor_ids(server: &Server) -> Vec<u16> {
    let mut entries: Vec<(u16, ResEntry)> = server.peers.iter()
        .filter(|p| p.in_game && p.id as i32 != server.game.exe)
        .map(|p| (p.id, ResEntry::from_peer(p, server.game.exe)))
        .collect();
    entries.sort_by(|a, b| compare_primary(&a.1, &b.1));
    entries.sort_by(|a, b| compare_secondary(&a.1, &b.1));
    entries.into_iter().map(|(id, _)| id).collect()
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
            if !server.chat_rate_allow(v_id) { return; } // anti-flood
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

            let nick = server.find_peer(v_id).map(|p| p.nickname.clone()).unwrap_or_default();
            log::info!("[{}] (id {}): {}", crate::colors::colorize(&nick), v_id, msg);

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
