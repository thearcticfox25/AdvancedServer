use crate::config::cfg;
use crate::maps::MAP_LIST;
use crate::packet::{Packet, PacketType};
use crate::player::{flags as plrflags, Player};
use crate::server::{
    BigRingState, DisconnectReason, Ending, GameState, OutboxMsg, PeerData, Server,
};
use crate::states::results::results_init;
use crate::status::with_status;

use rand::seq::SliceRandom;
use std::time::Instant;
use crate::states::seconds;
use crate::core::config::ticks_per_second;

pub const PLRSTATE_ALIVE: u8     = 0;
pub const PLRSTATE_ESCAPED: u8   = 2;
pub const PLRSTATE_EXE: u8       = 3;

pub fn game_init(exe: i32, map: i8, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    log::debug!("Entering Game state...");
    server.state = GameState::Game;

    server.game.exe          = exe;
    server.game.map          = map;
    server.game.started      = false;
    server.game.sudden_death = false;
    server.game.left.clear();
    server.game.next_entity_id        = 0;
    server.game.bring_state  = BigRingState::None;
    server.game.bring_loc    = rand::random::<u8>();
    server.game.end          = 0.0;
    server.game.elapsed      = 0.0;

    server.game.time         = seconds(1.0);
    server.game.time_sec     = 0;

    server.game.start_timeout = seconds(cfg().states.gameplay.waiting_timeout as f64);

    for peer in server.peers.iter_mut() {
        if !peer.in_game { continue; }
        peer.plr = Player::default();
        if peer.id as i32 == exe {
            peer.plr.flags |= plrflags::KILLER;
            peer.plr.state  = PLRSTATE_EXE;
        } else {
            peer.plr.state = PLRSTATE_ALIVE;
        }
        peer.ready = false;
    }

    let ambush_pct = cfg().states.gameplay.gmcycle.ambush_force_demonization_percentage_on_start;
    if ambush_pct > 0 {
        ambush_force_demonize_on_start(exe, ambush_pct, server, outbox);
    }

    let pkt = Packet::new(PacketType::SERVER_LOBBY_GAME_START);
    outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));
    server.game.round = None;
    crate::states::round::start(server, outbox);
    // Anyone who was watching the round before this one, or asked to watch while this
    // one was still being voted for, watches this one from its first tick.
    crate::states::spectate::enter_round(server, outbox);

    announce_roster_to_waiting_room(exe, server, outbox);

    log::info!("{} Game starting: map={} exe={}", crate::server::lobby_tag(server.id), map, exe);
}

/// Tells everyone still in the waiting room who is now playing the round, so
/// their waiting-room player list matches the game that just started.
fn announce_roster_to_waiting_room(exe: i32, server: &Server, outbox: &mut Vec<OutboxMsg>) {
    use crate::states::waiting_room as wr;

    let waiting_ids: Vec<u16> = server.peers.iter()
        .filter(|peer| !peer.in_game)
        .map(|peer| peer.id)
        .collect();

    for waiter_id in waiting_ids {
        let anonymous = wr::anonymous_for(waiter_id, server);
        for peer in server.peers.iter().filter(|peer| peer.id != waiter_id) {
            let pkt = wr::waiting_player_info(peer, peer.in_game, exe, anonymous);
            outbox.push(OutboxMsg::SendTo(waiter_id, pkt.data().to_vec(), true));
        }
    }
}

pub fn game_checkstart(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if server.game.started { return; }

    let ingame_count = server.peers.iter().filter(|peer| peer.in_game).count();
    let ready_count  = server.peers.iter().filter(|peer| peer.in_game && peer.ready).count();

    if ready_count < ingame_count { return; }
    if ingame_count == 0 { return; }

    let pkt = Packet::new(PacketType::SERVER_GAME_PLAYERS_READY);
    outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));

    crate::maps::map_init(server, server.game.map as usize);

    let cfg = cfg();
    if !cfg.states.gameplay.banana.disable_timer {
        let cap = cfg.states.gameplay.gametimers_ceiling as u16;
        if server.game.time_sec > cap {
            server.game.time_sec = cap;
            server.game.time = seconds(cap as f64);
        }
    }

    server.game.started = true;

    log::info!("{} Game players ready, map initialised. time_sec={}", crate::server::lobby_tag(server.id), server.game.time_sec);
}

pub fn game_uninit(server: &mut Server, show_results: bool, outbox: &mut Vec<OutboxMsg>) {
    server.game.round = None;

    if show_results {
        results_init(server, outbox);
    } else {
        crate::states::lobby::lobby_init(server, outbox);
        crate::states::lobby::lobby_broadcast_init(server, outbox);
    }
}

pub fn game_end(server: &mut Server, ending: Ending, achievement: bool, outbox: &mut Vec<OutboxMsg>) {
    if server.game.end > 0.0 { return; }

    log::info!("Ending is {:?}", ending);

    let cfg = cfg();
    server.game.ending = ending;
    server.game.end    = seconds(cfg.states.gameplay.ending_timer as f64);

    with_status(|status| {
        match ending {
            Ending::ExeWin   => status.exe_win_rounds  += 1,
            Ending::SurvWin  => status.surv_win_rounds += 1,
            Ending::TimeOver => status.draw_rounds      += 1,
        }
    });
    crate::status::save_status();

    let pkt_type = match ending {
        Ending::ExeWin   => PacketType::SERVER_GAME_EXE_WINS,
        Ending::SurvWin  => PacketType::SERVER_GAME_SURVIVOR_WIN,
        Ending::TimeOver => PacketType::SERVER_GAME_TIME_OVER,
    };
    let mut pkt = Packet::new(pkt_type);
    let _ = pkt.write_u8(if achievement { 1 } else { 0 });
    outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));
    let sim_ending = match ending {
        Ending::ExeWin   => crate::core::game::RoundEnding::ExeWon,
        Ending::SurvWin  => crate::core::game::RoundEnding::SurvivorsEscaped,
        Ending::TimeOver => crate::core::game::RoundEnding::TimeOver,
    };
    crate::states::round::end(server, sim_ending);
}

pub fn game_bigring(server: &mut Server, state: BigRingState, outbox: &mut Vec<OutboxMsg>) {
    if server.game.bring_state == state { return; }
    server.game.bring_state = state;
    log::info!("Big ring is {}!", if state == BigRingState::Activated { "activated" } else { "deactivated" });

    let mut pkt = Packet::new(PacketType::SERVER_GAME_SPAWN_RING);
    let _ = pkt.write_u8(if state == BigRingState::Activated { 1 } else { 0 });
    let _ = pkt.write_u8(server.game.bring_loc);
    outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));
}

fn game_demonize(player_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();
    // Every EXE of the round is left out, not only the first.
    let survivor_side = |peer: &&PeerData| peer.in_game && !server.lobby.is_exe(peer.id);

    let players   = server.peers.iter().filter(survivor_side).count() as f64;
    let demonized = server.peers.iter()
        .filter(|peer| survivor_side(peer) && peer.plr.flags & plrflags::DEMONIZED != 0)
        .count() as f64;

    let should_demonize = players * (cfg.states.gameplay.demonization_percentage as f64 / 100.0) > demonized;

    let nick = server.find_peer(player_id).map(|peer| peer.nickname.clone()).unwrap_or_default();

    let mut pkt = Packet::new(PacketType::SERVER_GAME_DEATHTIMER_END);
    if should_demonize {
        if let Some(peer) = server.find_peer_mut(player_id) {
            peer.plr.flags &= !plrflags::DEAD;
            peer.plr.flags |=  plrflags::DEMONIZED;
            peer.plr.stats.rings = 0;
        }
        with_status(|status| { status.total_demonised += 1; });
        let _ = pkt.write_u8(1);
        log::info!("{} (id {}) was demonized!", crate::colors::colorize(&nick), player_id);
    } else {
        if let Some(peer) = server.find_peer_mut(player_id) {
            peer.plr.flags |= plrflags::CANTREVIVE;
        }
        with_status(|status| { status.total_died += 1; });
        let _ = pkt.write_u8(0);
        log::info!("{} (id {}) died!", crate::colors::colorize(&nick), player_id);
    }
    outbox.push(OutboxMsg::SendTo(player_id, pkt.data().to_vec(), true));
    crate::states::round::change_player(server, player_id, |player, cfg, events| player.death_timer_end(cfg, should_demonize, events));
}

/// Ambush: instantly demonizes `percentage`% of non-exe in-game players at round start,
/// bypassing the wound/death-timer flow entirely.
fn ambush_force_demonize_on_start(exe: i32, percentage: u8, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let mut candidates: Vec<u16> = server.peers.iter()
        .filter(|peer| peer.in_game && peer.id as i32 != exe)
        .map(|peer| peer.id)
        .collect();
    candidates.shuffle(&mut rand::thread_rng());

    let count = ((candidates.len() as f64) * (percentage as f64 / 100.0)).round() as usize;
    for &id in candidates.iter().take(count) {
        if let Some(peer) = server.find_peer_mut(id) {
            peer.plr.flags |= plrflags::DEMONIZED;
            peer.plr.stats.rings = 0;
        }
        let mut pkt = Packet::new(PacketType::SERVER_GAME_DEATHTIMER_END);
        let _ = pkt.write_u8(1);
        outbox.push(OutboxMsg::SendTo(id, pkt.data().to_vec(), true));
    }
    if count > 0 {
        with_status(|status| { status.total_demonised += count as u32; });
        log::info!("{} Ambush: {} player(s) demonized on start", crate::server::lobby_tag(server.id), count);
    }
}

pub fn game_state_join(player_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    use crate::states::waiting_room as wr;
    let exe_id = server.game.exe;
    let map    = server.game.map;
    wr::send_waiting_player_list(player_id, exe_id, server, outbox);
    wr::announce_waiter_joined(player_id, server, outbox);
    wr::send_waiting_room_greeting(player_id, Some(map), server, outbox);
}

pub fn game_state_left(player_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let mut pkt = Packet::new(PacketType::SERVER_PLAYER_LEFT);
    let _ = pkt.write_u16(player_id);
    outbox.push(OutboxMsg::BroadcastEx(pkt.data().to_vec(), true, player_id));

    if server.game.end > 0.0 { return; }
    if !is_ingame(player_id, server) { return; }

    let min_to_continue = cfg().states.lobby_misc.min_players_required.max(1) as usize;
    let remaining = server.peers.iter().filter(|peer| peer.in_game && peer.id != player_id).count();
    if remaining < min_to_continue {
        game_uninit(server, false, outbox);
        return;
    }

    // With several EXE (states.lobby_misc.character_selection.exe_count) the survivors
    // win only once the last of them is gone.
    let exe_left = server.lobby.is_exe(player_id);
    let exe_stays = server.peers.iter().any(|peer| peer.in_game && peer.id != player_id && server.lobby.is_exe(peer.id));
    if exe_left && exe_stays {
        hand_over_main_exe(player_id, server);
    }
    // Their character leaves the level with them (the original destroyed its puppet):
    // nobody can hit it any more, and a departed EXE hits nobody.
    crate::states::round::change_player(server, player_id, |player, _, _| player.removed = true);

    if !server.game.started {
        if exe_left && !exe_stays {
            with_status(|status| { status.exe_crashed_rounds += 1; });
            game_uninit(server, false, outbox);
        } else {
            game_checkstart(server, outbox);
        }
        return;
    }

    if let Some(peer) = server.find_peer(player_id) {
        let mut left_pd = PeerData::new(peer.id, peer.ip.clone());
        left_pd.plr     = peer.plr.clone();
        left_pd.plr.flags |= plrflags::LEFT;
        left_pd.in_game    = peer.in_game;
        left_pd.surv_char  = peer.surv_char;
        left_pd.exe_char   = peer.exe_char;
        server.game.left.push(left_pd);
    }

    if exe_left && !exe_stays {
        game_end(server, Ending::SurvWin, server.game.elapsed >= seconds(60.0), outbox);
        return;
    }

    game_state_check(server, Some(player_id), outbox);
}

/// The round rules of the original protocol know one EXE (server.game.exe). When that
/// one leaves while another EXE plays on, the next one drawn takes its place, so the
/// rules that ask for "the" EXE keep finding one in the round.
fn hand_over_main_exe(leaving: u16, server: &mut Server) {
    if server.game.exe != leaving as i32 {
        return;
    }
    let next = server.lobby.exes.iter().copied().find(|&id| id != leaving && server.peers.iter().any(|peer| peer.id == id && peer.in_game));
    if let Some(next) = next {
        server.game.exe = next as i32;
    }
}

/// Ends the round once no survivor is left playing. `leaving` is a player who is
/// disconnecting right now: still in the list of peers, but no longer playing.
pub fn game_state_check(server: &mut Server, leaving: Option<u16>, outbox: &mut Vec<OutboxMsg>) {
    if server.game.end > 0.0 { return; }

    let mut escaped  = 0u32;
    let mut dead     = 0u32;
    let mut exes     = 0u32;
    let mut total_ingame = 0i32;

    for peer in &server.peers {
        if !peer.in_game || Some(peer.id) == leaving { continue; }
        total_ingame += 1;
        if peer.plr.flags & plrflags::ESCAPED   != 0 { escaped += 1; }
        if peer.plr.flags & (plrflags::DEAD | plrflags::DEMONIZED) != 0 { dead += 1; }
        // Every EXE of the round (states.lobby_misc.character_selection.exe_count), not only the first.
        if server.lobby.is_exe(peer.id) { exes += 1; }
    }

    let alive_survivors = total_ingame - (exes as i32 + dead as i32 + escaped as i32);

    if alive_survivors <= 0 {
        if escaped > 0 {
            game_end(server, Ending::SurvWin, true, outbox);
        } else {
            game_end(server, Ending::ExeWin, true, outbox);
        }
    }
}

pub fn game_state_handle(
    player_id: u16,
    packet: &mut Packet,
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
) {
    // A spectator watches the round from outside it, so none of the round logic below
    // is theirs to run (states::spectate). The chat still is: they never left the
    // waiting room's, and unlike the players they have it at hand while they watch.
    if crate::states::spectate::plays_no_part(player_id, server) {
        if packet.packet_type() == Some(PacketType::CLIENT_CHAT_MESSAGE) {
            handle_chat(player_id, packet, server, outbox);
        }
        return;
    }

    if !server.game.started {
        if let Some(peer) = server.find_peer_mut(player_id) {
            if !peer.ready {
                peer.ready = true;
                peer.plr.last_packet = Some(Instant::now());
            }
        }
        // The buttons of the round count from the first tick a client plays.
        if packet.packet_type() == Some(PacketType::CLIENT_ROUND_INPUT) {
            crate::states::round::receive_input(player_id, packet, server);
        }
        game_checkstart(server, outbox);
        return;
    }

    let packet_type = match packet.packet_type() {
        Some(found) => found,
        None        => return,
    };

    // The server decides everything about the round: a client only says which buttons
    // it holds. Nothing it could claim (positions, hits, rings, map objects) is taken.
    match packet_type {
        PacketType::CLIENT_ROUND_INPUT => crate::states::round::receive_input(player_id, packet, server),
        PacketType::CLIENT_ROUND_PAUSE => crate::states::round::receive_pause(player_id, packet, server),
        PacketType::CLIENT_PLAYER_PALETTE | PacketType::CLIENT_PET_PALETTE => relay_to_others(player_id, packet, outbox, true),
        PacketType::CLIENT_PING => handle_ping(player_id, server, outbox),
        PacketType::CLIENT_CHAT_MESSAGE => handle_chat(player_id, packet, server, outbox),
        _ => {}
    }
}

/// The chat during a round: the pause menu's, which the original has no room for, and
/// the waiting room's, from the players who are watching the round from outside it.
fn handle_chat(player_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !server.chat_rate_allow(player_id) {
        return;
    }
    if server.find_peer(player_id).is_some_and(|peer| peer.in_game) {
        relay_to_others(player_id, packet, outbox, true);
        return;
    }
    packet.pos = 2;
    let _sender = packet.read_u16();
    let Some(msg) = packet.read_str() else { return };
    let nick = server.find_peer(player_id).map(|peer| peer.nickname.clone()).unwrap_or_default();
    log::info!("{} (id {}): {}", crate::colors::colorize(&nick), player_id, msg);
    crate::states::waiting_room::handle_waiter_chat(player_id, &msg, server, outbox);
}

/// Whether this player is taking part in the round now.
fn is_ingame(player_id: u16, server: &Server) -> bool {
    server.find_peer(player_id).is_some_and(|peer| peer.in_game)
}

/// Forwards the packet, unchanged and from its first byte, to everyone in the
/// round except the peer that sent it.
fn relay_to_others(player_id: u16, packet: &mut Packet, outbox: &mut Vec<OutboxMsg>, reliable: bool) {
    packet.pos = 0;
    outbox.push(OutboxMsg::BroadcastEx(packet.data().to_vec(), reliable, player_id));
}

/// Answers a client ping and tells everyone else what that client's round-trip
/// time is, so their HUD can show it.
fn handle_ping(player_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let rtt = server.find_peer(player_id).map(|peer| peer.rtt).unwrap_or(1).max(1);

    let mut pong = Packet::new(PacketType::SERVER_PONG);
    let _ = pong.write_u16(rtt);
    outbox.push(OutboxMsg::SendTo(player_id, pong.data().to_vec(), false));

    let mut game_ping_pkt = Packet::new(PacketType::SERVER_GAME_PING);
    let _ = game_ping_pkt.write_u16(player_id);
    let _ = game_ping_pkt.write_u16(rtt);
    outbox.push(OutboxMsg::BroadcastEx(game_ping_pkt.data().to_vec(), false, player_id));

    if let Some(peer) = server.find_peer_mut(player_id) {
        peer.plr.ping_last = rtt;
    }
}

/// CLIENT_PLAYER_DEATH_STATE of the original, and a death in an authoritative round:
/// a survivor went down (or got back up), which starts or stops their death timer.
pub fn player_death_state(player_id: u16, dead: bool, revival_times: u8, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if server.game.end > 0.0 { return; }

    if !is_ingame(player_id, server) { return; }

    if server.lobby.is_exe(player_id) { return; }

    if server.find_peer(player_id).map(|peer| peer.plr.flags & plrflags::DEMONIZED != 0).unwrap_or(false) { return; }

    let dead = u8::from(dead);

    let mut death_pkt = Packet::new(PacketType::SERVER_PLAYER_DEATH_STATE);
    let _ = death_pkt.write_u16(player_id);
    let _ = death_pkt.write_u8(dead);
    let _ = death_pkt.write_u8(revival_times);
    outbox.push(OutboxMsg::Broadcast(death_pkt.data().to_vec(), true));

    broadcast_revival_status(player_id, false, outbox);

    if dead != 0 {
        let already_down = server.find_peer(player_id)
            .map(|peer| peer.plr.flags & (plrflags::DEAD | plrflags::ESCAPED) != 0)
            .unwrap_or(true);
        if already_down { return; }

        if let Some(peer) = server.find_peer_mut(player_id) {
            peer.plr.flags |= plrflags::DEAD;
        }

        let cfg = cfg();

        let revived_flag = server.find_peer(player_id)
            .map(|peer| peer.plr.flags & plrflags::REVIVED != 0)
            .unwrap_or(false);
        let in_sudden_death = server.game.sudden_death;

        if revived_flag || in_sudden_death {
            game_demonize(player_id, server, outbox);
        } else {
            let respawn_time = cfg.states.gameplay.respawn_time as u16;
            let sd_timer     = cfg.states.gameplay.sudden_death_timer as u16;
            let time_to_sd   = ticks_until_sudden_death(
                server.game.time_sec, sd_timer, cfg.states.gameplay.banana.disable_timer,
            );
            let death_timer_sec = if cfg.states.gameplay.sync_sudden_death_timers && time_to_sd < respawn_time {
                time_to_sd
            } else {
                respawn_time
            } as u8;

            if let Some(peer) = server.find_peer_mut(player_id) {
                peer.plr.death_timer_sec = death_timer_sec;
                // death_timer (the 0..60 sub-second accumulator) starts at 0 at the
                // moment of death, independent from death_timer_sec (the whole-second
                // count): tick_players' >=60 rollover check must only fire after a
                // full real second has actually accumulated.
                peer.plr.death_timer     = 0.0;
            }

            let v_pos    = server.find_peer(player_id).map(|peer| peer.plr.pos).unwrap_or((0.0, 0.0));
            let exe_near = camp_penalty_at(v_pos, server).exe_near;

            broadcast_death_timer(player_id, death_timer_sec, exe_near, outbox);

            with_status(|status| { status.total_stuns += 1; });
        }
    } else {
        if server.find_peer(player_id).map(|peer| peer.plr.death_timer_sec == 0).unwrap_or(true) { return; }
        if let Some(peer) = server.find_peer_mut(player_id) {
            peer.plr.flags       &= !plrflags::DEAD;
            peer.plr.death_timer  = 0.0;
        }
    }

    game_state_check(server, None, outbox);
}

/// CLIENT_PLAYER_ESCAPED of the original, and a survivor reaching the big ring in an
/// authoritative round. Returns whether the survivor escaped now.
pub fn player_escaped(player_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) -> bool {
    if !is_ingame(player_id, server) { return false; }
    if server.lobby.is_exe(player_id) { return false; }

    let flags = server.find_peer(player_id).map(|peer| peer.plr.flags).unwrap_or(0);
    if flags & (plrflags::DEAD | plrflags::DEMONIZED | plrflags::ESCAPED) != 0 { return false; }

    if let Some(peer) = server.find_peer_mut(player_id) {
        peer.plr.flags |= plrflags::ESCAPED;
        peer.plr.state  = PLRSTATE_ESCAPED;
    }

    let escaped_pkt = Packet::new(PacketType::SERVER_PLAYER_ESCAPED);
    outbox.push(OutboxMsg::SendTo(player_id, escaped_pkt.data().to_vec(), true));

    let mut announce_pkt = Packet::new(PacketType::SERVER_GAME_PLAYER_ESCAPED);
    let _ = announce_pkt.write_u16(player_id);
    outbox.push(OutboxMsg::BroadcastEx(announce_pkt.data().to_vec(), true, player_id));

    with_status(|status| { status.total_escaped += 1; });

    game_state_check(server, None, outbox);
    true
}


/// CLIENT_REVIVAL_PROGRESS of the original, and an authoritative round's reviving:
/// `player_id` looks down on the downed `target_id` with `rings` rings beyond three.
pub fn revival_progress(player_id: u16, target_id: u16, rings: u8, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if server.game.end > 0.0 { return; }

    let target_dead = server.find_peer(target_id)
        .map(|peer| peer.plr.flags & plrflags::DEAD != 0)
        .unwrap_or(false);
    if !target_dead { return; }

    let cantrevive = server.find_peer(target_id)
        .map(|peer| peer.plr.flags & plrflags::CANTREVIVE != 0)
        .unwrap_or(false);

    if cantrevive {
        if let Some(target) = server.find_peer_mut(target_id) {
            target.plr.death_timer_sec = 0;
            target.plr.death_timer     = 0.0;
        }
        broadcast_revival_status(target_id, false, outbox);
        return;
    }

    let was_zero = server.find_peer(target_id).map(|peer| peer.plr.revival <= 0.0).unwrap_or(true);
    if was_zero {
        broadcast_revival_status(target_id, true, outbox);
    }

    let increment = 0.015 + 0.004 * rings as f64;
    if let Some(target) = server.find_peer_mut(target_id) {
        target.plr.revival += increment;
    }

    let revival = server.find_peer(target_id).map(|peer| peer.plr.revival).unwrap_or(0.0);

    if revival < 1.0 {
        let mut progress_pkt = Packet::new(PacketType::SERVER_REVIVAL_PROGRESS);
        let _ = progress_pkt.write_u16(target_id);
        let _ = progress_pkt.write_f64(revival);
        outbox.push(OutboxMsg::Broadcast(progress_pkt.data().to_vec(), false));

        if let Some(target) = server.find_peer_mut(target_id) {
            let already_listed = target.plr.revival_init.iter()
                .any(|&reviver| reviver == player_id as i32);
            if !already_listed {
                for slot in &mut target.plr.revival_init {
                    if *slot == -1 { *slot = player_id as i32; break; }
                }
            }
        }
    } else {
        if let Some(target) = server.find_peer_mut(target_id) {
            target.plr.stats.rings = 0;
            target.plr.flags &= !plrflags::DEAD;
            target.plr.flags |=  plrflags::REVIVED;
            target.plr.revival = 0.0;
        }

        broadcast_revival_status(target_id, false, outbox);

        let revived_pkt = Packet::new(PacketType::SERVER_REVIVAL_REVIVED);
        outbox.push(OutboxMsg::SendTo(target_id, revived_pkt.data().to_vec(), true));
        crate::states::round::change_player(server, target_id, |player, cfg, _| player.revive(cfg));

        if let Some(peer) = server.find_peer(target_id) {
            log::info!("{} (id {}) was revived!", crate::colors::colorize(&peer.nickname), target_id);
        }

        let revivers: Vec<i32> = server.find_peer(target_id)
            .map(|peer| peer.plr.revival_init.to_vec())
            .unwrap_or_default();
        for reviver_id in revivers {
            if reviver_id == -1 { break; }
            log::debug!("Removed rings from {}", reviver_id);
            let ring_cost_pkt = Packet::new(PacketType::SERVER_REVIVAL_RINGSUB);
            outbox.push(OutboxMsg::SendTo(reviver_id as u16, ring_cost_pkt.data().to_vec(), true));
            crate::states::round::change_player(server, reviver_id as u16, |player, _, _| player.rings = 0);
        }

        if let Some(target) = server.find_peer_mut(target_id) {
            target.plr.revival_init = [-1; 5];
        }
    }
}






pub fn game_state_tick(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();

    // An authoritative round simulates while waiting too: players fall onto the ground.
    crate::states::round::tick(server, outbox);

    // Still waiting on every client to report ready: nothing about the round has
    // begun yet, so only the join deadline ticks.
    if !server.game.started {
        tick_start_timeout(server, outbox);
        return;
    }

    // The round is over and the ending animation is playing out.
    if server.game.end > 0.0 {
        server.game.end -= 1.0;
        if server.game.end <= 0.0 {
            game_uninit(server, cfg.states.results_misc.enabled, outbox);
        }
        return;
    }

    server.game.elapsed += 1.0;
    if !tick_round_clock(server, outbox) {
        return;
    }
    trigger_sudden_death(server, outbox);

    tick_players(server, outbox);
    tick_bigring(server, outbox);
    tick_map(server, outbox);
}

/// Counts down the deadline for the slowest client to send its ready packet, and
/// kicks whoever is still holding the round up when it expires.
fn tick_start_timeout(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    server.game.start_timeout -= 1.0;
    if server.game.start_timeout > 0.0 {
        return;
    }
    let unready = server.peers.iter().find(|peer| peer.in_game && !peer.ready).map(|peer| peer.id);
    if let Some(id) = unready {
        log::warn!(
            "{} Waiting for players took too long, kicking unready player id={}",
            crate::server::lobby_tag(server.id), id
        );
        outbox.push(OutboxMsg::Disconnect(id, DisconnectReason::PacketsNotRecv as u32));
    }
}

/// Advances the round clock by one tick and, on every whole second, spawns the
/// periodic ring and syncs the time to the clients.
///
/// Returns false when the clock ran out and ended the round, meaning the rest of
/// this tick must not run.
fn tick_round_clock(server: &mut Server, outbox: &mut Vec<OutboxMsg>) -> bool {
    let cfg = cfg();
    let disable_timer = cfg.states.gameplay.banana.disable_timer;

    // With disable_timer on, the clock counts up (elapsed) and never ends the
    // round; gametimers_ceiling deliberately does not apply either. That mode is
    // a reimplementation of the "No Timer" mod (README credits), whose whole
    // point is that no time-based ending exists: a ceiling forcing game_end here
    // would hold the round hostage to an artificial cutoff (and skew the duration
    // Results displays) exactly contrary to it. game_init's own ceiling clamp is
    // gated on !disable_timer the same way.
    let new_sec = if disable_timer {
        (server.game.elapsed / ticks_per_second()) as u16
    } else {
        server.game.time -= 1.0;
        (server.game.time / ticks_per_second()).max(0.0) as u16
    };
    if new_sec == server.game.time_sec {
        return true;
    }
    server.game.time_sec = new_sec;

    let mut pkt = Packet::new(PacketType::SERVER_GAME_TIME_SYNC);
    let _ = pkt.write_u16(server.game.time_sec * 60);
    outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));

    if server.game.time_sec != 0 || disable_timer {
        return true;
    }

    // Time is up. Anyone who made it out means the survivors won on points;
    // nobody out means the round simply expired.
    let anyone_escaped = server.peers.iter()
        .filter(|peer| peer.in_game && peer.id as i32 != server.game.exe)
        .any(|peer| peer.plr.flags & plrflags::ESCAPED != 0);
    if anyone_escaped {
        game_end(server, Ending::SurvWin, false, outbox);
    } else {
        game_end(server, Ending::TimeOver, false, outbox);
    }
    false
}

/// Starts sudden death the first tick the clock passes its threshold, demonizing
/// everyone already waiting to be revived, longest-waiting first.
fn trigger_sudden_death(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();
    let sd_timer = cfg.states.gameplay.sudden_death_timer as u16;

    // The clock counts up with disable_timer on and down without it, so the
    // threshold comparison flips with it.
    let past_threshold = if cfg.states.gameplay.banana.disable_timer {
        server.game.time_sec > sd_timer
    } else {
        server.game.time_sec <= sd_timer
    };
    if server.game.sudden_death || !past_threshold {
        return;
    }

    server.game.sudden_death = true;
    with_status(|status| { status.timeouts += 1; });
    let pkt = Packet::new(PacketType::SERVER_GAME_SUDDEN_DEATH);
    outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));

    let mut dead_players: Vec<(u16, f64)> = server.peers.iter()
        .filter(|peer| peer.in_game && peer.plr.flags & plrflags::DEAD != 0)
        .map(|peer| (peer.id, peer.plr.death_timer_sec as f64 + peer.plr.death_timer / ticks_per_second()))
        .collect();
    dead_players.sort_by(|left, right| left.1.partial_cmp(&right.1).unwrap_or(std::cmp::Ordering::Equal));

    log::debug!("Demonization order:");
    for (id, secs) in &dead_players {
        log::debug!("{}: {}", id, secs);
    }
    for (id, _) in dead_players {
        game_demonize(id, server, outbox);
    }
}

/// Runs the current map's own per-tick script, if it has one.
fn tick_map(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let map_idx = server.game.map as usize;
    if map_idx >= MAP_LIST.len() {
        return;
    }
    let mut map_outbox = Vec::new();
    (MAP_LIST[map_idx].tick)(server, &mut map_outbox);
    outbox.append(&mut map_outbox);
}

/// Ticks remaining before sudden death actually triggers (see
/// game_state_tick's past_sudden_death_threshold). The two directions aren't
/// symmetric: normal mode's threshold is inclusive (time_sec <= sd_timer)
/// so counting down reaches the trigger the instant time_sec hits sd_timer,
/// but disable_timer's is exclusive (time_sec > sd_timer) -- time_sec can
/// sit exactly *at* sd_timer and still need one more increment before it
/// fires. A plain abs_diff misses that +1 and reports 0 a full tick before
/// disable_timer's real trigger.
fn ticks_until_sudden_death(time_sec: u16, sd_timer: u16, disable_timer: bool) -> u16 {
    if disable_timer {
        sd_timer.saturating_sub(time_sec) + 1
    } else {
        time_sec.saturating_sub(sd_timer)
    }
}

/// One tick of everything that happens to players regardless of any packet
/// they send.
fn tick_players(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    tick_peer_timers(server, outbox);
    tick_death_timers(server, outbox);
}

/// Advances every in-game player's own per-tick bookkeeping: idle/AFK deadlines,
/// ability cooldowns, the rolling ping average, and the Results-screen timers.
fn tick_peer_timers(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg     = cfg();
    let delta   = server.delta;
    let lobby   = &server.lobby;

    // Where every EXE of the round stands, for the survivors' danger time.
    let exe_positions: Vec<(f32, f32)> = server.peers.iter()
        .filter(|peer| peer.in_game && lobby.is_exe(peer.id))
        .map(|peer| peer.plr.pos)
        .collect();

    let mut to_disconnect: Vec<u16> = Vec::new();

    for peer in server.peers.iter_mut() {
        if !peer.in_game { continue; }

        if let Some(last) = peer.plr.last_packet {
            if last.elapsed().as_secs_f64() > 30.0 {
                to_disconnect.push(peer.id);
                continue;
            }
        }

        if delta < 2.5 {
            peer.plr.ping_timer += delta;
            peer.plr.ping_total += peer.plr.ping_last as f64 * delta;

            if peer.plr.ping_timer >= 20.0 * 60.0 {
                let avg = peer.plr.ping_total / peer.plr.ping_timer;
                let limit = cfg.server_config.pairing.ping_limit as f64;
                if limit > 0.0 && avg >= limit {
                    to_disconnect.push(peer.id);
                    peer.plr.ping_timer = 0.0;
                    peer.plr.ping_total = 0.0;
                    continue;
                }
                peer.plr.ping_timer = 0.0;
                peer.plr.ping_total = 0.0;
            }
        }

        if !lobby.is_exe(peer.id)
            && peer.plr.flags & plrflags::DEAD    == 0
            && peer.plr.flags & plrflags::ESCAPED == 0
            && peer.plr.flags & plrflags::DEMONIZED == 0
        {
            peer.plr.stats.survive_time += delta;

            let near_exe = exe_positions.iter().any(|&exe_pos| crate::player::vec2_dist(peer.plr.pos, exe_pos) < 300.0);
            if near_exe {
                peer.plr.stats.danger_time += delta;
            }
        }

        if peer.plr.flags & plrflags::DEAD != 0
            && peer.plr.flags & plrflags::CANTREVIVE == 0
            && peer.plr.revival > 0.0
        {
            peer.plr.revival -= 0.0025 * delta;
        }

        if peer.plr.flags & plrflags::ESCAPED == 0 {
            peer.plr.timeout += delta;
            if peer.plr.timeout >= 4.0 * 60.0 {
                to_disconnect.push(peer.id);
                continue;
            }
        }
    }

    for id in to_disconnect {
        outbox.push(OutboxMsg::Disconnect(id, DisconnectReason::AfkTimeout as u32));
    }
}

/// Counts down the death timer of every player waiting to be revived, and
/// demonizes the ones whose timer runs out. Once sudden death has started, every
/// such player is demonized at once instead.
fn tick_death_timers(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let waiting_for_revival: Vec<u16> = server.peers.iter()
        .filter(|peer| peer.in_game
                 && peer.plr.flags & plrflags::DEAD != 0
                 && peer.plr.flags & plrflags::CANTREVIVE == 0
                 && peer.plr.death_timer_sec > 0)
        .map(|peer| peer.id)
        .collect();

    for id in waiting_for_revival {
        // Sudden death is the point of no return: nobody gets revived any more.
        if server.game.sudden_death {
            game_demonize(id, server, outbox);
        } else {
            tick_one_death_timer(id, server, outbox);
        }
    }
}

/// How the exe (and demonized players) camping a downed survivor changes that
/// survivor's death timer.
struct CampPenalty {
    /// The exe is standing over them: the countdown freezes entirely.
    exe_near: bool,
    /// A demonized player is nearby (and the exe is not): the countdown runs at
    /// half speed.
    demonized_near: bool,
}

/// Looks for anyone camping the downed player at `pos`. The exe wins over a
/// demonized player: finding the exe stops the search and cancels the
/// half-speed penalty in favour of the full freeze.
fn camp_penalty_at(pos: (f32, f32), server: &Server) -> CampPenalty {
    let mut penalty = CampPenalty { exe_near: false, demonized_near: false };
    if !cfg().states.gameplay.exe_camp_penalty {
        return penalty;
    }

    const CAMP_RADIUS: f32 = 240.0;
    for other in server.peers.iter() {
        if !other.in_game { continue; }
        let is_exe       = server.lobby.is_exe(other.id);
        let is_demonized = other.plr.flags & plrflags::DEMONIZED != 0;
        if !is_exe && !is_demonized { continue; }
        if crate::player::vec2_dist(pos, other.plr.pos) > CAMP_RADIUS { continue; }

        if is_exe {
            penalty.demonized_near = false;
            penalty.exe_near = true;
            break;
        }
        penalty.demonized_near = true;
    }
    penalty
}

/// One tick of one downed player's death timer: apply the sudden-death ceiling,
/// count the whole second down when one has accumulated, and add this tick.
fn tick_one_death_timer(id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();
    let mut death_timer_sec = server.find_peer(id).map(|peer| peer.plr.death_timer_sec).unwrap_or(0);

    // Checked every tick rather than only on rollover, because demonized_near
    // changes the accumulation rate itself.
    let pos = server.find_peer(id).map(|peer| peer.plr.pos).unwrap_or((0.0, 0.0));
    let camp = camp_penalty_at(pos, server);

    let time_to_sd = ticks_until_sudden_death(
        server.game.time_sec,
        cfg.states.gameplay.sudden_death_timer as u16,
        cfg.states.gameplay.banana.disable_timer,
    );
    let sync_to_sd = cfg.states.gameplay.sync_sudden_death_timers;

    // Catch-up sync, every tick -- not gated on this player's own ~1s sub-tick
    // rollover below, and not on camp.exe_near either: this is the hard ceiling
    // sync_sudden_death_timers promises (death_timer_sec can never show more
    // time than is actually left), not just an anti-camp correction, so it must
    // apply regardless of each player's rollover phase against the global clock.
    // Server-internal bookkeeping only: it still broadcasts the same
    // SERVER_GAME_DEATHTIMER_TICK the client already expects.
    if sync_to_sd && time_to_sd < death_timer_sec as u16 {
        death_timer_sec = time_to_sd as u8;
        if let Some(peer) = server.find_peer_mut(id) {
            peer.plr.death_timer_sec = death_timer_sec;
        }
        if death_timer_sec == 0 {
            game_demonize(id, server, outbox);
            return;
        }
        broadcast_death_timer(id, death_timer_sec, camp.exe_near, outbox);
    }

    // Checks the accumulator carried over from previous ticks *before* adding
    // this tick's increment (added unconditionally at the end) -- not
    // accumulate-then-check. This keeps each dead player's rollover aligned to
    // when they actually died, instead of drifting by a tick against players who
    // died at a different phase.
    let accumulated = server.find_peer(id).map(|peer| peer.plr.death_timer).unwrap_or(0.0);
    if accumulated >= seconds(1.0) {
        if let Some(peer) = server.find_peer_mut(id) {
            peer.plr.death_timer = 0.0;
        }

        // Camping freezes the countdown, unless the sudden-death ceiling is
        // already pushing it down faster than the camper can hold it up.
        let counts_down = !camp.exe_near || (sync_to_sd && time_to_sd < death_timer_sec as u16);
        if counts_down {
            let remaining = death_timer_sec.saturating_sub(1);
            if let Some(peer) = server.find_peer_mut(id) {
                peer.plr.death_timer_sec = remaining;
            }
            if remaining == 0 {
                game_demonize(id, server, outbox);
                return;
            }
        }

        let shown = server.find_peer(id).map(|peer| peer.plr.death_timer_sec).unwrap_or(0);
        broadcast_death_timer(id, shown, camp.exe_near, outbox);
    }

    let increment = if camp.demonized_near { 0.5 } else { 1.0 };
    if let Some(peer) = server.find_peer_mut(id) {
        peer.plr.death_timer += increment;
    }
}

/// Tells everyone whether `player_id` is currently being revived, which is what
/// drives the revival ring drawn around them on every client.
fn broadcast_revival_status(player_id: u16, being_revived: bool, outbox: &mut Vec<OutboxMsg>) {
    let mut pkt = Packet::new(PacketType::SERVER_REVIVAL_STATUS);
    let _ = pkt.write_u8(if being_revived { 1 } else { 0 });
    let _ = pkt.write_u16(player_id);
    outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));
}

fn broadcast_death_timer(id: u16, seconds: u8, exe_near: bool, outbox: &mut Vec<OutboxMsg>) {
    let mut pkt = Packet::new(PacketType::SERVER_GAME_DEATHTIMER_TICK);
    let _ = pkt.write_u8(exe_near as u8);
    let _ = pkt.write_u16(id);
    let _ = pkt.write_u8(seconds);
    outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));
}

fn tick_bigring(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg        = cfg();
    if cfg.states.gameplay.banana.disable_timer || cfg.states.gameplay.gmcycle.overhell {
        return;
    }
    let ring_time  = cfg.states.gameplay.ring_appearance_timer as u16;
    let escape_time = cfg.states.gameplay.escape_time as u16;

    match server.game.bring_state {
        BigRingState::None => {
            if server.game.time_sec <= ring_time {
                game_bigring(server, BigRingState::Deactivated, outbox);
            }
        }
        BigRingState::Deactivated => {
            // rmz_checkstate: Ravine Mist's ring waits for enough shards.
            let may_open = server.game.round.as_ref().is_none_or(crate::states::round::big_ring_may_open);
            if server.game.time_sec <= escape_time && may_open {
                game_bigring(server, BigRingState::Activated, outbox);
            }
        }
        BigRingState::Activated => {}
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /// A started round with EXE `exes` and survivors `survivors`, everyone playing.
    fn round_with(exes: &[u16], survivors: &[u16]) -> Server {
        crate::config::set_test_config();
        crate::status::init_empty_status();
        let mut server = Server::new(0);
        server.state = GameState::Game;
        server.game.started = true;
        server.game.exe = exes[0] as i32;
        server.lobby.exes = exes.to_vec();
        for &id in exes.iter().chain(survivors) {
            let mut peer = PeerData::new(id, format!("10.0.0.{id}"));
            peer.in_game = true;
            server.peers.push(peer);
        }
        server
    }

    /// What the worker does when a player disconnects.
    fn disconnect(player_id: u16, server: &mut Server) {
        game_state_left(player_id, server, &mut Vec::new());
        server.peers.retain(|peer| peer.id != player_id);
    }

    #[test]
    fn survivors_win_only_once_every_exe_has_left() {
        let mut server = round_with(&[1, 2], &[3, 4]);
        disconnect(1, &mut server);
        assert_eq!(server.game.end, 0.0, "another EXE still plays");
        assert_eq!(server.game.exe, 2, "and the rules' EXE is now that one");

        disconnect(2, &mut server);
        assert!(server.game.end > 0.0, "the last EXE is gone");
        assert_eq!(server.game.ending, Ending::SurvWin);
    }

    #[test]
    fn the_last_survivor_leaving_ends_the_round_at_once() {
        let mut server = round_with(&[1], &[2, 3]);
        server.find_peer_mut(2).unwrap().plr.flags |= plrflags::DEAD;
        disconnect(3, &mut server);
        assert!(server.game.end > 0.0, "nobody is left to survive: the leaver is not counted as alive");
        assert_eq!(server.game.ending, Ending::ExeWin);
    }
}
