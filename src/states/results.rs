use crate::config::cfg;
use crate::packet::{Packet, PacketType};
use crate::player::flags as plrflags;
use crate::server::{DisconnectReason, GameState, OutboxMsg, PeerData, Server};
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
    log::debug!("Entering Results state...");
    let cfg = cfg();
    server.state = GameState::Results;
    server.results.countdown = cfg.states.results_misc.timer as f64 * 60.0;

    let mut pkt = Packet::new(PacketType::SERVER_RESULTS);
    let _ = pkt.write_u8(server.game.map as u8);

    if tournament_continues(server) {
        // The waiting room isn't getting into Lobby after this Results screen
        // (the bracket keeps going), and the client only leaves Results once
        // it's told to enter Lobby -- so showing it to them here would strand
        // them on the Results screen with no such packet ever coming. Anyone
        // spectating is the exception: they followed the round on the same
        // screens its players did and follow it out the same way.
        for peer in server.peers.iter().filter(|peer| peer.follows_round()) {
            outbox.push(OutboxMsg::SendTo(peer.id, pkt.data().to_vec(), true));
        }
    } else {
        outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));
    }

    log::info!("Server is now in Results");
}

pub fn results_state_left(player_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let remaining = server.peers.iter().filter(|peer| peer.in_game && peer.id != player_id).count();
    let min_to_continue = cfg().states.lobby_misc.min_players_required.max(1) as usize;
    if remaining < min_to_continue {
        results_uninit(server, outbox);
    }
}

fn results_uninit(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    server.game.entities.clear();
    server.game.left.clear();

    tournament_advance(server, outbox);
}

/// Non-exe in-game survivor(s) tournament_mode would eliminate this round (see
/// Gameplay::tournament_mode for the mode rules), ranked worst-first. If every
/// survivor escaped, there's no bad performance to rank, so the exe is
/// eliminated instead.
fn tournament_eliminate_ids(mode: u8, server: &Server) -> Vec<u16> {
    let exe_id = server.game.exe;
    let ranked = ranked_survivor_ids(server);

    let all_escaped = !ranked.is_empty() && ranked.iter().all(|&id| {
        server.find_peer(id).map(|peer| peer.plr.flags & plrflags::ESCAPED != 0).unwrap_or(false)
    });

    if ranked.is_empty() || all_escaped {
        return server.find_peer(exe_id as u16).map(|exe| vec![exe.id]).unwrap_or_default();
    }

    let participants = server.peers.iter().filter(|peer| peer.in_game).count();
    let count = match mode {
        2 if participants % 2 == 0 => participants / 2,
        3 if participants % 2 == 0 => participants / 2,
        3 if participants % 3 == 0 => participants * 2 / 3,
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
    let participants = server.peers.iter().filter(|peer| peer.in_game).count();
    let eliminated = tournament_eliminate_ids(mode, server).len();
    let min_to_continue = cfg().states.lobby_misc.min_players_required.max(2) as usize;
    participants.saturating_sub(eliminated) < min_to_continue
}

/// Whether an elimination bracket is running that still has another round left
/// in it after the one being played now.
///
/// This is the whole of "is a tournament going on right now", and every stage
/// asks it directly rather than being told: results_init skips the Results
/// screen for the waiting room while it is true, lobby_init keeps that same
/// waiting room out while it is true, and tournament_advance eliminates
/// somebody while it is true. There is no flag to set, consume or keep in sync,
/// and no order the calls have to happen in.
pub fn tournament_continues(server: &Server) -> bool {
    let mode = cfg().states.gameplay.tournament_mode;
    mode != 0 && !tournament_ending(mode, server)
}

/// Fallback for the round hand-off below: a client that never sends the
/// CLIENT_LOBBY_PLAYERS_REQUEST it is waited on (dropped packet, dead
/// connection) must not stall the bracket. Everyone answering normally starts
/// the vote the moment the last of them is served.
const TOURNAMENT_WAIT_SECS: f64 = 5.0;

/// Round end under tournament_mode.
///
/// Once the bracket is over -- and whenever tournament_mode is off entirely --
/// this is just an ordinary Lobby entry and nothing else: the waiting room is
/// let in by lobby_init itself, and the last round's loser stays too.
///
/// While it isn't over, this eliminates the round's worst player(s) and runs
/// the next round. That needs no separate Lobby code path either: it is the
/// same lobby_init as always, which asks tournament_continues itself and so
/// leaves the waiting room alone. All this adds on top is lobby.tournament_owed
/// -- the count of players who still have to fetch their Lobby roster before
/// the vote can start, which the Lobby's own CLIENT_LOBBY_PLAYERS_REQUEST
/// handler counts down and acts on.
fn tournament_advance(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !tournament_continues(server) {
        lobby_init(server, outbox);
        lobby_broadcast_init(server, outbox);
        return;
    }

    let mode = cfg().states.gameplay.tournament_mode;
    let eliminate = tournament_eliminate_ids(mode, server);
    log::info!(
        "[Server {}] Tournament: eliminating {} of {} in-game",
        server.id, eliminate.len(),
        server.peers.iter().filter(|peer| peer.in_game).count(),
    );

    for &id in &eliminate {
        let nick = server.find_peer(id).map(|peer| peer.nickname.clone()).unwrap_or_default();
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

    // The ordinary Lobby entry, run while the eliminated players are still on
    // the roster: lobby_init asks tournament_continues itself, and that answer
    // has to be about the round that just finished, not about what is left
    // after this elimination. They are already queued for disconnect above, so
    // the Lobby packets it sends never reach them.
    lobby_init(server, outbox);
    lobby_broadcast_init(server, outbox);

    server.peers.retain(|peer| !eliminate.contains(&peer.id));

    // The client throws its whole lobby away and asks for the roster back the
    // moment it enters its lobby room (SERVER_GAME_BACK_TO_LOBBY, which
    // lobby_broadcast_init just sent), and it stops accepting that roster once
    // the vote starts. So the vote starts when the last one has been served, in
    // the CLIENT_LOBBY_PLAYERS_REQUEST handler itself (lobby.rs) -- all that is
    // set here is how many that is, plus the fallback for a client that never
    // asks. The players just eliminated are gone from the roster and the
    // waiting room was never in_game, so neither owes us anything.
    server.lobby.tournament_owed = server.peers.iter().filter(|peer| peer.in_game).count() as u16;
    server.lobby.tournament_wait = TOURNAMENT_WAIT_SECS * 60.0;
}

/// Non-exe in-game survivors, ranked best-to-worst by the same criteria as the
/// results screen (see compare_primary/compare_secondary).
fn ranked_survivor_ids(server: &Server) -> Vec<u16> {
    let mut entries: Vec<(u16, ResEntry)> = server.peers.iter()
        .filter(|peer| peer.in_game && peer.id as i32 != server.game.exe)
        .map(|peer| (peer.id, ResEntry::from_peer(peer, server.game.exe)))
        .collect();
    entries.sort_by(|left, right| compare_primary(&left.1, &right.1));
    entries.sort_by(|left, right| compare_secondary(&left.1, &right.1));
    entries.into_iter().map(|(id, _)| id).collect()
}

pub fn results_state_tick(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    server.results.countdown -= 1.0;
    if server.results.countdown <= 0.0 {
        results_uninit(server, outbox);
    }
}

pub fn results_state_handle(
    player_id: u16,
    packet: &mut Packet,
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
) {
    let packet_type = match packet.packet_type() {
        Some(found) => found,
        None => return,
    };

    match packet_type {
        PacketType::CLIENT_RESULTS_REQUEST => {
            send_results_to(player_id, server, outbox);
        }

        PacketType::CLIENT_CHAT_MESSAGE => {
            if !server.chat_rate_allow(player_id) { return; } // anti-flood
            let in_game = server.find_peer(player_id).map(|peer| peer.in_game).unwrap_or(true);
            if in_game {
                return;
            }
            packet.pos = 2;
            let _pid = packet.read_u16();
            let msg = match packet.read_str() {
                Some(text) => text,
                None => return,
            };
            if let Some(peer) = server.find_peer_mut(player_id) {
                peer.timeout = 0.0;
            }

            let nick = server.find_peer(player_id).map(|peer| peer.nickname.clone()).unwrap_or_default();
            log::info!("[{}] (id {}): {}", crate::colors::colorize(&nick), player_id, msg);

            if let Some(cmd) = crate::terminal::cmd::parse_cmd(&msg) {
                let op = server.find_peer(player_id).map(|peer| peer.op).unwrap_or(0);
                if crate::states::lobby::exec_cmd_pub(&cmd, player_id, op, server, outbox) {
                    return;
                }
            }
        }

        PacketType::CLIENT_PING => {
            let pkt = Packet::new(PacketType::SERVER_PONG);
            outbox.push(OutboxMsg::SendTo(player_id, pkt.data().to_vec(), false));
        }

        _ => {}
    }
}

fn send_results_to(player_id: u16, server: &Server, outbox: &mut Vec<OutboxMsg>) {
    let mut entries: Vec<ResEntry> = Vec::new();

    for peer in server.peers.iter() {
        if !peer.in_game {
            continue;
        }
        entries.push(ResEntry::from_peer(peer, server.game.exe));
    }
    for peer in server.game.left.iter() {
        if !peer.in_game {
            continue;
        }
        entries.push(ResEntry::from_peer(peer, server.game.exe));
    }

    entries.sort_by(compare_primary);
    entries.sort_by(compare_secondary);

    let viewer_op = server.peers.iter().find(|peer| peer.id == player_id).map(|peer| peer.op).unwrap_or(0);
    let cfg = cfg();
    for entry in &entries {
        let pkt = build_result_packet(entry, server, cfg, viewer_op);
        outbox.push(OutboxMsg::SendTo(player_id, pkt.data().to_vec(), true));
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
    fn from_peer(peer: &PeerData, exe_id: i32) -> Self {
        Self {
            nickname: peer.nickname.clone(),
            flags: peer.plr.flags,
            exe_char: peer.exe_char as u8,
            surv_char: peer.surv_char as u8,
            is_exe: peer.id as i32 == exe_id,
            has_quit: peer.plr.flags & plrflags::LEFT != 0,
            danger_time: peer.plr.stats.danger_time,
            camp_time: peer.plr.stats.camp_time,
            brain_damage: peer.plr.stats.brain_damage,
            rings: peer.plr.stats.rings,
            kills: peer.plr.stats.kills,
            damage: peer.plr.stats.damage,
            damage_taken: peer.plr.stats.damage_taken,
            stun_time: peer.plr.stats.stun_time,
            stuns: peer.plr.stats.stuns,
            hp_restored: peer.plr.stats.hp_restored,
            survive_time: peer.plr.stats.survive_time,
        }
    }
}

fn compare_primary(left: &ResEntry, right: &ResEntry) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;

    let left_is_killer = left.flags & plrflags::KILLER != 0;
    let right_is_killer = right.flags & plrflags::KILLER != 0;
    if left_is_killer || right_is_killer {
        let order = (right_is_killer as i8) - (left_is_killer as i8);
        if order != 0 { return order.cmp(&0); }
    }

    let left_has_left = left.flags & plrflags::LEFT != 0;
    let right_has_left = right.flags & plrflags::LEFT != 0;
    if left_has_left || right_has_left {
        let order = (left_has_left as i8) - (right_has_left as i8);
        if order != 0 { return order.cmp(&0); }
    }

    let left_demonized = left.flags & plrflags::DEMONIZED != 0;
    let right_demonized = right.flags & plrflags::DEMONIZED != 0;
    if left_demonized || right_demonized {
        let order = (left_demonized as i8) - (right_demonized as i8);
        if order != 0 { return order.cmp(&0); }
    }

    let left_dead = left.flags & plrflags::DEAD != 0;
    let right_dead = right.flags & plrflags::DEAD != 0;
    if left_dead || right_dead {
        let order = (left_dead as i8) - (right_dead as i8);
        if order != 0 { return order.cmp(&0); }
    }

    Equal
}

fn compare_secondary(left: &ResEntry, right: &ResEntry) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;

    let left_is_killer = left.flags & plrflags::KILLER != 0;
    let right_is_killer = right.flags & plrflags::KILLER != 0;
    if left_is_killer || right_is_killer {
        let order = (right_is_killer as i8) - (left_is_killer as i8);
        if order != 0 { return order.cmp(&0); }
    }

    let left_has_left = left.flags & plrflags::LEFT != 0;
    let right_has_left = right.flags & plrflags::LEFT != 0;
    if left_has_left || right_has_left { return Equal; }

    let left_demonized = left.flags & plrflags::DEMONIZED != 0;
    let right_demonized = right.flags & plrflags::DEMONIZED != 0;
    if left_demonized || right_demonized { return Equal; }

    let left_dead = left.flags & plrflags::DEAD != 0;
    let right_dead = right.flags & plrflags::DEAD != 0;
    if left_dead == right_dead {
        let diff = right.danger_time - left.danger_time;
        if diff > 0.0 { return Less; }
        if diff < 0.0 { return Greater; }
    }

    Equal
}

fn build_result_packet(entry: &ResEntry, server: &Server, cfg: &crate::config::Config, viewer_op: u8) -> Packet {
    let mut plr_type = RES_ALIVE;
    if entry.flags & plrflags::ESCAPED != 0 { plr_type = RES_ESCAPED; }
    if entry.flags & plrflags::DEMONIZED != 0 { plr_type = RES_DEMONIZED; }
    else if entry.flags & plrflags::DEAD != 0 { plr_type = RES_DEAD; }
    else if entry.is_exe { plr_type = RES_EXE; }

    let nickname = if cfg.states.lobby_misc.anonymous_mode && viewer_op < 1 {
        "anonymous".to_string()
    } else {
        let postfix = if cfg.states.results_misc.pride {
            if entry.brain_damage && (entry.flags & plrflags::ESCAPED != 0) {
                let idx = (entry.nickname.len() + entry.rings as usize) % SHAMES_1.len();
                SHAMES_1[idx]
            } else if entry.camp_time >= 30.0 * 60.0 {
                let idx = (entry.nickname.len() + entry.kills as usize) % SHAMES_2.len();
                SHAMES_2[idx]
            } else {
                ""
            }
        } else {
            ""
        };
        if postfix.is_empty() {
            entry.nickname.clone()
        } else {
            format!("{} {}", entry.nickname, postfix)
        }
    };

    let char_byte = if entry.exe_char != 255 { entry.exe_char } else { entry.surv_char };

    let mut pkt = Packet::new(PacketType::SERVER_RESULTS_DATA);
    let _ = pkt.write_str(&nickname);
    let _ = pkt.write_u8(char_byte);
    let _ = pkt.write_u8(server.game.ending as u8);
    let _ = pkt.write_u16(server.game.time_sec);
    let _ = pkt.write_u8(if entry.has_quit { 1 } else { 0 });
    let _ = pkt.write_u8(plr_type);
    let _ = pkt.write_u16(entry.rings);
    let _ = pkt.write_u16(entry.kills);
    let _ = pkt.write_u16(entry.damage);
    let _ = pkt.write_u16(entry.damage_taken);
    let _ = pkt.write_u16(entry.stun_time);
    let _ = pkt.write_u16(entry.stuns);
    let _ = pkt.write_u16(entry.hp_restored);
    write_double_compat(&mut pkt, entry.survive_time);
    write_double_compat(&mut pkt, entry.danger_time);
    pkt
}

fn write_double_compat(pkt: &mut Packet, value: f64) {
    let _ = pkt.write_f32(value as f32);
    let _ = pkt.write_u32(0);
}
