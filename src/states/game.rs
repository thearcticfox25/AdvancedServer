use crate::config::cfg;
use crate::entities::{Entity, EntityCtx};
use crate::maps::MAP_LIST;
use crate::packet::{Packet, PacketType};
use crate::anticheat::physics;
use crate::anticheat::zone;
use crate::player::{flags as plrflags, Player, vec2_dist};
use crate::server::{
    BigRingState, DisconnectReason, Ending, ExeChar, GameState, OutboxMsg, PeerData, Server,
    SurvChar,
};
use crate::states::results::results_init;
use crate::status::with_status;

use rand::SeedableRng;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use std::time::Instant;

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
    server.game.entities.clear();
    server.game.rings        = [false; 256];
    server.game.left.clear();
    server.game.next_entity_id        = 0;
    server.game.bring_state  = BigRingState::None;
    server.game.bring_loc    = rand::random::<u8>();
    server.game.end          = 0.0;
    server.game.elapsed      = 0.0;

    server.game.time         = 60.0;
    server.game.time_sec     = 0;

    server.game.start_timeout = cfg().states.gameplay.waiting_timeout as f64 * 60.0;

    for peer in server.peers.iter_mut() {
        if !peer.in_game { continue; }
        peer.plr = Player::default();
        peer.plr.mod_tool = peer.mod_tool;
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

    announce_roster_to_waiting_room(exe, server, outbox);

    log::info!("[Server {}] Game starting: map={} exe={}", server.id, map, exe);
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

    let map_idx = server.game.map as usize;
    if map_idx < MAP_LIST.len() {
        (MAP_LIST[map_idx].init)(server, outbox);
    } else {
        crate::maps::map_init(server, outbox);
    }

    let cfg = cfg();
    if !cfg.states.gameplay.banana.disable_timer {
        let cap = cfg.states.gameplay.gametimers_ceiling as u16;
        if server.game.time_sec > cap {
            server.game.time_sec = cap;
            server.game.time = cap as f64 * 60.0;
        }
    }

    server.game.started = true;

    log::info!("[Server {}] Game players ready, map initialised. time_sec={}", server.id, server.game.time_sec);
}

pub fn game_uninit(server: &mut Server, show_results: bool, outbox: &mut Vec<OutboxMsg>) {
    with_entity_op(server, outbox, |entities, ctx| {
        for entity in entities.iter_mut() {
            entity.on_uninit(ctx);
        }
    });
    server.game.entities.clear();

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
    server.game.end    = cfg.states.gameplay.ending_timer as f64 * 60.0;

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
    let exe_id = server.game.exe;

    let players   = server.peers.iter().filter(|peer| peer.in_game && peer.id as i32 != exe_id).count() as f64;
    let demonized = server.peers.iter()
        .filter(|peer| peer.in_game && peer.id as i32 != exe_id && peer.plr.flags & plrflags::DEMONIZED != 0)
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
        log::info!("[Server {}] Ambush: {} player(s) demonized on start", server.id, count);
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

    let map_idx = server.game.map as usize;
    if map_idx < MAP_LIST.len() {
        let mut map_outbox = Vec::new();
        (MAP_LIST[map_idx].left)(player_id, server, &mut map_outbox);
        outbox.append(&mut map_outbox);
    }

    let min_to_continue = cfg().states.lobby_misc.min_players_required.max(1) as usize;
    let remaining = server.peers.iter().filter(|peer| peer.in_game && peer.id != player_id).count();
    if remaining < min_to_continue {
        game_uninit(server, false, outbox);
        return;
    }

    if !server.game.started {
        if player_id as i32 == server.game.exe {
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

    if player_id as i32 == server.game.exe {
        game_end(server, Ending::SurvWin, server.game.elapsed >= 60.0 * 60.0, outbox);
        return;
    }

    game_state_check(server, outbox);
}

pub fn game_state_check(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if server.game.end > 0.0 { return; }

    let exe_id = server.game.exe;
    let mut escaped  = 0u32;
    let mut dead     = 0u32;
    let mut exes     = 0u32;

    for peer in &server.peers {
        if !peer.in_game { continue; }
        if peer.plr.flags & plrflags::ESCAPED   != 0 { escaped += 1; }
        if peer.plr.flags & (plrflags::DEAD | plrflags::DEMONIZED) != 0 { dead += 1; }
        if peer.id as i32 == exe_id { exes += 1; }
    }

    let total_ingame = server.ingame_count() as i32;
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
    // A spectator is standing in the map but takes no part in the round, so
    // none of the round logic below is theirs to run. See states::spectate.
    if crate::states::spectate::handle_packet(player_id, packet, server, outbox) { return; }

    if !server.game.started {
        if let Some(peer) = server.find_peer_mut(player_id) {
            if !peer.ready {
                peer.ready = true;
                peer.plr.last_packet = Some(Instant::now());
            }
        }
        game_checkstart(server, outbox);
        return;
    }

    let packet_type = match packet.packet_type() {
        Some(found) => found,
        None        => return,
    };

    match packet_type {
        PacketType::CLIENT_PLAYER_DATA => handle_player_data(player_id, packet, server, outbox),

        // Purely cosmetic client packets: the server has nothing to validate or
        // record in them, it only forwards them to everyone else in the round.
        PacketType::CLIENT_PLAYER_HURT
        | PacketType::CLIENT_RING_BROKE
        | PacketType::CLIENT_PLAYER_POTATER
        | PacketType::CLIENT_SPAWN_EFFECT
        | PacketType::CLIENT_SPRING_USE
        | PacketType::CLIENT_PLAYER_PALETTE => {
            if !is_ingame(player_id, server) { return; }
            relay_to_others(player_id, packet, outbox, true);
        }

        PacketType::CLIENT_PLAYER_DEATH_STATE => handle_player_death_state(player_id, packet, server, outbox),

        PacketType::CLIENT_PLAYER_ESCAPED => handle_player_escaped(player_id, packet, server, outbox),

        PacketType::CLIENT_RING_COLLECTED => handle_ring_collected(player_id, packet, server, outbox),

        PacketType::CLIENT_BRING_COLLECTED => handle_bring_collected(player_id, packet, server, outbox),

        PacketType::CLIENT_REVIVAL_PROGRESS => handle_revival(player_id, packet, server, outbox),

        PacketType::CLIENT_TPROJECTILE_STARTCHARGE => handle_tprojectile_startcharge(player_id, server),

        PacketType::CLIENT_TPROJECTILE => handle_tprojectile(player_id, packet, server, outbox),

        PacketType::CLIENT_TPROJECTILE_HIT => handle_tprojectile_hit(player_id, server, outbox),

        PacketType::CLIENT_CREAM_SPAWN_RINGS => handle_cream_rings(player_id, packet, server, outbox),

        PacketType::CLIENT_ETRACKER => handle_etracker(player_id, packet, server, outbox),

        PacketType::CLIENT_ETRACKER_ACTIVATED => handle_etracker_activated(player_id, packet, server, outbox),

        PacketType::CLIENT_ERECTOR_BRING_SPAWN => handle_erector_bring_spawn(player_id, packet, server, outbox),

        PacketType::CLIENT_EXELLER_SPAWN_CLONE => handle_exeller_spawn_clone(player_id, packet, server, outbox),

        PacketType::CLIENT_EXELLER_TELEPORT_CLONE => handle_exeller_teleport_clone(player_id, packet, server, outbox),

        PacketType::CLIENT_ERECTOR_BALLS => handle_erector_balls(player_id, packet, server, outbox),

        PacketType::CLIENT_MERCOIN_BONUS => handle_mercoin_bonus(player_id, packet, server, outbox),

        PacketType::CLIENT_STATS_REPORT => handle_stats_report(player_id, packet, server, outbox),

        PacketType::CLIENT_SOUND_EMIT => {
            if !cfg().states.gameplay.enable_sounds { return; }
            if !is_ingame(player_id, server) { return; }
            relay_to_others(player_id, packet, outbox, true);
        }

        // Pet palettes are passthrough (as in upstream): the palette validator only
        // knows the character tables, so it does not apply here.
        PacketType::CLIENT_PET_PALETTE => relay_to_others(player_id, packet, outbox, false),

        PacketType::CLIENT_PLAYER_HEAL_PART => handle_heal_part(player_id, packet, server, outbox),

        PacketType::CLIENT_PLAYER_HEAL => handle_heal(player_id, packet, server, outbox),

        PacketType::CLIENT_CHAT_MESSAGE => {
            if !server.chat_rate_allow(player_id) { return; } // anti-flood
            // Only the waiting room chats through this path; anyone actually
            // playing the round is handled by the client-side in-game chat.
            if !server.find_peer(player_id).map(|peer| !peer.in_game).unwrap_or(false) { return; }
            packet.pos = 2;
            let _sender = packet.read_u16();
            let msg = match packet.read_str() { Some(text) => text, None => return };
            let nick = server.find_peer(player_id).map(|peer| peer.nickname.clone()).unwrap_or_default();
            log::info!("{} (id {}): {}", crate::colors::colorize(&nick), player_id, msg);
            crate::states::waiting_room::handle_waiter_chat(player_id, &msg, server, outbox);
        }

        PacketType::CLIENT_PING => handle_ping(player_id, server, outbox),

        // Everything else is an entity or map packet, handled by the map's own
        // tcp_msg hook that runs for every packet just below.
        _ => {}
    }

    packet.pos = 0;
    let map_idx = server.game.map as usize;
    if map_idx < MAP_LIST.len() {
        let mut map_outbox = Vec::new();
        (MAP_LIST[map_idx].tcp_msg)(player_id, packet, server, &mut map_outbox);
        outbox.append(&mut map_outbox);
    }
}

/// Adds `amount` to a player's suspicion score and disconnects them once it
/// passes player_maximum_errors. Returns false when they were disconnected, so
/// the caller stops handling their packet.
fn player_add_error(
    player_id: u16,
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
    amount: u16,
) -> bool {
    let max_errors = cfg().server_config.pairing.player_maximum_errors;
    if let Some(peer) = server.find_peer_mut(player_id) {
        peer.plr.errors = peer.plr.errors.saturating_add(amount);
        log::debug!("{} error is now {}", player_id, peer.plr.errors);
        if peer.plr.errors >= max_errors {
            outbox.push(OutboxMsg::Disconnect(player_id, DisconnectReason::KickedByHost as u32));
            return false;
        }
    }
    true
}

fn player_check_zone(player_id: u16, map_id: i8, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !cfg().states.gameplay.anticheat.zone_anticheat { return; }
    if !server.game.started { return; }

    let (pos, ex_tp) = match server.find_peer(player_id) {
        Some(peer) => (peer.plr.pos, peer.plr.ex_teleport),
        None => return,
    };
    if pos == (0.0, 0.0) { return; }
    if ex_tp > 0 { return; }

    let (px, py) = (pos.0, pos.1);
    let error_amount: u16 = if map_id == 6 { 30 } else { 1000 };

    if zone::legacy_zone_anticheat(px, py, map_id) {
        log::debug!("{} is inside invalid area (legacy zone AC)", player_id);
        player_add_error(player_id, server, outbox, error_amount);
        return;
    }

    if zone::zone_anticheat(px, py, map_id) {
        log::debug!("{} is out of bounds (physics JSON zone AC)", player_id);
        player_add_error(player_id, server, outbox, error_amount);
    }
}

/// True when this peer is actually playing the round rather than watching it
/// from the waiting room. Almost every in-game packet handler starts here.
fn is_ingame(player_id: u16, server: &Server) -> bool {
    server.find_peer(player_id).map(|peer| peer.in_game).unwrap_or(false)
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
    if cfg().states.gameplay.anticheat.useless_anticheat.enable {
        let is_mod = server.find_peer(player_id).map(|peer| peer.plr.mod_tool).unwrap_or(false);
        if is_mod && server.game.time_sec <= 125 {
            return;
        }
    }

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

/// One CLIENT_PLAYER_DATA report, exactly as the client sent it.
///
/// Every field here is a *claim*, not a fact: the checks below decide whether
/// the server believes it before any of it reaches `PeerData::plr`.
struct PlayerReport {
    pos:          (f32, f32),
    vel:          (f32, f32),
    state:        u8,
    flags:        u8,
    is_exe:       bool,
    surv_char:    SurvChar,
    /// Survivor-only fields; zero for the exe, which does not report them.
    hp:           i8,
    revival:      u8,
    rings:        i16,
    tails_charge: u8,
}

/// Bits the client packs into the report's flag byte.
mod report_flags {
    pub const REDRING:   u8 = 1 << 3;
    pub const ATTACKING: u8 = 1 << 4;
    pub const INVIS:     u8 = 1 << 6;
}

impl PlayerReport {
    fn has(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }

    fn is_attacking(&self) -> bool {
        self.has(report_flags::ATTACKING)
    }

    /// The red-ring (or black-ring) effect disables every ability client-side, and
    /// the survivor announces it here. The ability handlers read it back off the
    /// peer to reject abilities used while it is active (Cheat-Engine bypass).
    fn under_red_ring(&self) -> bool {
        !self.is_exe && self.has(report_flags::REDRING)
    }
}

fn handle_player_data(player_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if packet.len < 10 { return; }
    if !is_ingame(player_id, server) { return; }

    let report = match read_player_report(player_id, packet, server) {
        Some(report) => report,
        None         => return,
    };

    // Each check returns false once it has already disconnected or corrected the
    // player, which means this report must not be acted on any further.
    if !accept_reported_rings(player_id, &report, server, outbox)    { return; }
    if !check_attack_rate(player_id, &report, server, outbox)        { return; }
    if !check_zero_trust(player_id, &report, server, outbox)         { return; }
    if !check_map_geometry(player_id, &report, server, outbox)       { return; }
    if !check_travel_distance(player_id, &report, server, outbox)    { return; }
    if !check_physics_correction(player_id, &report, server, outbox) { return; }

    store_player_report(player_id, &report, server);
    force_black_ring_contact(player_id, &report, server, outbox);

    if !quarantine_mod_tool(player_id, server, outbox) { return; }

    // Relayed verbatim from byte 2 on, with the sender's id spliced in front, so
    // every other client sees exactly the state its owner reported.
    let raw = packet.buf[2..packet.len].to_vec();
    let mut pkt = Packet::new(PacketType::CLIENT_PLAYER_DATA);
    let _ = pkt.write_u16(player_id);
    for &byte in &raw { let _ = pkt.write_u8(byte); }
    outbox.push(OutboxMsg::BroadcastEx(pkt.data().to_vec(), false, player_id));
}

/// Parses the report. Returns None if the packet is truncated anywhere the
/// server actually needs a value.
fn read_player_report(player_id: u16, packet: &mut Packet, server: &Server) -> Option<PlayerReport> {
    packet.pos = 2;
    let x     = packet.read_u16()?;
    let y     = packet.read_u16()?;
    let xspd  = packet.read_u16().map(physics::f16_to_f32).unwrap_or(0.0);
    let yspd  = packet.read_u16().map(physics::f16_to_f32).unwrap_or(0.0);
    let state = packet.read_u8()?;

    // Angle, sprite index and x-scale are pure presentation: read past them.
    let _angle  = packet.read_i16();
    let _index  = packet.read_u8();
    let _xscale = packet.read_i8();

    let is_exe    = player_id as i32 == server.game.exe;
    let surv_char = server.find_peer(player_id).map(|peer| peer.surv_char).unwrap_or(SurvChar::None);

    let mut report = PlayerReport {
        pos: (x as f32, y as f32),
        vel: (xspd, yspd),
        state,
        flags: 0,
        is_exe,
        surv_char,
        hp: 0,
        revival: 0,
        rings: 0,
        tails_charge: 0,
    };

    if is_exe {
        report.flags = packet.read_u8()?;
        return Some(report);
    }

    report.hp      = packet.read_i8()?;
    report.revival = packet.read_u8()?;
    report.rings   = packet.read_i16()?;
    report.flags   = packet.read_u8()?;

    // Only Tails reports a charge level, followed by a value the server ignores.
    if surv_char == SurvChar::Tails {
        report.tails_charge = packet.read_u8().unwrap_or(0);
        let _ = packet.read_i16();
    }

    Some(report)
}

/// Copies the reported ring count onto the peer and, with data_based_anticheat on,
/// rejects counts the game can never produce. Skipped entirely for a player the
/// server already considers dead or demonized, whose counts are server-owned.
fn accept_reported_rings(
    player_id: u16,
    report: &PlayerReport,
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
) -> bool {
    if report.is_exe { return true; }

    let peer_flags = server.find_peer(player_id).map(|peer| peer.plr.flags).unwrap_or(0);
    if peer_flags & (plrflags::DEAD | plrflags::DEMONIZED) != 0 {
        return true;
    }

    if let Some(peer) = server.find_peer_mut(player_id) {
        peer.plr.rings = report.rings;
    }

    let server_owns_counts =
        peer_flags & (plrflags::DEMONIZED | plrflags::REVIVED | plrflags::CANTREVIVE) != 0;
    if server_owns_counts || !cfg().states.gameplay.anticheat.data_based_anticheat {
        return true;
    }

    // Map 20 is the practice map, where the ring cap does not apply.
    let impossible_rings = report.rings < 0 || (server.game.map != 20 && report.rings >= 120);
    if impossible_rings || report.hp > 100 {
        outbox.push(OutboxMsg::Disconnect(player_id, DisconnectReason::Other as u32));
        return false;
    }
    true
}

/// Attacking is a held state, not an instant: a client that reports it for longer
/// than the character's attack can possibly last is holding it open artificially.
fn check_attack_rate(
    player_id: u16,
    report: &PlayerReport,
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
) -> bool {
    if server.game.end > 0.0 || !report.is_attacking() {
        if let Some(peer) = server.find_peer_mut(player_id) {
            peer.plr.attack_timer = 0.0;
        }
        return true;
    }

    let attack_timer = server.find_peer(player_id).map(|peer| peer.plr.attack_timer).unwrap_or(0.0);
    if attack_timer <= 0.0 {
        if let Some(peer) = server.find_peer_mut(player_id) {
            peer.plr.last_attack  = Some(Instant::now());
            peer.plr.attack_timer = 1.0;
        }
        return true;
    }

    let max_attack_ms = if !report.is_exe && report.surv_char == SurvChar::Eggman {
        3000.0f64
    } else {
        2000.0f64
    };
    let elapsed_ms = server.find_peer(player_id)
        .and_then(|peer| peer.plr.last_attack)
        .map(|last| last.elapsed().as_millis() as f64)
        .unwrap_or(0.0);

    let new_timer = attack_timer + elapsed_ms;
    if new_timer >= max_attack_ms {
        outbox.push(OutboxMsg::Disconnect(player_id, DisconnectReason::KickedByHost as u32));
        return false;
    }
    if let Some(peer) = server.find_peer_mut(player_id) {
        peer.plr.attack_timer = new_timer;
        peer.plr.last_attack  = Some(Instant::now());
    }
    true
}

/// Compares the report against what the server itself knows about this player:
/// an invisible exe cannot attack, revival counts only ever go up and only as far
/// as the server's own flags allow, and hp only goes up by as much as the player
/// has actually earned (hp_credit).
fn check_zero_trust(
    player_id: u16,
    report: &PlayerReport,
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
) -> bool {
    let cfg = cfg();
    if !cfg.states.gameplay.anticheat.zero_trust_anticheat || !server.game.started {
        return true;
    }
    if server.find_peer(player_id).map(|peer| peer.plr.mod_tool).unwrap_or(false) {
        return true;
    }

    let log_only   = cfg.states.gameplay.anticheat.zero_trust_log_only;
    let peer_flags = server.find_peer(player_id).map(|peer| peer.plr.flags).unwrap_or(0);
    let nick       = server.find_peer(player_id).map(|peer| peer.nickname.clone()).unwrap_or_default();

    if report.is_exe {
        if report.has(report_flags::INVIS) && report.is_attacking() {
            log::debug!(
                "[zero_trust] INVIS_ATTACK: player {}({}) is attacking while invisible",
                player_id, nick
            );
            if !log_only {
                outbox.push(OutboxMsg::Disconnect(player_id, DisconnectReason::Other as u32));
                return false;
            }
        }
        return true;
    }

    let prev_hp   = server.find_peer(player_id).map(|peer| peer.plr.hp       ).unwrap_or(-1);
    let hp_credit = server.find_peer(player_id).map(|peer| peer.plr.hp_credit).unwrap_or(0);

    let is_demonized = peer_flags & plrflags::DEMONIZED  != 0;
    let was_revived  = peer_flags & plrflags::REVIVED    != 0;
    let cant_revive  = peer_flags & plrflags::CANTREVIVE != 0;
    let server_revival_max: u8 = if is_demonized { 2 }
                            else if was_revived || cant_revive { 1 }
                            else { 0 };

    if report.revival > server_revival_max.saturating_add(1) {
        log::debug!(
            "[zero_trust] REVIVAL_EXCEED: player {}({}) claimed revival={} \
             but server max={} (flags={:#04x})",
            player_id, nick, report.revival, server_revival_max, peer_flags
        );
        outbox.push(OutboxMsg::Disconnect(player_id, DisconnectReason::Other as u32));
        return false;
    }

    let prev_revival = server.find_peer(player_id).map(|peer| peer.plr.revival_times).unwrap_or(0);
    if report.revival < prev_revival {
        log::debug!(
            "[zero_trust] REVIVAL_DEC: player {}({}) revival {}->{} (impossible)",
            player_id, nick, prev_revival, report.revival
        );
        outbox.push(OutboxMsg::Disconnect(player_id, DisconnectReason::Other as u32));
        return false;
    }

    if prev_hp >= 0 && !is_demonized && server_revival_max < 2 {
        let gain = (report.hp as i16) - (prev_hp as i16);
        if gain > hp_credit as i16 {
            log::debug!(
                "[zero_trust] HP_GAIN: player {}({}) hp {}->{} \
                 credit={} (unexplained gain={})",
                player_id, nick, prev_hp, report.hp, hp_credit, gain
            );
            if !log_only {
                outbox.push(OutboxMsg::Disconnect(player_id, DisconnectReason::Other as u32));
                return false;
            }
        }
    }

    if report.surv_char == SurvChar::Tails && report.tails_charge > 0 && report.vel.0.abs() > 1.5 {
        log::debug!(
            "[zero_trust] TAILS_MOVE_CHARGE: player {}({}) charge={} xspd={:.2}",
            player_id, nick, report.tails_charge, report.vel.0
        );
    }

    // Whatever hp the player did gain this report is now paid for.
    let gain = ((report.hp as i16) - (prev_hp as i16)).max(0) as i8;
    if let Some(peer) = server.find_peer_mut(player_id) {
        peer.plr.hp_credit = peer.plr.hp_credit.saturating_sub(gain).max(0);
    }
    true
}

/// Checks the reported position against the map geometry extracted into
/// MapPhysics.json: outside the room at all, or standing inside a solid.
fn check_map_geometry(
    player_id: u16,
    report: &PlayerReport,
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
) -> bool {
    let cfg = cfg();
    if !cfg.states.gameplay.anticheat.physics_anticheat { return true; }
    if !server.game.started || is_mod_tool(player_id, server) || report.pos == (0.0, 0.0) {
        return true;
    }

    let map_id = server.game.map;
    // The physics table is loaded once at startup and never changes, so this
    // borrow does not tie up `server`.
    let map_physics = match physics::table().get(map_id) {
        Some(mp) => mp,
        None     => return true,
    };

    let log_only = cfg.states.gameplay.anticheat.physics_anticheat_log_only;
    let nick     = server.find_peer(player_id).map(|peer| peer.nickname.clone()).unwrap_or_default();
    let map_name = MAP_LIST.get(map_id as usize).map(|map| map.name).unwrap_or("?");
    let (px, py) = report.pos;

    if map_physics.out_of_bounds(px, py) {
        log::debug!(
            "[physics_ac] OOB: player {}({}) at ({:.0},{:.0}) map=\"{}\" room={}x{}",
            player_id, nick, px, py, map_name, map_physics.width, map_physics.height
        );
        if !log_only && !player_add_error(player_id, server, outbox, 300) {
            return false;
        }
    }

    if let Some(solid) = map_physics.solid_violation(px, py) {
        log::debug!(
            "[physics_ac] SOLID: player {}({}) at ({:.0},{:.0}) inside solid \
             ({:.0},{:.0} {}x{}) map=\"{}\"",
            player_id, nick, px, py, solid.x, solid.y, solid.width, solid.height, map_name
        );
        if !log_only && !player_add_error(player_id, server, outbox, 600) {
            return false;
        }
    }
    true
}

/// Hard cap on how far a player may move between two reports. Legitimate warps
/// announce themselves by setting ex_teleport, which spends one report each tick.
fn check_travel_distance(
    player_id: u16,
    report: &PlayerReport,
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
) -> bool {
    if !cfg().states.gameplay.anticheat.distance_anticheat { return true; }

    let old_pos = server.find_peer(player_id).map(|peer| peer.plr.pos).unwrap_or((0.0, 0.0));
    let ex_tp   = server.find_peer(player_id).map(|peer| peer.plr.ex_teleport).unwrap_or(0);

    if !server.game.started || is_mod_tool(player_id, server) || old_pos == (0.0, 0.0) {
        return true;
    }

    if ex_tp > 0 {
        if let Some(peer) = server.find_peer_mut(player_id) {
            peer.plr.ex_teleport -= 1;
        }
        return true;
    }

    let dist = vec2_dist(old_pos, report.pos);
    // Maps whose own scripts teleport the player further than this in one tick.
    if dist > 700.0 && !matches!(server.game.map, 6 | 8 | 13 | 15) {
        outbox.push(OutboxMsg::Disconnect(player_id, DisconnectReason::Other as u32));
        return false;
    }
    if dist > 60.0 && !player_add_error(player_id, server, outbox, 500) {
        return false;
    }
    true
}

/// Reachable-set check (see anticheat::physics::check_correction): a report that
/// lands outside the box the game's own speed clamps allow gets the player
/// snapped back to their last accepted position instead of being disconnected.
fn check_physics_correction(
    player_id: u16,
    report: &PlayerReport,
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
) -> bool {
    let cfg = cfg();
    if !cfg.states.gameplay.anticheat.physics_correction { return true; }
    if !server.game.started || is_mod_tool(player_id, server) { return true; }

    let prev_pos = server.find_peer(player_id).map(|peer| peer.plr.pos     ).unwrap_or((0.0, 0.0));
    let good_pos = server.find_peer(player_id).map(|peer| peer.plr.good_pos).unwrap_or((0.0, 0.0));

    // Nothing to compare against yet: this is the player's first report.
    if prev_pos == (0.0, 0.0) {
        if let Some(peer) = server.find_peer_mut(player_id) {
            peer.plr.good_pos = report.pos;
        }
        return true;
    }

    let prev_vel   = server.find_peer(player_id).map(|peer| peer.plr.vel).unwrap_or((0.0, 0.0));
    let elapsed_ms = server.find_peer(player_id)
        .and_then(|peer| peer.plr.last_packet)
        .map(|last| last.elapsed().as_millis() as f32)
        .unwrap_or(physics::MS_PER_TICK);

    let log_only = cfg.states.gameplay.anticheat.physics_correction_log_only;
    let nick     = server.find_peer(player_id).map(|peer| peer.nickname.clone()).unwrap_or_default();
    let map_id   = server.game.map;
    let map_name = MAP_LIST.get(map_id as usize).map(|map| map.name).unwrap_or("?");

    let (verdict, deviation) =
        physics::check_correction(prev_pos, prev_vel, report.pos, report.vel, elapsed_ms);

    match verdict {
        physics::CorrectionVerdict::VelocityOverflow => {
            log::debug!(
                "[physics_corr] VEL_OVERFLOW: player {}({}) vel=({:.2},{:.2}) \
                 max={:.1} map=\"{}\"",
                player_id, nick, report.vel.0, report.vel.1, deviation, map_name
            );
            if !log_only {
                send_backtrack(player_id, good_pos, outbox, false);
                return false;
            }
        }
        physics::CorrectionVerdict::PositionMismatch => {
            // Skip maps that move the player via direct x/y writes (warps),
            // which legitimately exceed the per-tick box: Priceless Freedom
            // lifts (10), Majin Forest room-wrap (13), Echidna Ruins
            // ziplines (19).
            if !matches!(map_id, 10 | 13 | 19) {
                log::debug!(
                    "[physics_corr] POS_MISMATCH: player {}({}) at ({:.0},{:.0}) \
                     prev=({:.0},{:.0}) vel=({:.2},{:.2}) dev={:.1}px \
                     elapsed={:.0}ms map=\"{}\"",
                    player_id, nick,
                    report.pos.0, report.pos.1,
                    prev_pos.0, prev_pos.1,
                    report.vel.0, report.vel.1, deviation, elapsed_ms,
                    map_name
                );
                if !log_only {
                    send_backtrack(player_id, good_pos, outbox, false);
                    return false;
                }
            }
            if let Some(peer) = server.find_peer_mut(player_id) {
                peer.plr.good_pos = report.pos;
            }
        }
        physics::CorrectionVerdict::Ok => {
            if let Some(peer) = server.find_peer_mut(player_id) {
                peer.plr.good_pos = report.pos;
            }
        }
    }
    true
}

/// Writes the accepted report onto the peer. From here on this is what the
/// server itself believes about the player.
fn store_player_report(player_id: u16, report: &PlayerReport, server: &mut Server) {
    let is_attacking = report.is_attacking();
    let red_ring     = report.under_red_ring();

    if let Some(peer) = server.find_peer_mut(player_id) {
        // First real position after a spawn or a warp: grant a short teleport
        // grace period so the distance check does not fire on the jump itself.
        if peer.plr.pos == (0.0, 0.0) && report.pos != (0.0, 0.0) {
            peer.plr.ex_teleport = peer.plr.ex_teleport.max(10);
        }
        peer.plr.pos          = report.pos;
        peer.plr.vel          = report.vel;
        peer.plr.state        = report.state;
        peer.plr.last_packet  = Some(Instant::now());
        peer.plr.timeout      = 0.0;
        peer.plr.is_attacking = is_attacking;
        peer.plr.red_ring     = red_ring;
        if !report.is_exe {
            peer.plr.hp            = report.hp;
            peer.plr.revival_times = report.revival;
            peer.plr.tails_charge  = report.tails_charge;
        }
    }
}

/// An honest survivor standing inside an erector black ring takes 20 damage
/// (net_state_game.SERVER_BRING_COLLECTED). Cheats either never send
/// CLIENT_BRING_COLLECTED or patch out the damage. The server knows where the
/// erector put its rings, so it forces the collection when a survivor is clearly
/// inside one it never reported. (Map-placed rings arrive with a sentinel
/// position -> deferred to the geometry-extraction work; verifying the 20 hp
/// actually landed is the same hp-authority problem as the exe-hit case, also
/// deferred.)
fn force_black_ring_contact(
    player_id: u16,
    report: &PlayerReport,
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
) {
    let cfg = cfg();
    if !cfg.states.gameplay.anticheat.zero_trust_anticheat { return; }
    if !server.game.started || is_mod_tool(player_id, server) || report.is_exe { return; }
    if report.hp <= 0 || report.revival >= 2 { return; }

    let (px, py) = report.pos;
    // Ring box [ring_x,ring_x+30] by [ring_y,ring_y+30] shrunk 6 px against the player box
    // (x +-12, y-18..y+14) - deliberately conservative, so an AABB-vs-mask edge
    // skim never costs 20 hp.
    let touched_ring = server.game.entities.iter()
        .filter_map(|entity| entity.bring_hit_pos().map(|ring_pos| (entity.id(), ring_pos)))
        .find(|&(_, (ring_x, ring_y))| {
            px + 12.0 > ring_x + 6.0 && px - 12.0 < ring_x + 24.0 &&
            py + 14.0 > ring_y + 6.0 && py - 18.0 < ring_y + 24.0
        })
        .map(|(entity_id, _)| entity_id);

    let ring_entity_id = match touched_ring { Some(id) => id, None => return };

    let nick = server.find_peer(player_id).map(|peer| peer.nickname.clone()).unwrap_or_default();
    log::debug!(
        "[zero_trust] BRING_PHASE: player {}({}) inside black ring {} without collecting",
        player_id, nick, ring_entity_id
    );
    if cfg.states.gameplay.anticheat.zero_trust_log_only { return; }

    game_despawn(server, outbox, ring_entity_id);
    let pkt = Packet::new(PacketType::SERVER_BRING_COLLECTED);
    outbox.push(OutboxMsg::SendTo(player_id, pkt.data().to_vec(), true));
}

/// useless_anticheat's answer to a detected mod-tool client: after a minute it
/// pins the player to wherever they were standing and drags them back whenever
/// they wander off, rather than disconnecting them outright.
///
/// Returns false while the player is pinned, so their report is not relayed.
fn quarantine_mod_tool(player_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) -> bool {
    const TICKS_UNTIL_PINNED: u32 = 60;

    if !is_mod_tool(player_id, server) { return true; }
    if !cfg().states.gameplay.anticheat.useless_anticheat.enable { return true; }

    let timer = server.find_peer(player_id).map(|peer| peer.plr.mod_tool_timer).unwrap_or(0);
    if timer > TICKS_UNTIL_PINNED {
        let cur_pos   = server.find_peer(player_id).map(|peer| peer.plr.pos      ).unwrap_or((0.0, 0.0));
        let start_pos = server.find_peer(player_id).map(|peer| peer.plr.start_pos).unwrap_or((0.0, 0.0));
        let dist = vec2_dist(cur_pos, start_pos);
        if dist >= 300.0 {
            outbox.push(OutboxMsg::Disconnect(player_id, DisconnectReason::Other as u32));
        } else if dist > 240.0 {
            send_backtrack(player_id, start_pos, outbox, true);
        }
        return false;
    }

    if let Some(peer) = server.find_peer_mut(player_id) {
        peer.plr.mod_tool_timer += 1;
    }
    if timer + 1 != TICKS_UNTIL_PINNED { return true; }

    // The moment of pinning: remember the spot and let every client play the
    // effect on this player.
    let cur_pos = server.find_peer(player_id).map(|peer| peer.plr.pos).unwrap_or((0.0, 0.0));
    if let Some(peer) = server.find_peer_mut(player_id) {
        peer.plr.start_pos = cur_pos;
    }
    let mut pkt = Packet::new(PacketType::SERVER_FELLA);
    let _ = pkt.write_u16(player_id);
    let _ = pkt.write_f32(cur_pos.0);
    let _ = pkt.write_f32(cur_pos.1);
    outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));
    false
}

/// Whether this peer was flagged as running a modified client (see anticheat::auth).
fn is_mod_tool(player_id: u16, server: &Server) -> bool {
    server.find_peer(player_id).map(|peer| peer.plr.mod_tool).unwrap_or(false)
}

/// Tells one client to snap back to `pos`.
fn send_backtrack(player_id: u16, pos: (f32, f32), outbox: &mut Vec<OutboxMsg>, reliable: bool) {
    let mut pkt = Packet::new(PacketType::SERVER_PLAYER_BACKTRACK);
    let _ = pkt.write_f32(pos.0);
    let _ = pkt.write_f32(pos.1);
    outbox.push(OutboxMsg::SendTo(player_id, pkt.data().to_vec(), reliable));
}

fn handle_player_death_state(player_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if server.game.end > 0.0 { return; }

    if !is_ingame(player_id, server) { return; }

    if player_id as i32 == server.game.exe { return; }

    if server.find_peer(player_id).map(|peer| peer.plr.flags & plrflags::DEMONIZED != 0).unwrap_or(false) { return; }

    packet.pos = 2;
    let dead          = match packet.read_u8() { Some(value) => value, None => return };
    let revival_times = match packet.read_u8() { Some(value) => value, None => return };

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

            let exe_id = server.game.exe;
            let exe_pos = server.find_peer(exe_id as u16).map(|peer| peer.plr.pos).unwrap_or((0.0, 0.0));
            let v_pos   = server.find_peer(player_id).map(|peer| peer.plr.pos).unwrap_or((0.0, 0.0));
            let dist    = crate::player::vec2_dist(v_pos, exe_pos);
            let exe_near = (dist <= 240.0 && cfg.states.gameplay.exe_camp_penalty) as u8;

            broadcast_death_timer(player_id, death_timer_sec, exe_near != 0, outbox);

            with_status(|status| { status.total_stuns += 1; });
        }
    } else {
        if server.find_peer(player_id).map(|peer| peer.plr.death_timer_sec == 0).unwrap_or(true) { return; }
        if let Some(peer) = server.find_peer_mut(player_id) {
            peer.plr.flags       &= !plrflags::DEAD;
            peer.plr.death_timer  = 0.0;
        }
    }

    game_state_check(server, outbox);
}

fn handle_player_escaped(player_id: u16, _packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !is_ingame(player_id, server) { return; }
    if player_id as i32 == server.game.exe { return; }

    if cfg().states.gameplay.anticheat.useless_anticheat.enable {
        if server.find_peer(player_id).map(|peer| peer.plr.mod_tool).unwrap_or(false) {
            outbox.push(OutboxMsg::Disconnect(player_id, DisconnectReason::ServerTimeout as u32));
            return;
        }
    }

    let flags = server.find_peer(player_id).map(|peer| peer.plr.flags).unwrap_or(0);
    if flags & (plrflags::DEAD | plrflags::DEMONIZED | plrflags::ESCAPED) != 0 { return; }

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

    game_state_check(server, outbox);
}

fn handle_ring_collected(player_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !is_ingame(player_id, server) { return; }

    packet.pos = 2;
    let ring_id = match packet.read_u8()  { Some(value) => value, None => return };
    let entity_id     = match packet.read_u16() { Some(value) => value, None => return };

    let red = server.game.entities.iter()
        .find(|entity| entity.id() == entity_id)
        .map(|entity| entity.is_red())
        .unwrap_or(false);

    let despawned = game_find_entity(server, entity_id).is_some();
    if despawned {
        game_despawn(server, outbox, entity_id);
    } else {
        return;
    }

    if !red {
        if let Some(peer) = server.find_peer_mut(player_id) {
            peer.plr.rings = peer.plr.rings.saturating_add(1);
            peer.plr.stats.rings += 1;
        }
    }

    let has_rings = server.find_peer(player_id).map(|peer| peer.plr.rings > 0).unwrap_or(false);
    let mut pkt = Packet::new(PacketType::SERVER_RING_COLLECTED);
    let _ = pkt.write_u8(ring_id);
    let _ = pkt.write_u16(entity_id);
    let _ = pkt.write_u8(red as u8);
    let _ = pkt.write_u8(if has_rings { 1 } else { 0 });
    outbox.push(OutboxMsg::SendTo(player_id, pkt.data().to_vec(), true));
}

fn handle_bring_collected(player_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !is_ingame(player_id, server) { return; }
    if player_id as i32 == server.game.exe { return; }

    packet.pos = 2;
    let entity_id = match packet.read_u16() { Some(value) => value, None => return };

    if game_find_entity(server, entity_id).is_some() {
        game_despawn(server, outbox, entity_id);
        let pkt = Packet::new(PacketType::SERVER_BRING_COLLECTED);
        outbox.push(OutboxMsg::SendTo(player_id, pkt.data().to_vec(), true));
    }
}

fn handle_revival(player_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if server.game.end > 0.0 { return; }

    packet.pos = 2;
    let target_id   = match packet.read_u16() { Some(value) => value, None => return };
    let rings = match packet.read_u8()  { Some(value) => value, None => return };

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
        }

        if let Some(target) = server.find_peer_mut(target_id) {
            target.plr.revival_init = [-1; 5];
        }
    }
}

fn handle_heal_part(player_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !is_ingame(player_id, server) { return; }

    packet.pos = 2;
    let _x    = match packet.read_u16() { Some(value) => value, None => return };
    let _y    = match packet.read_u16() { Some(value) => value, None => return };
    let rings = match packet.read_u16() { Some(value) => value, None => return };

    let cfg = cfg();
    if cfg.states.gameplay.anticheat.data_based_anticheat {
        if rings < 10 {
            outbox.push(OutboxMsg::Disconnect(player_id, DisconnectReason::Other as u32));
            return;
        }
        if rings >= 140 && server.game.map != 20 {
            outbox.push(OutboxMsg::Disconnect(player_id, DisconnectReason::Other as u32));
            return;
        }
    }

    if let Some(peer) = server.find_peer_mut(player_id) {
        peer.plr.heal_rings = peer.plr.rings as u16;
    }

    relay_to_others(player_id, packet, outbox, true);
}

fn handle_heal(player_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !is_ingame(player_id, server) { return; }

    packet.pos = 2;
    let _id   = packet.read_u16();
    let rings = match packet.read_u16() { Some(value) => value, None => return };

    let cfg = cfg();
    if cfg.states.gameplay.anticheat.data_based_anticheat {
        if rings < 10 {
            outbox.push(OutboxMsg::Disconnect(player_id, DisconnectReason::Other as u32));
            return;
        }
        if rings >= 140 && server.game.map != 20 {
            outbox.push(OutboxMsg::Disconnect(player_id, DisconnectReason::Other as u32));
            return;
        }
    }

    if cfg.states.gameplay.anticheat.useless_anticheat.enable {
        if server.find_peer(player_id).map(|peer| peer.plr.mod_tool).unwrap_or(false) {
            let mut pkt = Packet::new(PacketType::SERVER_RING_COLLECTED);
            let _ = pkt.write_u8(0);
            let _ = pkt.write_u16(0);
            let _ = pkt.write_u8(1);
            let _ = pkt.write_u8(0);
            outbox.push(OutboxMsg::SendTo(player_id, pkt.data().to_vec(), true));
            return;
        }
    }

    if let Some(peer) = server.find_peer_mut(player_id) {
        peer.plr.heal_rings = 0;
        peer.plr.stats.hp_restored += 1;
        peer.plr.hp_credit = peer.plr.hp_credit.saturating_add(20).min(100);
    }

    relay_to_others(player_id, packet, outbox, true);
}

fn handle_cream_rings(player_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !is_ingame(player_id, server) { return; }
    if player_id as i32 == server.game.exe { return; }
    if !server.find_peer(player_id).map(|peer| peer.surv_char == SurvChar::Cream).unwrap_or(false) { return; }

    let cfg = cfg();
    if cfg.states.gameplay.anticheat.ability_anticheat {
        let cooldown = server.find_peer(player_id).map(|peer| peer.plr.cooldown).unwrap_or(1.0);
        if cooldown > 0.0 { return; }
        // Reject abilities used under the red/black-ring effect (client blocks
        // them; a Cheat-Engine bypass would still send the packet).
        if server.find_peer(player_id).map(|peer| peer.plr.red_ring).unwrap_or(false) { return; }
    }

    if cfg.states.gameplay.anticheat.useless_anticheat.enable {
        if server.find_peer(player_id).map(|peer| peer.plr.mod_tool).unwrap_or(false) {
            let mut pkt = Packet::new(PacketType::SERVER_RING_COLLECTED);
            let _ = pkt.write_u8(0);
            let _ = pkt.write_u16(0);
            let _ = pkt.write_u8(1);
            let _ = pkt.write_u8(0);
            outbox.push(OutboxMsg::SendTo(player_id, pkt.data().to_vec(), true));
            return;
        }
    }

    packet.pos = 2;
    let x         = match packet.read_u16() { Some(value) => value, None => return };
    let y         = match packet.read_u16() { Some(value) => value, None => return };
    let red_ring  = match packet.read_u8()  { Some(value) => value != 0, None => return };

    let v_pos = server.find_peer(player_id).map(|peer| peer.plr.pos).unwrap_or((0.0, 0.0));
    if vec2_dist(v_pos, (x as f32, y as f32)) > 40.0 { return; }

    if red_ring {
        let existing: Vec<(f32, f32)> = server.game.entities.iter()
            .filter(|entity| entity.tag() == "cring")
            .map(|entity| entity.pos())
            .collect();
        for pos in &existing {
            if vec2_dist(*pos, (x as f32, y as f32)) < 150.0 { return; }
        }

        let offsets: [(i16, i16); 2] = [(25, 0), (-27, 0)];
        for &(offset_x, offset_y) in &offsets {
            let ring_x = (x as i32 + offset_x as i32) as u16;
            let ring_y = (y as i32 + offset_y as i32) as u16;
            game_spawn(server, outbox, crate::entities::cream_ring::CreamRing::new(ring_x as f32, ring_y as f32, true));
        }
        with_status(|status| { status.cream_rings_spawned += 2; });
    } else {
        let offsets: [(i16, i16); 3] = [(26, 0), (0, -26), (-27, 0)];
        for &(offset_x, offset_y) in &offsets {
            let ring_x = (x as i32 + offset_x as i32) as u16;
            let ring_y = (y as i32 + offset_y as i32) as u16;
            game_spawn(server, outbox, crate::entities::cream_ring::CreamRing::new(ring_x as f32, ring_y as f32, false));
        }
        with_status(|status| { status.cream_rings_spawned += 3; });
    }

    if cfg.states.gameplay.anticheat.ability_anticheat {
        if let Some(peer) = server.find_peer_mut(player_id) {
            peer.plr.cooldown = 25.0 * 60.0;
        }
    }
}

fn handle_stats_report(player_id: u16, packet: &mut Packet, server: &mut Server, _outbox: &mut Vec<OutboxMsg>) {
    if !is_ingame(player_id, server) { return; }

    packet.pos = 2;
    let stat_type = match packet.read_u8() { Some(value) => value, None => return };

    match stat_type {
        0 => {
            if let Some(peer) = server.find_peer_mut(player_id) {
                peer.plr.stats.hp_restored += 1;
            }
        }
        1 => {
            let stunned    = match packet.read_u16() { Some(value) => value, None => return };
            let stunned_by = match packet.read_u16() { Some(value) => value, None => return };
            let seconds    = match packet.read_u8()  { Some(value) => value, None => return };
            if let Some(victim) = server.find_peer_mut(stunned) {
                victim.plr.stats.stun_time += seconds as u16;
            }
            if let Some(attacker) = server.find_peer_mut(stunned_by) {
                attacker.plr.stats.stuns += 1;
            }
            with_status(|status| { status.total_stuns += 1; });
        }
        2 => {
            let attacker_id = match packet.read_u16() { Some(value) => value, None => return };
            let damage         = match packet.read_u16() { Some(value) => value, None => return };
            let hp          = match packet.read_u16() { Some(value) => value, None => return };
            if let Some(attacker) = server.find_peer_mut(attacker_id) {
                if hp == 0 { attacker.plr.stats.kills += 1; }
                attacker.plr.stats.damage = attacker.plr.stats.damage.saturating_add(damage / 20);
            }
            with_status(|status| { status.damage_taken += (damage / 20) as u32; });
        }
        3 => {
            let damage = match packet.read_u8() { Some(value) => value, None => return };
            if let Some(peer) = server.find_peer_mut(player_id) {
                peer.plr.stats.damage_taken = peer.plr.stats.damage_taken.saturating_add((damage / 20) as u16);
            }
        }
        _ => {}
    }
}

fn handle_tprojectile_startcharge(player_id: u16, server: &mut Server) {
    let cfg = cfg();
    if cfg.states.gameplay.anticheat.ability_anticheat {
        if let Some(peer) = server.find_peer_mut(player_id) {
            peer.plr.tails_last_proj = Some(Instant::now());
        }
    }
}

fn handle_tprojectile(player_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !is_ingame(player_id, server) { return; }
    if player_id as i32 == server.game.exe { return; }
    if !server.find_peer(player_id).map(|peer| peer.surv_char == SurvChar::Tails).unwrap_or(false) { return; }

    let cfg = cfg();
    if cfg.states.gameplay.anticheat.ability_anticheat {
        let cooldown = server.find_peer(player_id).map(|peer| peer.plr.cooldown).unwrap_or(1.0);
        if cooldown > 0.0 { return; }
        // Reject abilities used under the red/black-ring effect (client blocks
        // them; a Cheat-Engine bypass would still send the packet).
        if server.find_peer(player_id).map(|peer| peer.plr.red_ring).unwrap_or(false) { return; }
    }

    packet.pos = 2;
    let x   = match packet.read_u16() { Some(value) => value, None => return };
    let y   = match packet.read_u16() { Some(value) => value, None => return };
    let dir = match packet.read_i8()  { Some(value) => value, None => return };
    let damage = match packet.read_u8()  { Some(value) => value, None => return };
    let exe = match packet.read_u8()  { Some(value) => value, None => return };
    let charge = match packet.read_u8()  { Some(value) => value, None => return };

    if dir < -1 || dir > 1 { return; }

    let dir = if cfg.states.gameplay.anticheat.useless_anticheat.enable
        && server.find_peer(player_id).map(|peer| peer.plr.mod_tool).unwrap_or(false)
    {
        if dir > 0 { -1 } else if dir < 0 { 1 } else { dir }
    } else {
        dir
    };

    let is_demonized = server.find_peer(player_id)
        .map(|peer| peer.plr.flags & plrflags::DEMONIZED != 0)
        .unwrap_or(false);

    if cfg.states.gameplay.anticheat.ability_anticheat {
        let last_ms = server.find_peer(player_id)
            .and_then(|peer| peer.plr.tails_last_proj)
            .map(|last| last.elapsed().as_millis() as u64)
            .unwrap_or(0);

        if is_demonized {
            if damage > 60 { return; }
            if damage >= 60 && !(last_ms > 1500 && last_ms < 12000) { return; }
        } else {
            if damage > 6 { return; }
            if damage >= 6 && !(last_ms > 1500 && last_ms < 12000) { return; }
        }
    }

    with_status(|status| { status.tails_shots += 1; });
    game_spawn(server, outbox, crate::entities::tails_projectile::TailsProjectile::new(
        x as f32, y as f32, player_id, dir, exe != 0, charge, damage,
    ));

    if cfg.states.gameplay.anticheat.ability_anticheat {
        if let Some(peer) = server.find_peer_mut(player_id) {
            peer.plr.cooldown = 10.0 * 60.0;
        }
    }
}

fn handle_tprojectile_hit(player_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !is_ingame(player_id, server) { return; }

    with_status(|status| { status.tails_hits += 1; });

    let entity_id = server.game.entities.iter().find(|entity| entity.tag() == "tproj").map(|entity| entity.id());
    if let Some(entity_id) = entity_id {
        game_despawn(server, outbox, entity_id);
    }
}

fn handle_etracker(player_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !is_ingame(player_id, server) { return; }
    if player_id as i32 == server.game.exe { return; }
    if !server.find_peer(player_id).map(|peer| peer.surv_char == SurvChar::Eggman).unwrap_or(false) { return; }

    let cfg = cfg();
    if cfg.states.gameplay.anticheat.ability_anticheat {
        let cooldown = server.find_peer(player_id).map(|peer| peer.plr.cooldown).unwrap_or(1.0);
        if cooldown > 0.0 { return; }
        // Reject abilities used under the red/black-ring effect (client blocks
        // them; a Cheat-Engine bypass would still send the packet).
        if server.find_peer(player_id).map(|peer| peer.plr.red_ring).unwrap_or(false) { return; }
    }

    packet.pos = 2;
    let x = match packet.read_u16() { Some(value) => value, None => return };
    let y = match packet.read_u16() { Some(value) => value, None => return };

    with_status(|status| { status.eggman_mines_placed += 1; });
    game_spawn(server, outbox, crate::entities::eggman_tracker::EggmanTracker::new(x as f32, y as f32));

    if let Some(peer) = server.find_peer_mut(player_id) {
        peer.plr.cooldown = 10.0 * 60.0;
    }
}

fn handle_etracker_activated(player_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !is_ingame(player_id, server) { return; }

    packet.pos = 2;
    let entity_id = match packet.read_u16() { Some(value) => value, None => return };

    game_eggtrack_activate(server, outbox, entity_id, player_id);
}

fn handle_erector_bring_spawn(player_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !is_ingame(player_id, server) { return; }
    if player_id as i32 != server.game.exe { return; }
    if !server.find_peer(player_id).map(|peer| peer.exe_char == ExeChar::Exetior).unwrap_or(false) { return; }

    let cfg = cfg();
    if cfg.states.gameplay.anticheat.ability_anticheat {
        let cooldown = server.find_peer(player_id).map(|peer| peer.plr.cooldown).unwrap_or(1.0);
        if cooldown > 0.0 { return; }
    }

    packet.pos = 2;
    let x = match packet.read_u16() { Some(value) => value, None => return };
    let y = match packet.read_u16() { Some(value) => value, None => return };

    let bring_positions: Vec<(f32, f32)> = server.game.entities.iter()
        .filter(|entity| entity.tag() == "bring")
        .map(|entity| entity.pos())
        .collect();
    for pos in &bring_positions {
        if vec2_dist(*pos, (x as f32, y as f32)) < 100.0 { return; }
    }

    if cfg.states.gameplay.anticheat.useless_anticheat.enable
        && server.find_peer(player_id).map(|peer| peer.plr.mod_tool).unwrap_or(false)
    {
        game_spawn(server, outbox, crate::entities::cream_ring::CreamRing::new(x as f32, y as f32, false));
        return;
    }

    with_status(|status| { status.exetior_bring_spawned += 1; });
    game_spawn(server, outbox, crate::entities::black_ring::BlackRing::new(x as f32, y as f32));

    if cfg.states.gameplay.anticheat.ability_anticheat {
        if let Some(peer) = server.find_peer_mut(player_id) {
            peer.plr.cooldown = 10.0 * 60.0;
        }
    }
}

fn handle_exeller_spawn_clone(player_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !is_ingame(player_id, server) { return; }
    if player_id as i32 != server.game.exe { return; }
    if !server.find_peer(player_id).map(|peer| peer.exe_char == ExeChar::Exeller).unwrap_or(false) { return; }

    let clone_count = server.game.entities.iter().filter(|entity| entity.tag() == "exclone").count();
    if clone_count >= 2 { return; }

    packet.pos = 2;
    let x   = match packet.read_u16() { Some(value) => value, None => return };
    let y   = match packet.read_u16() { Some(value) => value, None => return };
    let dir = match packet.read_i8()  { Some(value) => value, None => return };

    with_status(|status| { status.exeller_clones_placed += 1; });
    game_spawn(server, outbox, crate::entities::exeller_clone::ExellerClone::new(x as f32, y as f32, dir, player_id));
}

fn handle_exeller_teleport_clone(player_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !is_ingame(player_id, server) { return; }
    if player_id as i32 != server.game.exe { return; }
    if !server.find_peer(player_id).map(|peer| peer.exe_char == ExeChar::Exeller).unwrap_or(false) { return; }

    packet.pos = 2;
    let entity_id = match packet.read_u16() { Some(value) => value, None => return };

    if let Some(idx) = game_find_entity(server, entity_id) {
        let clone_owner = server.game.entities[idx].owner_id();
        let clone_dir   = server.game.entities[idx].dir_i8();

        if cfg().states.gameplay.anticheat.useless_anticheat.enable
            && server.find_peer(player_id).map(|peer| peer.plr.mod_tool).unwrap_or(false)
        {
            let plr_pos = server.find_peer(player_id).map(|peer| peer.plr.pos).unwrap_or((0.0, 0.0));
            game_despawn(server, outbox, entity_id);
            let mut pkt = Packet::new(PacketType::SERVER_EXELLERCLONE_STATE);
            let _ = pkt.write_u8(0);
            let _ = pkt.write_u16(entity_id);
            let _ = pkt.write_u16(clone_owner);
            let _ = pkt.write_u16(plr_pos.0 as u16);
            let _ = pkt.write_u16(plr_pos.1 as u16);
            let _ = pkt.write_i8(clone_dir);
            outbox.push(OutboxMsg::SendTo(player_id, pkt.data().to_vec(), true));
            return;
        }

        let mut pkt = Packet::new(PacketType::SERVER_EXELLERCLONE_STATE);
        let _ = pkt.write_u8(1);
        let _ = pkt.write_u16(entity_id);
        outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));

        game_despawn(server, outbox, entity_id);

        if let Some(peer) = server.find_peer_mut(player_id) {
            peer.plr.ex_teleport = 60;
        }
    }

    with_status(|status| { status.exeller_clones_activated += 1; });
}

fn handle_erector_balls(player_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !is_ingame(player_id, server) { return; }
    if player_id as i32 != server.game.exe { return; }

    packet.pos = 2;
    let x = match packet.read_f32() { Some(value) => value, None => return };
    let y = match packet.read_f32() { Some(value) => value, None => return };

    if cfg().states.gameplay.anticheat.useless_anticheat.enable
        && server.find_peer(player_id).map(|peer| peer.plr.mod_tool).unwrap_or(false)
    {
        for step in -3i32..3 {
            game_spawn(server, outbox, crate::entities::cream_ring::CreamRing::new(x + step as f32 * 8.0, y, false));
        }
        return;
    }

    let mut pkt = Packet::new(PacketType::CLIENT_ERECTOR_BALLS);
    let _ = pkt.write_f32(x);
    let _ = pkt.write_f32(y);
    outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));
}

fn handle_mercoin_bonus(player_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();
    if !cfg.states.gameplay.enable_achievements { return; }
    if !is_ingame(player_id, server) { return; }
    relay_to_others(player_id, packet, outbox, true);
}

/// Registers `entity` with the round: assigns it the next entity id, runs its
/// `on_init`, and keeps it only if `on_init` asked to be kept.
pub fn game_spawn<E: Entity + 'static>(
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
    entity: E,
) -> u16 {
    game_spawn_dyn(server, outbox, Box::new(entity))
}

/// Same as `game_spawn`, for an entity whose concrete type is only known at
/// runtime (entities spawned by other entities through `EntityCtx::spawn_queue`).
pub fn game_spawn_dyn(
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
    mut entity: Box<dyn Entity>,
) -> u16 {
    let id = server.game.next_entity_id;
    server.game.next_entity_id = server.game.next_entity_id.wrapping_add(1);
    entity.set_id(id);

    with_entity_op(server, outbox, |entities, ctx| {
        if entity.on_init(ctx) {
            entities.push(entity);
        }
    });
    id
}

pub fn game_despawn(server: &mut Server, outbox: &mut Vec<OutboxMsg>, ent_id: u16) {
    with_entity_op(server, outbox, |entities, ctx| {
        if let Some(idx) = entities.iter().position(|entity| entity.id() == ent_id) {
            entities[idx].on_uninit(ctx);
            entities.remove(idx);
        }
    });
}

pub fn game_find_entity(server: &Server, ent_id: u16) -> Option<usize> {
    server.game.entities.iter().position(|entity| entity.id() == ent_id)
}

pub fn game_eggtrack_activate(server: &mut Server, outbox: &mut Vec<OutboxMsg>, entity_id: u16, activator: u16) {
    if let Some(idx) = server.game.entities.iter().position(|entity| entity.tag() == "eggtrack" && entity.id() == entity_id) {
        server.game.entities[idx].set_activ_id(activator);
        game_despawn(server, outbox, entity_id);
    }
}

/// Runs `f` over the live entity list together with the read-only view of the
/// round that entity callbacks are allowed to see.
///
/// Every entity callback (`on_init`, `on_tick`, `on_uninit`) needs exactly this
/// context, so it is assembled here once instead of being spelled out at each
/// call site.
pub fn with_entity_op<F, R>(server: &mut Server, outbox: &mut Vec<OutboxMsg>, run: F) -> R
where
    F: FnOnce(&mut Vec<Box<dyn Entity>>, &mut EntityCtx) -> R,
{
    let mut rng       = SmallRng::from_entropy();
    let ingame_peers  = build_ingame_snapshot(server);
    let entity_ids    = server.game.entities.iter().map(|entity| entity.id()).collect();
    let map_id        = server.game.map;

    let mut ctx = EntityCtx {
        outbox,
        map_id,
        map_ring_count: map_ring_count(map_id),
        rings:          &mut server.game.rings,
        ingame_peers,
        entity_ids,
        exe_id:         server.game.exe,
        game_time_sec:  server.game.time_sec,
        game_time:      server.game.time,
        rand:           &mut rng,
        spawn_queue:    Vec::new(),
    };

    run(&mut server.game.entities, &mut ctx)
}

/// How many rings the map with this id was authored with; 0 for an id that is
/// not in MAP_LIST at all.
fn map_ring_count(map_id: i8) -> u8 {
    MAP_LIST.get(map_id as usize).map(|map| map.ring_count).unwrap_or(0)
}

fn build_ingame_snapshot(server: &Server) -> Vec<(u16, (f32, f32), u8, i8, i8, u8)> {
    server.peers.iter()
        .filter(|peer| peer.in_game)
        .map(|peer| (peer.id, peer.plr.pos, peer.plr.flags, peer.surv_char as i8, peer.exe_char as i8, peer.op))
        .collect()
}

pub fn game_state_tick(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();

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
    tick_entities(server, outbox);
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
            "[Server {}] Waiting for players took too long, kicking unready player id={}",
            server.id, id
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
        (server.game.elapsed / 60.0) as u16
    } else {
        server.game.time -= 1.0;
        (server.game.time / 60.0).max(0.0) as u16
    };
    if new_sec == server.game.time_sec {
        return true;
    }
    server.game.time_sec = new_sec;

    let ring_period = server.game.ring_coff as u16;
    if ring_period > 0
        && server.game.time_sec % ring_period == 0
        && cfg.states.gameplay.entities_misc.global.rings.enabled
    {
        game_spawn(server, outbox, crate::entities::ring::Ring::new());
    }

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

    let mut dead_players: Vec<(u16, f64)> = server.peers.iter()
        .filter(|peer| peer.in_game && peer.plr.flags & plrflags::DEAD != 0)
        .map(|peer| (peer.id, peer.plr.death_timer_sec as f64 + peer.plr.death_timer / 60.0))
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
    check_all_player_zones(server, outbox);
    tick_death_timers(server, outbox);
}

/// Advances every in-game player's own per-tick bookkeeping: idle/AFK deadlines,
/// ability cooldowns, the rolling ping average, and the Results-screen timers.
fn tick_peer_timers(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg     = cfg();
    let exe_id  = server.game.exe;
    let delta   = server.delta;

    let exe_pos: (f32, f32) = server.find_peer(exe_id as u16)
        .map(|peer| peer.plr.pos)
        .unwrap_or((0.0, 0.0));

    let mut to_disconnect: Vec<u16> = Vec::new();

    for peer in server.peers.iter_mut() {
        if !peer.in_game { continue; }

        if let Some(last) = peer.plr.last_packet {
            if last.elapsed().as_secs_f64() > 30.0 {
                to_disconnect.push(peer.id);
                continue;
            }
        }

        if peer.plr.cooldown > 0.0 {
            peer.plr.cooldown -= 1.0;
        } else {
            peer.plr.cooldown = 0.0;
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

        if peer.id as i32 != exe_id
            && peer.plr.flags & plrflags::DEAD    == 0
            && peer.plr.flags & plrflags::ESCAPED == 0
            && peer.plr.flags & plrflags::DEMONIZED == 0
        {
            peer.plr.stats.survive_time += delta;

            let dist = crate::player::vec2_dist(peer.plr.pos, exe_pos);
            if dist < 300.0 {
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

/// Runs the zone anti-cheat over every in-game player's last reported position.
fn check_all_player_zones(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let map_id = server.game.map;
    let in_game_ids: Vec<u16> = server.peers.iter()
        .filter(|peer| peer.in_game)
        .map(|peer| peer.id)
        .collect();
    for id in in_game_ids {
        player_check_zone(id, map_id, server, outbox);
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
        let is_exe       = other.id as i32 == server.game.exe;
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
    if accumulated >= 60.0 {
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
            if server.game.time_sec <= escape_time {
                game_bigring(server, BigRingState::Activated, outbox);
            }
        }
        BigRingState::Activated => {}
    }
}

fn tick_entities(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    // Entity packets are collected apart from `outbox` and appended at the end,
    // so an entity spawned mid-loop cannot reorder packets already queued.
    let mut entity_outbox: Vec<OutboxMsg> = Vec::new();

    let mut index = 0;
    while index < server.game.entities.len() {
        // on_tick may queue entities of its own; they are spawned only after the
        // tick returns, so the spawner can then be told the id of the last one.
        let mut pending_spawns: Vec<Box<dyn Entity>> = Vec::new();
        let keep = with_entity_op(server, &mut entity_outbox, |entities, ctx| {
            let keep = entities[index].on_tick(ctx);
            pending_spawns = std::mem::take(&mut ctx.spawn_queue);
            keep
        });

        let mut last_spawned_id = 0u16;
        for pending in pending_spawns {
            last_spawned_id = game_spawn_dyn(server, &mut entity_outbox, pending);
        }
        if last_spawned_id != 0 {
            server.game.entities[index].spawner_set_slug(last_spawned_id);
        }

        if keep {
            index += 1;
        } else {
            with_entity_op(server, &mut entity_outbox, |entities, ctx| {
                entities[index].on_uninit(ctx);
                entities.remove(index);
            });
        }
    }

    outbox.append(&mut entity_outbox);
}
