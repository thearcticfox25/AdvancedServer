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
pub const PLRSTATE_DEAD: u8      = 1;
pub const PLRSTATE_ESCAPED: u8   = 2;
pub const PLRSTATE_EXE: u8       = 3;
pub const PLRSTATE_SPECTATOR: u8 = 4;

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
    server.game.entid        = 0;
    server.game.bring_state  = BigRingState::None;
    server.game.bring_loc    = rand::random::<u8>();
    server.game.end          = 0.0;
    server.game.elapsed      = 0.0;

    server.game.time         = 60.0;
    server.game.time_sec     = 0;


    server.game.start_timeout = cfg().states.gameplay.waiting_timeout as f64 * 60.0;


    for p in server.peers.iter_mut() {
        if !p.in_game { continue; }
        p.plr = Player::default();
        p.plr.mod_tool = p.mod_tool;
        if p.id as i32 == exe {
            p.plr.flags |= plrflags::KILLER;
            p.plr.state  = PLRSTATE_EXE;
        } else {
            p.plr.state = PLRSTATE_ALIVE;
        }
        p.ready = false;
    }

    let ambush_pct = cfg().states.gameplay.gmcycle.ambush_force_demonization_percentage_on_start;
    if ambush_pct > 0 {
        ambush_force_demonize_on_start(exe, ambush_pct, server, outbox);
    }

    let pkt = Packet::new(PacketType::SERVER_LOBBY_GAME_START);
    outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));


    let cfg = cfg();
    let anon_mode = cfg.states.lobby_misc.anonymous_mode;
    let peer_snapshots: Vec<(u16, bool, String, u8, bool, i8, i8, u8)> = server.peers.iter().map(|p| {
        (p.id, p.in_game, p.nickname.clone(), p.lobby_icon, p.id as i32 == exe, p.exe_char as i8, p.surv_char as i8, p.op)
    }).collect();

    let non_ingame_ids: Vec<u16> = server.peers.iter()
        .filter(|p| !p.in_game)
        .map(|p| p.id)
        .collect();

    for target_id in non_ingame_ids {
        let target_op = peer_snapshots.iter().find(|s| s.0 == target_id).map(|s| s.7).unwrap_or(0);
        let anonymous = anon_mode && target_op < 1;
        for &(pid, in_game, ref nick, lobby_icon, is_exe, exe_char, surv_char, _) in &peer_snapshots {
            if pid == target_id { continue; }

            let mut pkt = Packet::new(PacketType::SERVER_WAITING_PLAYER_INFO);
            let _ = pkt.write_u8(if in_game { 1 } else { 0 });
            let _ = pkt.write_u16(pid);
            if anonymous {
                let _ = pkt.write_str("anonymous");
                let _ = pkt.write_u8(0);
            } else {
                let _ = pkt.write_str(nick);
                if in_game {
                    let _ = pkt.write_u8(if is_exe { 1 } else { 0 });
                    let _ = pkt.write_i8(if is_exe { exe_char } else { surv_char });
                } else {
                    let _ = pkt.write_u8(lobby_icon);
                }
            }
            outbox.push(OutboxMsg::SendTo(target_id, pkt.data().to_vec(), true));
        }
    }

    log::info!("[Server {}] Game starting: map={} exe={}", server.id, map, exe);
}

pub fn game_checkstart(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if server.game.started { return; }

    let ingame_count = server.peers.iter().filter(|p| p.in_game).count();
    let ready_count  = server.peers.iter().filter(|p| p.in_game && p.ready).count();

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
        let ceil = cfg.states.gameplay.gametimers_ceiling as u16;
        if server.game.time_sec > ceil {
            server.game.time_sec = ceil;
            server.game.time = ceil as f64 * 60.0;
        }
    }

    server.game.started = true;

    log::info!("[Server {}] Game players ready, map initialised. time_sec={}", server.id, server.game.time_sec);
}

pub fn game_uninit(server: &mut Server, show_results: bool, outbox: &mut Vec<OutboxMsg>) {
    let mut rng = SmallRng::from_entropy();
    let mut i = 0;
    while i < server.game.entities.len() {
        let ingame  = build_ingame_snapshot(server);
        let rings   = &mut server.game.rings;
        let ring_coff = server.game.ring_coff;
        let map_id  = server.game.map;
        let server_id = server.id;
        let exe_id  = server.game.exe;
        let gt      = server.game.time_sec;
        let gtime   = server.game.time;
        let mrc     = if (map_id as usize) < MAP_LIST.len() { MAP_LIST[map_id as usize].ring_count } else { 0 };
        let entity_ids: Vec<u16> = server.game.entities.iter().map(|e| e.id()).collect();
        let mut ctx = EntityCtx {
            outbox,
            server_id,
            map_id,
            map_ring_count: mrc,
            rings,
            ring_coff,
            ingame_peers: ingame,
            entity_ids,
            exe_id,
            game_time_sec: gt,
            game_time: gtime,
            rand: &mut rng,
            spawn_queue: Vec::new(),
        };
        server.game.entities[i].on_uninit(&mut ctx);
        i += 1;
    }
    server.game.entities.clear();

    if show_results {
        results_init(server, outbox);
    } else {
        crate::states::lobby::lobby_init(server, outbox);
        crate::states::lobby::lobby_broadcast_init(server, outbox);
    }
}

pub fn game_end(server: &mut Server, ending: Ending, achiv: bool, outbox: &mut Vec<OutboxMsg>) {
    if server.game.end > 0.0 { return; }

    log::info!("Ending is {:?}", ending);

    let cfg = cfg();
    server.game.ending = ending;
    server.game.end    = cfg.states.gameplay.ending_timer as f64 * 60.0;

    with_status(|s| {
        match ending {
            Ending::ExeWin   => s.exe_win_rounds  += 1,
            Ending::SurvWin  => s.surv_win_rounds += 1,
            Ending::TimeOver => s.draw_rounds      += 1,
        }
    });
    crate::status::save_status();


    let pkt_type = match ending {
        Ending::ExeWin   => PacketType::SERVER_GAME_EXE_WINS,
        Ending::SurvWin  => PacketType::SERVER_GAME_SURVIVOR_WIN,
        Ending::TimeOver => PacketType::SERVER_GAME_TIME_OVER,
    };
    let mut pkt = Packet::new(pkt_type);
    let _ = pkt.write_u8(if achiv { 1 } else { 0 });
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

fn game_demonize(v_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();
    let exe_id = server.game.exe;


    let players   = server.peers.iter().filter(|p| p.in_game && p.id as i32 != exe_id).count() as f64;
    let demonized = server.peers.iter()
        .filter(|p| p.in_game && p.id as i32 != exe_id && p.plr.flags & plrflags::DEMONIZED != 0)
        .count() as f64;

    let should_demonize = players * (cfg.states.gameplay.demonization_percentage as f64 / 100.0) > demonized;

    let nick = server.find_peer(v_id).map(|p| p.nickname.clone()).unwrap_or_default();

    let mut pkt = Packet::new(PacketType::SERVER_GAME_DEATHTIMER_END);
    if should_demonize {
        if let Some(pd) = server.find_peer_mut(v_id) {
            pd.plr.flags &= !plrflags::DEAD;
            pd.plr.flags |=  plrflags::DEMONIZED;
            pd.plr.stats.rings = 0;
        }
        with_status(|s| { s.total_demonised += 1; });
        let _ = pkt.write_u8(1);
        log::info!("{} (id {}) was demonized!", crate::colors::colorize(&nick), v_id);
    } else {
        if let Some(pd) = server.find_peer_mut(v_id) {
            pd.plr.flags |= plrflags::CANTREVIVE;
        }
        with_status(|s| { s.total_died += 1; });
        let _ = pkt.write_u8(0);
        log::info!("{} (id {}) died!", crate::colors::colorize(&nick), v_id);
    }
    outbox.push(OutboxMsg::SendTo(v_id, pkt.data().to_vec(), true));
}

/// Ambush: instantly demonizes `pct`% of non-exe in-game players at round start,
/// bypassing the wound/death-timer flow entirely.
fn ambush_force_demonize_on_start(exe: i32, pct: u8, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let mut candidates: Vec<u16> = server.peers.iter()
        .filter(|p| p.in_game && p.id as i32 != exe)
        .map(|p| p.id)
        .collect();
    candidates.shuffle(&mut rand::thread_rng());

    let count = ((candidates.len() as f64) * (pct as f64 / 100.0)).round() as usize;
    for &id in candidates.iter().take(count) {
        if let Some(pd) = server.find_peer_mut(id) {
            pd.plr.flags |= plrflags::DEMONIZED;
            pd.plr.stats.rings = 0;
        }
        let mut pkt = Packet::new(PacketType::SERVER_GAME_DEATHTIMER_END);
        let _ = pkt.write_u8(1);
        outbox.push(OutboxMsg::SendTo(id, pkt.data().to_vec(), true));
    }
    if count > 0 {
        with_status(|s| { s.total_demonised += count as u32; });
        log::info!("[Server {}] Ambush: {} player(s) demonized on start", server.id, count);
    }
}

pub fn game_state_join(v_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    use crate::states::waiting_room as wr;
    let exe_id = server.game.exe;
    let map    = server.game.map;
    wr::send_waiting_player_list(v_id, exe_id, server, outbox);
    wr::announce_waiter_joined(v_id, server, outbox);
    wr::send_waiting_room_greeting(v_id, Some(map), server, outbox);
}

pub fn game_state_left(v_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {

    let mut pkt = Packet::new(PacketType::SERVER_PLAYER_LEFT);
    let _ = pkt.write_u16(v_id);
    outbox.push(OutboxMsg::BroadcastEx(pkt.data().to_vec(), true, v_id));


    if server.game.end > 0.0 { return; }
    if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }

    let map_idx = server.game.map as usize;
    if map_idx < MAP_LIST.len() {
        let mut ob = Vec::new();
        (MAP_LIST[map_idx].left)(v_id, server, &mut ob);
        outbox.append(&mut ob);
    }

    let min_to_continue = cfg().states.lobby_misc.min_players_required.max(1) as usize;
    let remaining = server.peers.iter().filter(|p| p.in_game && p.id != v_id).count();
    if remaining < min_to_continue {
        game_uninit(server, false, outbox);
        return;
    }

    if !server.game.started {
        if v_id as i32 == server.game.exe {
            with_status(|s| { s.exe_crashed_rounds += 1; });
            game_uninit(server, false, outbox);
        } else {
            game_checkstart(server, outbox);
        }
        return;
    }

    if let Some(pd) = server.find_peer(v_id) {
        let mut left_pd = PeerData::new(pd.id, pd.ip.clone());
        left_pd.plr     = pd.plr.clone();
        left_pd.plr.flags |= plrflags::LEFT;
        left_pd.in_game    = pd.in_game;
        left_pd.surv_char  = pd.surv_char;
        left_pd.exe_char   = pd.exe_char;
        server.game.left.push(left_pd);
    }

    if v_id as i32 == server.game.exe {
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

    for p in &server.peers {
        if !p.in_game { continue; }
        if p.plr.flags & plrflags::ESCAPED   != 0 { escaped += 1; }
        if p.plr.flags & (plrflags::DEAD | plrflags::DEMONIZED) != 0 { dead += 1; }
        if p.id as i32 == exe_id { exes += 1; }
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
    v_id: u16,
    packet: &mut Packet,
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
) {


    if !server.game.started {
        if let Some(pd) = server.find_peer_mut(v_id) {
            if !pd.ready {
                pd.ready = true;
                pd.plr.last_packet = Some(Instant::now());
            }
        }
        game_checkstart(server, outbox);
        return;
    }

    let ptype = match packet.packet_type() {
        Some(t) => t,
        None    => return,
    };

    match ptype {

        PacketType::CLIENT_PLAYER_DATA => {
            handle_player_data(v_id, packet, server, outbox);
        }


        PacketType::CLIENT_PLAYER_HURT => {

            if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }
            packet.pos = 0;
            let data = packet.data().to_vec();
            outbox.push(OutboxMsg::BroadcastEx(data, true, v_id));
        }

        PacketType::CLIENT_PLAYER_DEATH_STATE => {
            handle_player_death_state(v_id, packet, server, outbox);
        }

        PacketType::CLIENT_PLAYER_ESCAPED => {
            handle_player_escaped(v_id, packet, server, outbox);
        }


        PacketType::CLIENT_RING_COLLECTED => {
            handle_ring_collected(v_id, packet, server, outbox);
        }

        PacketType::CLIENT_RING_BROKE => {

            if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }
            packet.pos = 0;
            let data = packet.data().to_vec();
            outbox.push(OutboxMsg::BroadcastEx(data, true, v_id));
        }

        PacketType::CLIENT_BRING_COLLECTED => {
            handle_bring_collected(v_id, packet, server, outbox);
        }


        PacketType::CLIENT_REVIVAL_PROGRESS => {
            handle_revival(v_id, packet, server, outbox);
        }


        PacketType::CLIENT_TPROJECTILE_STARTCHARGE => {
            handle_tprojectile_startcharge(v_id, server);
        }

        PacketType::CLIENT_TPROJECTILE => {
            handle_tprojectile(v_id, packet, server, outbox);
        }

        PacketType::CLIENT_TPROJECTILE_HIT => {
            handle_tprojectile_hit(v_id, server, outbox);
        }

        PacketType::CLIENT_CREAM_SPAWN_RINGS => {
            handle_cream_rings(v_id, packet, server, outbox);
        }

        PacketType::CLIENT_ETRACKER => {
            handle_etracker(v_id, packet, server, outbox);
        }

        PacketType::CLIENT_ETRACKER_ACTIVATED => {
            handle_etracker_activated(v_id, packet, server, outbox);
        }

        PacketType::CLIENT_ERECTOR_BRING_SPAWN => {
            handle_erector_bring_spawn(v_id, packet, server, outbox);
        }

        PacketType::CLIENT_EXELLER_SPAWN_CLONE => {
            handle_exeller_spawn_clone(v_id, packet, server, outbox);
        }

        PacketType::CLIENT_EXELLER_TELEPORT_CLONE => {
            handle_exeller_teleport_clone(v_id, packet, server, outbox);
        }

        PacketType::CLIENT_ERECTOR_BALLS => {
            handle_erector_balls(v_id, packet, server, outbox);
        }

        PacketType::CLIENT_MERCOIN_BONUS => {
            handle_mercoin_bonus(v_id, packet, server, outbox);
        }


        PacketType::CLIENT_STATS_REPORT => {
            handle_stats_report(v_id, packet, server, outbox);
        }


        PacketType::CLIENT_PLAYER_POTATER
        | PacketType::CLIENT_SPAWN_EFFECT
        | PacketType::CLIENT_SPRING_USE => {
            if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }
            packet.pos = 0;
            let data = packet.data().to_vec();
            outbox.push(OutboxMsg::BroadcastEx(data, true, v_id));
        }

        PacketType::CLIENT_SOUND_EMIT => {
            if !cfg().states.gameplay.enable_sounds { return; }
            if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }
            packet.pos = 0;
            let data = packet.data().to_vec();
            outbox.push(OutboxMsg::BroadcastEx(data, true, v_id));
        }

        PacketType::CLIENT_PLAYER_PALETTE => {
            if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }
            packet.pos = 0;
            let data = packet.data().to_vec();
            outbox.push(OutboxMsg::BroadcastEx(data, true, v_id));
        }

        PacketType::CLIENT_PET_PALETTE => {
            packet.pos = 0;
            let data = packet.data().to_vec();
            outbox.push(OutboxMsg::BroadcastEx(data, false, v_id));
        }

        PacketType::CLIENT_PLAYER_HEAL_PART => {
            handle_heal_part(v_id, packet, server, outbox);
        }

        PacketType::CLIENT_PLAYER_HEAL => {
            handle_heal(v_id, packet, server, outbox);
        }


        PacketType::CLIENT_CHAT_MESSAGE => {
            if !server.chat_rate_allow(v_id) { return; } // anti-flood
            if server.find_peer(v_id).map(|p| p.in_game).unwrap_or(true) { return; }
            packet.pos = 2;
            let _sender = packet.read_u16();
            let msg = match packet.read_str() { Some(s) => s, None => return };
            let nick = server.find_peer(v_id).map(|p| p.nickname.clone()).unwrap_or_default();
            log::info!("{} (id {}): {}", crate::colors::colorize(&nick), v_id, msg);
            crate::states::waiting_room::handle_waiter_chat(v_id, &msg, server, outbox);
        }


        PacketType::CLIENT_PING => {
            if cfg().states.gameplay.anticheat.useless_anticheat.enable {
                let is_mod = server.find_peer(v_id).map(|p| p.plr.mod_tool).unwrap_or(false);
                if is_mod && server.game.time_sec <= 125 {
                    return;
                }
            }

            let rtt = server.find_peer(v_id).map(|p| p.rtt).unwrap_or(1).max(1);

            let mut pong = Packet::new(PacketType::SERVER_PONG);
            let _ = pong.write_u16(rtt);
            outbox.push(OutboxMsg::SendTo(v_id, pong.data().to_vec(), false));


            let mut gping = Packet::new(PacketType::SERVER_GAME_PING);
            let _ = gping.write_u16(v_id);
            let _ = gping.write_u16(rtt);
            outbox.push(OutboxMsg::BroadcastEx(gping.data().to_vec(), false, v_id));

            if let Some(pd) = server.find_peer_mut(v_id) {
                pd.plr.ping_last = rtt;
            }
        }


        _ => {
            handle_entity_packet(v_id, ptype, packet, server, outbox);
        }
    }


    packet.pos = 0;
    let map_idx = server.game.map as usize;
    if map_idx < MAP_LIST.len() {
        let mut ob = Vec::new();
        (MAP_LIST[map_idx].tcp_msg)(v_id, packet, server, &mut ob);
        outbox.append(&mut ob);
    }
}

fn player_add_error(v_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>, by: u16) -> bool {
    let max_errors = cfg().server_config.pairing.player_maximum_errors;
    if let Some(pd) = server.find_peer_mut(v_id) {
        pd.plr.errors = pd.plr.errors.saturating_add(by);
        log::debug!("{} error is now {}", v_id, pd.plr.errors);
        if pd.plr.errors >= max_errors {
            outbox.push(OutboxMsg::Disconnect(v_id, DisconnectReason::KickedByHost as u32));
            return false;
        }
    }
    true
}

fn player_check_zone(v_id: u16, map_id: i8, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !cfg().states.gameplay.anticheat.zone_anticheat { return; }
    if !server.game.started { return; }

    let (pos, ex_tp) = match server.find_peer(v_id) {
        Some(p) => (p.plr.pos, p.plr.ex_teleport),
        None => return,
    };
    if pos == (0.0, 0.0) { return; }
    if ex_tp > 0 { return; }

    let (px, py) = (pos.0, pos.1);
    let error_amount: u16 = if map_id == 6 { 30 } else { 1000 };

    if zone::legacy_zone_anticheat(px, py, map_id) {
        log::debug!("{} is inside invalid area (legacy zone AC)", v_id);
        player_add_error(v_id, server, outbox, error_amount);
        return;
    }

    if zone::zone_anticheat(px, py, map_id) {
        log::debug!("{} is out of bounds (physics JSON zone AC)", v_id);
        player_add_error(v_id, server, outbox, error_amount);
    }
}

fn handle_player_data(v_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if packet.len < 10 { return; }

    let cfg    = cfg();
    let exe_id = server.game.exe;
    let map_id = server.game.map;

    if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }

    packet.pos = 2;
    let x    = match packet.read_u16() { Some(v) => v, None => return };
    let y    = match packet.read_u16() { Some(v) => v, None => return };
    let xspd = packet.read_u16().map(physics::f16_to_f32).unwrap_or(0.0);
    let yspd = packet.read_u16().map(physics::f16_to_f32).unwrap_or(0.0);
    let state = match packet.read_u8() { Some(v) => v, None => return };

    let _angle  = packet.read_i16();
    let _index  = packet.read_u8();
    let _xscale = packet.read_i8();

    let is_exe    = v_id as i32 == exe_id;
    let surv_char = server.find_peer(v_id).map(|p| p.surv_char).unwrap_or(SurvChar::None);

    let surv_hp:           i8;
    let surv_revival:      u8;
    let surv_rings:        i16;
    let surv_tails_charge: u8;
    let flags: u8;

    if !is_exe {
        surv_hp      = match packet.read_i8()  { Some(v) => v, None => return };
        surv_revival = match packet.read_u8()  { Some(v) => v, None => return };
        surv_rings   = match packet.read_i16() { Some(v) => v, None => return };
        flags        = match packet.read_u8()  { Some(v) => v, None => return };

        surv_tails_charge = if surv_char == SurvChar::Tails {
            let c = packet.read_u8().unwrap_or(0);
            let _ = packet.read_i16();
            c
        } else {
            0
        };

        let peer_flags = server.find_peer(v_id).map(|p| p.plr.flags).unwrap_or(0);
        let srv_is_normal = peer_flags & (plrflags::DEAD | plrflags::DEMONIZED) == 0;
        if srv_is_normal {
            if let Some(pd) = server.find_peer_mut(v_id) {
                pd.plr.rings = surv_rings;
            }

            let srv_not_demonized = peer_flags & (plrflags::DEMONIZED | plrflags::REVIVED | plrflags::CANTREVIVE) == 0;
            if srv_not_demonized && cfg.states.gameplay.anticheat.data_based_anticheat {
                if surv_rings < 0 || (map_id != 20 && surv_rings >= 120) {
                    outbox.push(OutboxMsg::Disconnect(v_id, DisconnectReason::Other as u32));
                    return;
                }
                if surv_hp > 100 {
                    outbox.push(OutboxMsg::Disconnect(v_id, DisconnectReason::Other as u32));
                    return;
                }
            }
        }
    } else {
        surv_hp = 0; surv_revival = 0; surv_rings = 0; surv_tails_charge = 0;
        flags = match packet.read_u8() { Some(v) => v, None => return };
    }

    const PLAYER_ATTACKING: u8 = 1 << 4;
    const PLAYER_REDRING:   u8 = 1 << 3;
    let is_attacking = flags & PLAYER_ATTACKING != 0;
    // Red-ring (or black-ring) effect disables every ability client-side; the
    // survivor broadcasts it via PLAYER_REDRING. Store it so the ability handlers can
    // reject ability packets sent while it is active (Cheat-Engine bypass).
    let red_ring_now = !is_exe && (flags & PLAYER_REDRING != 0);
    let duration_ms  = if !is_exe && surv_char == SurvChar::Eggman { 3000.0f64 } else { 2000.0f64 };

    if server.game.end <= 0.0 && is_attacking {
        let attack_timer = server.find_peer(v_id).map(|p| p.plr.attack_timer).unwrap_or(0.0);
        if attack_timer <= 0.0 {
            if let Some(pd) = server.find_peer_mut(v_id) {
                pd.plr.last_attack  = Some(Instant::now());
                pd.plr.attack_timer = 1.0;
            }
        } else {
            let elapsed_ms = server.find_peer(v_id)
                .and_then(|p| p.plr.last_attack)
                .map(|t| t.elapsed().as_millis() as f64)
                .unwrap_or(0.0);
            let new_timer = attack_timer + elapsed_ms;
            if new_timer >= duration_ms {
                outbox.push(OutboxMsg::Disconnect(v_id, DisconnectReason::KickedByHost as u32));
                return;
            }
            if let Some(pd) = server.find_peer_mut(v_id) {
                pd.plr.attack_timer = new_timer;
                pd.plr.last_attack  = Some(Instant::now());
            }
        }
    } else if let Some(pd) = server.find_peer_mut(v_id) {
        pd.plr.attack_timer = 0.0;
    }

    if cfg.states.gameplay.anticheat.zero_trust_anticheat && server.game.started {
        let is_mod_zt = server.find_peer(v_id).map(|p| p.plr.mod_tool).unwrap_or(false);
        if !is_mod_zt {
            let zt_log    = cfg.states.gameplay.anticheat.zero_trust_log_only;
            let peer_flags = server.find_peer(v_id).map(|p| p.plr.flags).unwrap_or(0);
            let nick      = server.find_peer(v_id).map(|p| p.nickname.clone()).unwrap_or_default();

            const PLAYER_INVIS: u8 = 1 << 6;

            if is_exe && (flags & PLAYER_INVIS != 0) && (flags & PLAYER_ATTACKING != 0) {
                log::debug!(
                    "[zero_trust] INVIS_ATTACK: player {}({}) is attacking while invisible",
                    v_id, nick
                );
                if !zt_log {
                    outbox.push(OutboxMsg::Disconnect(v_id, DisconnectReason::Other as u32));
                    return;
                }
            }

            if !is_exe {
                let prev_hp    = server.find_peer(v_id).map(|p| p.plr.hp       ).unwrap_or(-1);
                let hp_credit  = server.find_peer(v_id).map(|p| p.plr.hp_credit).unwrap_or(0);

                let is_demonized = peer_flags & plrflags::DEMONIZED   != 0;
                let was_revived  = peer_flags & plrflags::REVIVED      != 0;
                let cant_revive  = peer_flags & plrflags::CANTREVIVE   != 0;
                let server_revival_max: u8 = if is_demonized { 2 }
                                        else if was_revived || cant_revive { 1 }
                                        else { 0 };

                if surv_revival > server_revival_max.saturating_add(1) {
                    log::debug!(
                        "[zero_trust] REVIVAL_EXCEED: player {}({}) claimed revival={} \
                         but server max={} (flags={:#04x})",
                        v_id, nick, surv_revival, server_revival_max, peer_flags
                    );
                    outbox.push(OutboxMsg::Disconnect(v_id, DisconnectReason::Other as u32));
                    return;
                }

                let prev_revival = server.find_peer(v_id).map(|p| p.plr.revival_times).unwrap_or(0);
                if surv_revival < prev_revival {
                    log::debug!(
                        "[zero_trust] REVIVAL_DEC: player {}({}) revival {}→{} (impossible)",
                        v_id, nick, prev_revival, surv_revival
                    );
                    outbox.push(OutboxMsg::Disconnect(v_id, DisconnectReason::Other as u32));
                    return;
                }

                if prev_hp >= 0 && !is_demonized && server_revival_max < 2 {
                    let gain = (surv_hp as i16) - (prev_hp as i16);
                    if gain > hp_credit as i16 {
                        log::debug!(
                            "[zero_trust] HP_GAIN: player {}({}) hp {}→{} \
                             credit={} (unexplained gain={})",
                            v_id, nick, prev_hp, surv_hp, hp_credit, gain
                        );
                        if !zt_log {
                            outbox.push(OutboxMsg::Disconnect(v_id, DisconnectReason::Other as u32));
                            return;
                        }
                    }
                }

                if surv_char == SurvChar::Tails && surv_tails_charge > 0 && xspd.abs() > 1.5 {
                    log::debug!(
                        "[zero_trust] TAILS_MOVE_CHARGE: player {}({}) \
                         charge={} xspd={:.2}",
                        v_id, nick, surv_tails_charge, xspd
                    );
                }

                let gain = ((surv_hp as i16) - (prev_hp as i16)).max(0) as i8;
                if let Some(pd) = server.find_peer_mut(v_id) {
                    pd.plr.hp_credit = pd.plr.hp_credit.saturating_sub(gain).max(0);
                }
            }
        }
    }

    let new_pos = (x as f32, y as f32);

    let is_mod = server.find_peer(v_id).map(|p| p.plr.mod_tool).unwrap_or(false);

    if cfg.states.gameplay.anticheat.physics_anticheat {
        if server.game.started && !is_mod && new_pos != (0.0, 0.0) {
            if let Some(mp) = physics::table().get(map_id) {
                let log_only = cfg.states.gameplay.anticheat.physics_anticheat_log_only;
                let nick = server.find_peer(v_id).map(|p| p.nickname.clone()).unwrap_or_default();
                let map_name = MAP_LIST.get(map_id as usize).map(|m| m.name).unwrap_or("?");

                if mp.out_of_bounds(new_pos.0, new_pos.1) {
                    log::debug!(
                        "[physics_ac] OOB: player {}({}) at ({:.0},{:.0}) map=\"{}\" room={}x{}",
                        v_id, nick, new_pos.0, new_pos.1,
                        map_name, mp.width, mp.height
                    );
                    if !log_only && !player_add_error(v_id, server, outbox, 300) {
                        return;
                    }
                }

                if let Some(solid) = mp.solid_violation(new_pos.0, new_pos.1) {
                    log::debug!(
                        "[physics_ac] SOLID: player {}({}) at ({:.0},{:.0}) inside solid \
                         ({:.0},{:.0} {}x{}) map=\"{}\"",
                        v_id, nick, new_pos.0, new_pos.1,
                        solid.x, solid.y, solid.w, solid.h,
                        map_name
                    );
                    if !log_only && !player_add_error(v_id, server, outbox, 600) {
                        return;
                    }
                }
            }
        }
    }

    if cfg.states.gameplay.anticheat.distance_anticheat {
        let old_pos = server.find_peer(v_id).map(|p| p.plr.pos).unwrap_or((0.0, 0.0));
        let ex_tp   = server.find_peer(v_id).map(|p| p.plr.ex_teleport).unwrap_or(0);

        if server.game.started && !is_mod && old_pos != (0.0, 0.0) {
            if ex_tp == 0 {
                let dist = vec2_dist(old_pos, new_pos);
                if dist > 700.0 {
                    match map_id {
                        6 | 8 | 13 | 15 => {}
                        _ => {
                            outbox.push(OutboxMsg::Disconnect(v_id, DisconnectReason::Other as u32));
                            return;
                        }
                    }
                }
                if dist > 60.0 && !player_add_error(v_id, server, outbox, 500) {
                    return;
                }
            } else if let Some(pd) = server.find_peer_mut(v_id) {
                pd.plr.ex_teleport -= 1;
            }
        }
    }

    if cfg.states.gameplay.anticheat.physics_correction {
        if server.game.started && !is_mod {
            let prev_pos  = server.find_peer(v_id).map(|p| p.plr.pos     ).unwrap_or((0.0, 0.0));
            let prev_vel  = server.find_peer(v_id).map(|p| p.plr.vel     ).unwrap_or((0.0, 0.0));
            let good_pos  = server.find_peer(v_id).map(|p| p.plr.good_pos).unwrap_or((0.0, 0.0));
            let elapsed_ms = server.find_peer(v_id)
                .and_then(|p| p.plr.last_packet)
                .map(|t| t.elapsed().as_millis() as f32)
                .unwrap_or(16.667);

            if prev_pos != (0.0, 0.0) {
                let new_vel = (xspd, yspd);
                let log_only = cfg.states.gameplay.anticheat.physics_correction_log_only;
                let nick     = server.find_peer(v_id).map(|p| p.nickname.clone()).unwrap_or_default();
                let map_name = MAP_LIST.get(map_id as usize).map(|m| m.name).unwrap_or("?");

                let (verdict, deviation) =
                    physics::check_correction(prev_pos, prev_vel, new_pos, new_vel, elapsed_ms);

                match verdict {
                    physics::CorrectionVerdict::VelocityOverflow => {
                        log::debug!(
                            "[physics_corr] VEL_OVERFLOW: player {}({}) vel=({:.2},{:.2}) \
                             max={:.1} map=\"{}\"",
                            v_id, nick, xspd, yspd, deviation, map_name
                        );
                        if !log_only {
                            let mut pkt = Packet::new(PacketType::SERVER_PLAYER_BACKTRACK);
                            let _ = pkt.write_f32(good_pos.0);
                            let _ = pkt.write_f32(good_pos.1);
                            outbox.push(OutboxMsg::SendTo(v_id, pkt.data().to_vec(), false));
                            return;
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
                                v_id, nick,
                                new_pos.0, new_pos.1,
                                prev_pos.0, prev_pos.1,
                                xspd, yspd, deviation, elapsed_ms,
                                map_name
                            );
                            if !log_only {
                                let mut pkt = Packet::new(PacketType::SERVER_PLAYER_BACKTRACK);
                                let _ = pkt.write_f32(good_pos.0);
                                let _ = pkt.write_f32(good_pos.1);
                                outbox.push(OutboxMsg::SendTo(v_id, pkt.data().to_vec(), false));
                                return;
                            }
                        }
                        if let Some(pd) = server.find_peer_mut(v_id) {
                            pd.plr.good_pos = new_pos;
                        }
                    }
                    physics::CorrectionVerdict::Ok => {
                        if let Some(pd) = server.find_peer_mut(v_id) {
                            pd.plr.good_pos = new_pos;
                        }
                    }
                }
            } else {
                if let Some(pd) = server.find_peer_mut(v_id) {
                    pd.plr.good_pos = new_pos;
                }
            }
        }
    }

    if let Some(pd) = server.find_peer_mut(v_id) {
        if pd.plr.pos == (0.0, 0.0) && new_pos != (0.0, 0.0) {
            pd.plr.ex_teleport = pd.plr.ex_teleport.max(10);
        }
        pd.plr.pos          = new_pos;
        pd.plr.vel          = (xspd, yspd);
        pd.plr.state        = state;
        pd.plr.last_packet  = Some(Instant::now());
        pd.plr.timeout      = 0.0;
        pd.plr.is_attacking = is_attacking;
        pd.plr.red_ring     = red_ring_now;
        if !is_exe {
            pd.plr.hp            = surv_hp;
            pd.plr.revival_times = surv_revival;
            pd.plr.tails_charge  = surv_tails_charge;
        }
    }

    // Black-ring contact: an honest survivor standing inside an erector black ring
    // takes 20 damage (net_state_game.SERVER_BRING_COLLECTED). Cheats either never send
    // CLIENT_BRING_COLLECTED or patch out the damage. The server knows erector-ring
    // positions, so force the collection when a survivor is clearly inside one it never
    // reported. (Map-placed rings arrive with a sentinel position → deferred to the
    // geometry-extraction work; verifying the 20 hp actually landed is the same hp-
    // authority problem as the exe-hit case, also deferred.)
    if cfg.states.gameplay.anticheat.zero_trust_anticheat
        && server.game.started && !is_mod && !is_exe
        && surv_hp > 0 && surv_revival < 2
    {
        let (px, py) = new_pos;
        // Ring box [rx,rx+30]×[ry,ry+30] shrunk 6 px vs the player box (x±12, y-18..y+14)
        // — deliberately conservative so an AABB-vs-mask edge skim never costs 20 hp.
        let ring_eid = server.game.entities.iter()
            .filter_map(|e| e.bring_hit_pos().map(|p| (e.id(), p)))
            .find(|&(_, (rx, ry))| {
                px + 12.0 > rx + 6.0 && px - 12.0 < rx + 24.0 &&
                py + 14.0 > ry + 6.0 && py - 18.0 < ry + 24.0
            })
            .map(|(id, _)| id);

        if let Some(eid) = ring_eid {
            let nick = server.find_peer(v_id).map(|p| p.nickname.clone()).unwrap_or_default();
            log::debug!("[zero_trust] BRING_PHASE: player {}({}) inside black ring {} without collecting", v_id, nick, eid);
            if !cfg.states.gameplay.anticheat.zero_trust_log_only {
                game_despawn(server, outbox, eid);
                let pkt = Packet::new(PacketType::SERVER_BRING_COLLECTED);
                outbox.push(OutboxMsg::SendTo(v_id, pkt.data().to_vec(), true));
            }
        }
    }

    if is_mod && cfg.states.gameplay.anticheat.useless_anticheat.enable {
        let timer = server.find_peer(v_id).map(|p| p.plr.mod_tool_timer).unwrap_or(0);
        if timer < 61 {
            if let Some(pd) = server.find_peer_mut(v_id) {
                pd.plr.mod_tool_timer += 1;
            }
            let new_timer = timer + 1;
            if new_timer == 60 {
                let cur_pos = server.find_peer(v_id).map(|p| p.plr.pos).unwrap_or((0.0, 0.0));
                if let Some(pd) = server.find_peer_mut(v_id) {
                    pd.plr.start_pos = cur_pos;
                }
                let mut pkt = Packet::new(PacketType::SERVER_FELLA);
                let _ = pkt.write_u16(v_id);
                let _ = pkt.write_f32(cur_pos.0);
                let _ = pkt.write_f32(cur_pos.1);
                outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));
                return;
            }
        } else {
            let cur_pos   = server.find_peer(v_id).map(|p| p.plr.pos      ).unwrap_or((0.0, 0.0));
            let start_pos = server.find_peer(v_id).map(|p| p.plr.start_pos).unwrap_or((0.0, 0.0));
            let dist = vec2_dist(cur_pos, start_pos);
            if dist >= 300.0 {
                outbox.push(OutboxMsg::Disconnect(v_id, DisconnectReason::Other as u32));
            } else if dist > 240.0 {
                let mut pkt = Packet::new(PacketType::SERVER_PLAYER_BACKTRACK);
                let _ = pkt.write_f32(start_pos.0);
                let _ = pkt.write_f32(start_pos.1);
                outbox.push(OutboxMsg::SendTo(v_id, pkt.data().to_vec(), true));
            }
            return;
        }
    }

    let raw = packet.buf[2..packet.len].to_vec();
    let mut pkt = Packet::new(PacketType::CLIENT_PLAYER_DATA);
    let _ = pkt.write_u16(v_id);
    for &b in &raw { let _ = pkt.write_u8(b); }
    outbox.push(OutboxMsg::BroadcastEx(pkt.data().to_vec(), false, v_id));
}

fn handle_player_death_state(v_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if server.game.end > 0.0 { return; }

    if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }

    if v_id as i32 == server.game.exe { return; }

    if server.find_peer(v_id).map(|p| p.plr.flags & plrflags::DEMONIZED != 0).unwrap_or(false) { return; }

    packet.pos = 2;
    let dead   = match packet.read_u8() { Some(v) => v, None => return };
    let rtimes = match packet.read_u8() { Some(v) => v, None => return };


    let mut pkt = Packet::new(PacketType::SERVER_PLAYER_DEATH_STATE);
    let _ = pkt.write_u16(v_id);
    let _ = pkt.write_u8(dead);
    let _ = pkt.write_u8(rtimes);
    outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));


    let mut rs = Packet::new(PacketType::SERVER_REVIVAL_STATUS);
    let _ = rs.write_u8(0);
    let _ = rs.write_u16(v_id);
    outbox.push(OutboxMsg::Broadcast(rs.data().to_vec(), true));

    if dead != 0 {

        let already = server.find_peer(v_id)
            .map(|p| p.plr.flags & (plrflags::DEAD | plrflags::ESCAPED) != 0)
            .unwrap_or(true);
        if already { return; }

        if let Some(pd) = server.find_peer_mut(v_id) {
            pd.plr.flags |= plrflags::DEAD;
        }

        let cfg = cfg();


        let revived_flag = server.find_peer(v_id)
            .map(|p| p.plr.flags & plrflags::REVIVED != 0)
            .unwrap_or(false);
        let in_sudden_death = server.game.sudden_death;

        if revived_flag || in_sudden_death {
            game_demonize(v_id, server, outbox);
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

            if let Some(pd) = server.find_peer_mut(v_id) {
                pd.plr.death_timer_sec = death_timer_sec;
                // death_timer (the 0..60 sub-second accumulator) starts at 0 at the
                // moment of death, independent from death_timer_sec (the whole-second
                // count): tick_players' >=60 rollover check must only fire after a
                // full real second has actually accumulated.
                pd.plr.death_timer     = 0.0;
            }


            let exe_id = server.game.exe;
            let exe_pos = server.find_peer(exe_id as u16).map(|p| p.plr.pos).unwrap_or((0.0, 0.0));
            let v_pos   = server.find_peer(v_id).map(|p| p.plr.pos).unwrap_or((0.0, 0.0));
            let dist    = crate::player::vec2_dist(v_pos, exe_pos);
            let exe_near = (dist <= 240.0 && cfg.states.gameplay.exe_camp_penalty) as u8;

            let mut dt = Packet::new(PacketType::SERVER_GAME_DEATHTIMER_TICK);
            let _ = dt.write_u8(exe_near);
            let _ = dt.write_u16(v_id);
            let _ = dt.write_u8(death_timer_sec);
            outbox.push(OutboxMsg::Broadcast(dt.data().to_vec(), true));

            with_status(|s| { s.total_stuns += 1; });
        }
    } else {


        if server.find_peer(v_id).map(|p| p.plr.death_timer_sec == 0).unwrap_or(true) { return; }
        if let Some(pd) = server.find_peer_mut(v_id) {
            pd.plr.flags       &= !plrflags::DEAD;
            pd.plr.death_timer  = 0.0;
        }
    }

    game_state_check(server, outbox);
}

fn handle_player_escaped(v_id: u16, _packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }
    if v_id as i32 == server.game.exe { return; }

    if cfg().states.gameplay.anticheat.useless_anticheat.enable {
        if server.find_peer(v_id).map(|p| p.plr.mod_tool).unwrap_or(false) {
            outbox.push(OutboxMsg::Disconnect(v_id, DisconnectReason::ServerTimeout as u32));
            return;
        }
    }

    let flags = server.find_peer(v_id).map(|p| p.plr.flags).unwrap_or(0);
    if flags & (plrflags::DEAD | plrflags::DEMONIZED | plrflags::ESCAPED) != 0 { return; }

    if let Some(pd) = server.find_peer_mut(v_id) {
        pd.plr.flags |= plrflags::ESCAPED;
        pd.plr.state  = PLRSTATE_ESCAPED;
    }


    let mut pkt = Packet::new(PacketType::SERVER_PLAYER_ESCAPED);
    outbox.push(OutboxMsg::SendTo(v_id, pkt.data().to_vec(), true));


    let mut pkt2 = Packet::new(PacketType::SERVER_GAME_PLAYER_ESCAPED);
    let _ = pkt2.write_u16(v_id);
    outbox.push(OutboxMsg::BroadcastEx(pkt2.data().to_vec(), true, v_id));

    with_status(|s| { s.total_escaped += 1; });

    game_state_check(server, outbox);
}

fn handle_ring_collected(v_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }

    packet.pos = 2;
    let ring_id = match packet.read_u8()  { Some(v) => v, None => return };
    let eid     = match packet.read_u16() { Some(v) => v, None => return };


    let red = server.game.entities.iter()
        .find(|e| e.id() == eid)
        .map(|e| e.is_red())
        .unwrap_or(false);

    let despawned = game_find_entity(server, eid).is_some();
    if despawned {
        game_despawn(server, outbox, eid);
    } else {
        return;
    }


    if !red {
        if let Some(pd) = server.find_peer_mut(v_id) {
            pd.plr.rings = pd.plr.rings.saturating_add(1);
            pd.plr.stats.rings += 1;
        }
    }

    let has_rings = server.find_peer(v_id).map(|p| p.plr.rings > 0).unwrap_or(false);
    let mut pkt = Packet::new(PacketType::SERVER_RING_COLLECTED);
    let _ = pkt.write_u8(ring_id);
    let _ = pkt.write_u16(eid);
    let _ = pkt.write_u8(red as u8);
    let _ = pkt.write_u8(if has_rings { 1 } else { 0 });
    outbox.push(OutboxMsg::SendTo(v_id, pkt.data().to_vec(), true));
}

fn handle_bring_collected(v_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {

    if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }
    if v_id as i32 == server.game.exe { return; }

    packet.pos = 2;
    let eid = match packet.read_u16() { Some(v) => v, None => return };

    if game_find_entity(server, eid).is_some() {
        game_despawn(server, outbox, eid);
        let mut pkt = Packet::new(PacketType::SERVER_BRING_COLLECTED);
        outbox.push(OutboxMsg::SendTo(v_id, pkt.data().to_vec(), true));
    }
}

fn handle_revival(v_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if server.game.end > 0.0 { return; }

    packet.pos = 2;
    let pid   = match packet.read_u16() { Some(v) => v, None => return };
    let rings = match packet.read_u8()  { Some(v) => v, None => return };


    let target_dead = server.find_peer(pid)
        .map(|p| p.plr.flags & plrflags::DEAD != 0)
        .unwrap_or(false);
    if !target_dead { return; }

    let cantrevive = server.find_peer(pid)
        .map(|p| p.plr.flags & plrflags::CANTREVIVE != 0)
        .unwrap_or(false);

    if cantrevive {

        if let Some(tp) = server.find_peer_mut(pid) {
            tp.plr.death_timer_sec = 0;
            tp.plr.death_timer     = 0.0;
        }
        let mut rs = Packet::new(PacketType::SERVER_REVIVAL_STATUS);
        let _ = rs.write_u8(0);
        let _ = rs.write_u16(pid);
        outbox.push(OutboxMsg::Broadcast(rs.data().to_vec(), true));
        return;
    }


    let was_zero = server.find_peer(pid).map(|p| p.plr.revival <= 0.0).unwrap_or(true);
    if was_zero {
        let mut rs = Packet::new(PacketType::SERVER_REVIVAL_STATUS);
        let _ = rs.write_u8(1);
        let _ = rs.write_u16(pid);
        outbox.push(OutboxMsg::Broadcast(rs.data().to_vec(), true));
    }


    let increment = 0.015 + 0.004 * rings as f64;
    if let Some(tp) = server.find_peer_mut(pid) {
        tp.plr.revival += increment;
    }

    let revival = server.find_peer(pid).map(|p| p.plr.revival).unwrap_or(0.0);

    if revival < 1.0 {

        let mut pp = Packet::new(PacketType::SERVER_REVIVAL_PROGRESS);
        let _ = pp.write_u16(pid);
        let _ = pp.write_f64(revival);
        outbox.push(OutboxMsg::Broadcast(pp.data().to_vec(), false));


        if let Some(tp) = server.find_peer_mut(pid) {
            let has = tp.plr.revival_init.iter().any(|&r| r == v_id as i32);
            if !has {
                for slot in &mut tp.plr.revival_init {
                    if *slot == -1 { *slot = v_id as i32; break; }
                }
            }
        }
    } else {

        if let Some(tp) = server.find_peer_mut(pid) {
            tp.plr.stats.rings = 0;
            tp.plr.flags &= !plrflags::DEAD;
            tp.plr.flags |=  plrflags::REVIVED;
            tp.plr.revival = 0.0;
        }

        let mut rs = Packet::new(PacketType::SERVER_REVIVAL_STATUS);
        let _ = rs.write_u8(0);
        let _ = rs.write_u16(pid);
        outbox.push(OutboxMsg::Broadcast(rs.data().to_vec(), true));

        let mut rr = Packet::new(PacketType::SERVER_REVIVAL_REVIVED);
        outbox.push(OutboxMsg::SendTo(pid, rr.data().to_vec(), true));

        if let Some(p) = server.find_peer(pid) {
            log::info!("{} (id {}) was revived!", crate::colors::colorize(&p.nickname), pid);
        }

        let revivers: Vec<i32> = server.find_peer(pid)
            .map(|p| p.plr.revival_init.to_vec())
            .unwrap_or_default();
        for r in revivers {
            if r == -1 { break; }
            log::debug!("Removed rings from {}", r);
            let mut sub = Packet::new(PacketType::SERVER_REVIVAL_RINGSUB);
            outbox.push(OutboxMsg::SendTo(r as u16, sub.data().to_vec(), true));
        }


        if let Some(tp) = server.find_peer_mut(pid) {
            tp.plr.revival_init = [-1; 5];
        }
    }
}

fn handle_heal_part(v_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }

    packet.pos = 2;
    let _x    = match packet.read_u16() { Some(v) => v, None => return };
    let _y    = match packet.read_u16() { Some(v) => v, None => return };
    let rings = match packet.read_u16() { Some(v) => v, None => return };

    let cfg = cfg();
    if cfg.states.gameplay.anticheat.data_based_anticheat {
        if rings < 10 {
            outbox.push(OutboxMsg::Disconnect(v_id, DisconnectReason::Other as u32));
            return;
        }
        if rings >= 140 && server.game.map != 20 {
            outbox.push(OutboxMsg::Disconnect(v_id, DisconnectReason::Other as u32));
            return;
        }
    }

    if let Some(pd) = server.find_peer_mut(v_id) {
        pd.plr.heal_rings = pd.plr.rings as u16;
    }

    packet.pos = 0;
    let data = packet.data().to_vec();
    outbox.push(OutboxMsg::BroadcastEx(data, true, v_id));
}

fn handle_heal(v_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }

    packet.pos = 2;
    let _id   = packet.read_u16();
    let rings = match packet.read_u16() { Some(v) => v, None => return };

    let cfg = cfg();
    if cfg.states.gameplay.anticheat.data_based_anticheat {
        if rings < 10 {
            outbox.push(OutboxMsg::Disconnect(v_id, DisconnectReason::Other as u32));
            return;
        }
        if rings >= 140 && server.game.map != 20 {
            outbox.push(OutboxMsg::Disconnect(v_id, DisconnectReason::Other as u32));
            return;
        }
    }

    if cfg.states.gameplay.anticheat.useless_anticheat.enable {
        if server.find_peer(v_id).map(|p| p.plr.mod_tool).unwrap_or(false) {
            let mut pkt = Packet::new(PacketType::SERVER_RING_COLLECTED);
            let _ = pkt.write_u8(0);
            let _ = pkt.write_u16(0);
            let _ = pkt.write_u8(1);
            let _ = pkt.write_u8(0);
            outbox.push(OutboxMsg::SendTo(v_id, pkt.data().to_vec(), true));
            return;
        }
    }

    if let Some(pd) = server.find_peer_mut(v_id) {
        pd.plr.heal_rings = 0;
        pd.plr.stats.hp_restored += 1;
        pd.plr.hp_credit = pd.plr.hp_credit.saturating_add(20).min(100);
    }

    packet.pos = 0;
    let data = packet.data().to_vec();
    outbox.push(OutboxMsg::BroadcastEx(data, true, v_id));
}

fn handle_cream_rings(v_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }
    if v_id as i32 == server.game.exe { return; }
    if !server.find_peer(v_id).map(|p| p.surv_char == SurvChar::Cream).unwrap_or(false) { return; }

    let cfg = cfg();
    if cfg.states.gameplay.anticheat.ability_anticheat {
        let cd = server.find_peer(v_id).map(|p| p.plr.cooldown).unwrap_or(1.0);
        if cd > 0.0 { return; }
        // Reject abilities used under the red/black-ring effect (client blocks
        // them; a Cheat-Engine bypass would still send the packet).
        if server.find_peer(v_id).map(|p| p.plr.red_ring).unwrap_or(false) { return; }
    }

    if cfg.states.gameplay.anticheat.useless_anticheat.enable {
        if server.find_peer(v_id).map(|p| p.plr.mod_tool).unwrap_or(false) {
            let mut pkt = Packet::new(PacketType::SERVER_RING_COLLECTED);
            let _ = pkt.write_u8(0);
            let _ = pkt.write_u16(0);
            let _ = pkt.write_u8(1);
            let _ = pkt.write_u8(0);
            outbox.push(OutboxMsg::SendTo(v_id, pkt.data().to_vec(), true));
            return;
        }
    }

    packet.pos = 2;
    let x         = match packet.read_u16() { Some(v) => v, None => return };
    let y         = match packet.read_u16() { Some(v) => v, None => return };
    let red_ring  = match packet.read_u8()  { Some(v) => v != 0, None => return };


    let v_pos = server.find_peer(v_id).map(|p| p.plr.pos).unwrap_or((0.0, 0.0));
    if vec2_dist(v_pos, (x as f32, y as f32)) > 40.0 { return; }

    if red_ring {

        let existing: Vec<(f32, f32)> = server.game.entities.iter()
            .filter(|e| e.tag() == "cring")
            .map(|e| e.pos())
            .collect();
        for pos in &existing {
            let e_red = true;


            if vec2_dist(*pos, (x as f32, y as f32)) < 150.0 { return; }
        }


        let offsets: [(i16, i16); 2] = [(25, 0), (-27, 0)];
        for &(ox, oy) in &offsets {
            let rx = (x as i32 + ox as i32) as u16;
            let ry = (y as i32 + oy as i32) as u16;
            game_spawn(server, outbox, crate::entities::cream_ring::CreamRing::new(rx as f32, ry as f32, true));
        }
        with_status(|s| { s.cream_rings_spawned += 2; });
    } else {

        let offsets: [(i16, i16); 3] = [(26, 0), (0, -26), (-27, 0)];
        for &(ox, oy) in &offsets {
            let rx = (x as i32 + ox as i32) as u16;
            let ry = (y as i32 + oy as i32) as u16;
            game_spawn(server, outbox, crate::entities::cream_ring::CreamRing::new(rx as f32, ry as f32, false));
        }
        with_status(|s| { s.cream_rings_spawned += 3; });
    }

    if cfg.states.gameplay.anticheat.ability_anticheat {
        if let Some(pd) = server.find_peer_mut(v_id) {
            pd.plr.cooldown = 25.0 * 60.0;
        }
    }
}

fn handle_stats_report(v_id: u16, packet: &mut Packet, server: &mut Server, _outbox: &mut Vec<OutboxMsg>) {
    if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }

    packet.pos = 2;
    let stat_type = match packet.read_u8() { Some(v) => v, None => return };

    match stat_type {
        0 => {
            if let Some(pd) = server.find_peer_mut(v_id) {
                pd.plr.stats.hp_restored += 1;
            }
        }
        1 => {
            let recv  = match packet.read_u16() { Some(v) => v, None => return };
            let dmgr  = match packet.read_u16() { Some(v) => v, None => return };
            let sec   = match packet.read_u8()  { Some(v) => v, None => return };
            if let Some(rec) = server.find_peer_mut(recv) {
                rec.plr.stats.stun_time += sec as u16;
            }
            if let Some(d) = server.find_peer_mut(dmgr) {
                d.plr.stats.stuns += 1;
            }
            with_status(|s| { s.total_stuns += 1; });
        }
        2 => {
            let id  = match packet.read_u16() { Some(v) => v, None => return };
            let dmg = match packet.read_u16() { Some(v) => v, None => return };
            let hp  = match packet.read_u16() { Some(v) => v, None => return };
            if let Some(d) = server.find_peer_mut(id) {
                if hp == 0 { d.plr.stats.kills += 1; }
                d.plr.stats.damage = d.plr.stats.damage.saturating_add(dmg / 20);
            }
            with_status(|s| { s.damage_taken += (dmg / 20) as u32; });
        }
        3 => {
            let dmg = match packet.read_u8() { Some(v) => v, None => return };
            if let Some(pd) = server.find_peer_mut(v_id) {
                pd.plr.stats.damage_taken = pd.plr.stats.damage_taken.saturating_add((dmg / 20) as u16);
            }
        }
        _ => {}
    }
}

fn handle_tprojectile_startcharge(v_id: u16, server: &mut Server) {

    let cfg = cfg();
    if cfg.states.gameplay.anticheat.ability_anticheat {
        if let Some(pd) = server.find_peer_mut(v_id) {
            pd.plr.tails_last_proj = Some(Instant::now());
        }
    }
}

fn handle_tprojectile(v_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }
    if v_id as i32 == server.game.exe { return; }
    if !server.find_peer(v_id).map(|p| p.surv_char == SurvChar::Tails).unwrap_or(false) { return; }

    let cfg = cfg();
    if cfg.states.gameplay.anticheat.ability_anticheat {
        let cd = server.find_peer(v_id).map(|p| p.plr.cooldown).unwrap_or(1.0);
        if cd > 0.0 { return; }
        // Reject abilities used under the red/black-ring effect (client blocks
        // them; a Cheat-Engine bypass would still send the packet).
        if server.find_peer(v_id).map(|p| p.plr.red_ring).unwrap_or(false) { return; }
    }

    packet.pos = 2;
    let x   = match packet.read_u16() { Some(v) => v, None => return };
    let y   = match packet.read_u16() { Some(v) => v, None => return };
    let dir = match packet.read_i8()  { Some(v) => v, None => return };
    let dmg = match packet.read_u8()  { Some(v) => v, None => return };
    let exe = match packet.read_u8()  { Some(v) => v, None => return };
    let chg = match packet.read_u8()  { Some(v) => v, None => return };

    if dir < -1 || dir > 1 { return; }

    let dir = if cfg.states.gameplay.anticheat.useless_anticheat.enable
        && server.find_peer(v_id).map(|p| p.plr.mod_tool).unwrap_or(false)
    {
        if dir > 0 { -1 } else if dir < 0 { 1 } else { dir }
    } else {
        dir
    };

    let is_demonized = server.find_peer(v_id)
        .map(|p| p.plr.flags & plrflags::DEMONIZED != 0)
        .unwrap_or(false);

    if cfg.states.gameplay.anticheat.ability_anticheat {
        let last_ms = server.find_peer(v_id)
            .and_then(|p| p.plr.tails_last_proj)
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0);

        if is_demonized {
            if dmg > 60 { return; }
            if dmg >= 60 && !(last_ms > 1500 && last_ms < 12000) { return; }
        } else {
            if dmg > 6 { return; }
            if dmg >= 6 && !(last_ms > 1500 && last_ms < 12000) { return; }
        }
    }

    with_status(|s| { s.tails_shots += 1; });
    game_spawn(server, outbox, crate::entities::tails_projectile::TailsProjectile::new(
        x as f32, y as f32, v_id, dir, exe != 0, chg, dmg,
    ));

    if cfg.states.gameplay.anticheat.ability_anticheat {
        if let Some(pd) = server.find_peer_mut(v_id) {
            pd.plr.cooldown = 10.0 * 60.0;
        }
    }
}

fn handle_tprojectile_hit(v_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }

    with_status(|s| { s.tails_hits += 1; });

    let eid = server.game.entities.iter().find(|e| e.tag() == "tproj").map(|e| e.id());
    if let Some(eid) = eid {
        game_despawn(server, outbox, eid);
    }
}

fn handle_etracker(v_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }
    if v_id as i32 == server.game.exe { return; }
    if !server.find_peer(v_id).map(|p| p.surv_char == SurvChar::Eggman).unwrap_or(false) { return; }

    let cfg = cfg();
    if cfg.states.gameplay.anticheat.ability_anticheat {
        let cd = server.find_peer(v_id).map(|p| p.plr.cooldown).unwrap_or(1.0);
        if cd > 0.0 { return; }
        // Reject abilities used under the red/black-ring effect (client blocks
        // them; a Cheat-Engine bypass would still send the packet).
        if server.find_peer(v_id).map(|p| p.plr.red_ring).unwrap_or(false) { return; }
    }

    packet.pos = 2;
    let x = match packet.read_u16() { Some(v) => v, None => return };
    let y = match packet.read_u16() { Some(v) => v, None => return };

    with_status(|s| { s.eggman_mines_placed += 1; });
    game_spawn(server, outbox, crate::entities::eggman_tracker::EggmanTracker::new(x as f32, y as f32));

    if let Some(pd) = server.find_peer_mut(v_id) {
        pd.plr.cooldown = 10.0 * 60.0;
    }
}

fn handle_etracker_activated(v_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }

    packet.pos = 2;
    let eid = match packet.read_u16() { Some(v) => v, None => return };

    game_eggtrack_activate(server, outbox, eid, v_id);
}

fn handle_erector_bring_spawn(v_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }
    if v_id as i32 != server.game.exe { return; }
    if !server.find_peer(v_id).map(|p| p.exe_char == ExeChar::Exetior).unwrap_or(false) { return; }

    let cfg = cfg();
    if cfg.states.gameplay.anticheat.ability_anticheat {
        let cd = server.find_peer(v_id).map(|p| p.plr.cooldown).unwrap_or(1.0);
        if cd > 0.0 { return; }
    }

    packet.pos = 2;
    let x = match packet.read_u16() { Some(v) => v, None => return };
    let y = match packet.read_u16() { Some(v) => v, None => return };


    let bring_positions: Vec<(f32, f32)> = server.game.entities.iter()
        .filter(|e| e.tag() == "bring")
        .map(|e| e.pos())
        .collect();
    for pos in &bring_positions {
        if vec2_dist(*pos, (x as f32, y as f32)) < 100.0 { return; }
    }

    if cfg.states.gameplay.anticheat.useless_anticheat.enable
        && server.find_peer(v_id).map(|p| p.plr.mod_tool).unwrap_or(false)
    {
        game_spawn(server, outbox, crate::entities::cream_ring::CreamRing::new(x as f32, y as f32, false));
        return;
    }

    with_status(|s| { s.exetior_bring_spawned += 1; });
    game_spawn(server, outbox, crate::entities::black_ring::BlackRing::new(x as f32, y as f32));

    if cfg.states.gameplay.anticheat.ability_anticheat {
        if let Some(pd) = server.find_peer_mut(v_id) {
            pd.plr.cooldown = 10.0 * 60.0;
        }
    }
}

fn handle_exeller_spawn_clone(v_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }
    if v_id as i32 != server.game.exe { return; }
    if !server.find_peer(v_id).map(|p| p.exe_char == ExeChar::Exeller).unwrap_or(false) { return; }


    let clone_count = server.game.entities.iter().filter(|e| e.tag() == "exclone").count();
    if clone_count >= 2 { return; }

    packet.pos = 2;
    let x   = match packet.read_u16() { Some(v) => v, None => return };
    let y   = match packet.read_u16() { Some(v) => v, None => return };
    let dir = match packet.read_i8()  { Some(v) => v, None => return };

    with_status(|s| { s.exeller_clones_placed += 1; });
    game_spawn(server, outbox, crate::entities::exeller_clone::ExellerClone::new(x as f32, y as f32, dir, v_id));
}

fn handle_exeller_teleport_clone(v_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }
    if v_id as i32 != server.game.exe { return; }
    if !server.find_peer(v_id).map(|p| p.exe_char == ExeChar::Exeller).unwrap_or(false) { return; }

    packet.pos = 2;
    let eid = match packet.read_u16() { Some(v) => v, None => return };

    if let Some(idx) = game_find_entity(server, eid) {
        let clone_owner = server.game.entities[idx].owner_id();
        let clone_dir   = server.game.entities[idx].dir_i8();

        if cfg().states.gameplay.anticheat.useless_anticheat.enable
            && server.find_peer(v_id).map(|p| p.plr.mod_tool).unwrap_or(false)
        {
            let plr_pos = server.find_peer(v_id).map(|p| p.plr.pos).unwrap_or((0.0, 0.0));
            game_despawn(server, outbox, eid);
            let mut pkt = Packet::new(PacketType::SERVER_EXELLERCLONE_STATE);
            let _ = pkt.write_u8(0);
            let _ = pkt.write_u16(eid);
            let _ = pkt.write_u16(clone_owner);
            let _ = pkt.write_u16(plr_pos.0 as u16);
            let _ = pkt.write_u16(plr_pos.1 as u16);
            let _ = pkt.write_i8(clone_dir);
            outbox.push(OutboxMsg::SendTo(v_id, pkt.data().to_vec(), true));
            return;
        }

        let mut pkt = Packet::new(PacketType::SERVER_EXELLERCLONE_STATE);
        let _ = pkt.write_u8(1);
        let _ = pkt.write_u16(eid);
        outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));

        game_despawn(server, outbox, eid);

        if let Some(pd) = server.find_peer_mut(v_id) {
            pd.plr.ex_teleport = 60;
        }
    }

    with_status(|s| { s.exeller_clones_activated += 1; });
}

fn handle_erector_balls(v_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }
    if v_id as i32 != server.game.exe { return; }

    packet.pos = 2;
    let x = match packet.read_f32() { Some(v) => v, None => return };
    let y = match packet.read_f32() { Some(v) => v, None => return };

    if cfg().states.gameplay.anticheat.useless_anticheat.enable
        && server.find_peer(v_id).map(|p| p.plr.mod_tool).unwrap_or(false)
    {
        for i in -3i32..3 {
            game_spawn(server, outbox, crate::entities::cream_ring::CreamRing::new(x + i as f32 * 8.0, y, false));
        }
        return;
    }

    let mut pkt = Packet::new(PacketType::CLIENT_ERECTOR_BALLS);
    let _ = pkt.write_f32(x);
    let _ = pkt.write_f32(y);
    outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));
}

fn handle_mercoin_bonus(v_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();
    if !cfg.states.gameplay.enable_achievements { return; }
    if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }
    packet.pos = 0;
    let data = packet.data().to_vec();
    outbox.push(OutboxMsg::BroadcastEx(data, true, v_id));
}

fn handle_entity_packet(
    _v_id: u16,
    _ptype: PacketType,
    _packet: &mut Packet,
    _server: &mut Server,
    _outbox: &mut Vec<OutboxMsg>,
) {

}

pub fn check_win_condition(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    game_state_check(server, outbox);
}

pub fn game_spawn<E: Entity + 'static>(
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
    mut entity: E,
) -> u16 {
    let id = server.game.entid;
    server.game.entid = server.game.entid.wrapping_add(1);
    entity.set_id(id);

    let mut rng = SmallRng::from_entropy();
    let ingame = build_ingame_snapshot(server);
    let ring_coff = server.game.ring_coff;
    let map_id = server.game.map;
    let server_id = server.id;
    let exe_id = server.game.exe;
    let game_time_sec = server.game.time_sec;
    let game_time = server.game.time;
    let mrc = if (map_id as usize) < MAP_LIST.len() { MAP_LIST[map_id as usize].ring_count } else { 0 };

    let entity_ids: Vec<u16> = server.game.entities.iter().map(|e| e.id()).collect();
    let mut ctx = EntityCtx {
        outbox,
        server_id,
        map_id,
        map_ring_count: mrc,
        rings: &mut server.game.rings,
        ring_coff,
        ingame_peers: ingame,
        entity_ids,
        exe_id,
        game_time_sec,
        game_time,
        rand: &mut rng,
        spawn_queue: Vec::new(),
    };

    let keep = entity.on_init(&mut ctx);
    if keep {
        server.game.entities.push(Box::new(entity));
    }
    id
}

pub fn game_despawn(server: &mut Server, outbox: &mut Vec<OutboxMsg>, ent_id: u16) {
    if let Some(idx) = server.game.entities.iter().position(|e| e.id() == ent_id) {
        let mut rng = SmallRng::from_entropy();
        let ingame = build_ingame_snapshot(server);
        let ring_coff = server.game.ring_coff;
        let map_id = server.game.map;
        let server_id = server.id;
        let exe_id = server.game.exe;
        let game_time_sec = server.game.time_sec;
        let game_time = server.game.time;
        let mrc = if (map_id as usize) < MAP_LIST.len() { MAP_LIST[map_id as usize].ring_count } else { 0 };

        let entity_ids: Vec<u16> = server.game.entities.iter().map(|e| e.id()).collect();
        let mut ctx = EntityCtx {
            outbox,
            server_id,
            map_id,
            map_ring_count: mrc,
            rings: &mut server.game.rings,
            ring_coff,
            ingame_peers: ingame,
            entity_ids,
            exe_id,
            game_time_sec,
            game_time,
            rand: &mut rng,
            spawn_queue: Vec::new(),
        };
        server.game.entities[idx].on_uninit(&mut ctx);
        server.game.entities.remove(idx);
    }
}

pub fn game_find_entity(server: &Server, ent_id: u16) -> Option<usize> {
    server.game.entities.iter().position(|e| e.id() == ent_id)
}

pub fn game_eggtrack_activate(server: &mut Server, outbox: &mut Vec<OutboxMsg>, eid: u16, activator: u16) {
    if let Some(idx) = server.game.entities.iter().position(|e| e.tag() == "eggtrack" && e.id() == eid) {


        server.game.entities[idx].set_activ_id(activator);
        game_despawn(server, outbox, eid);
    }
}

pub fn build_ingame_snapshot_pub(server: &Server) -> Vec<(u16, (f32, f32), u8, i8, i8, u8)> {
    build_ingame_snapshot(server)
}

pub fn with_entity_op<F>(server: &mut Server, outbox: &mut Vec<OutboxMsg>, f: F)
where
    F: FnOnce(&mut Vec<Box<dyn Entity>>, &mut EntityCtx),
{
    let mut rng = SmallRng::from_entropy();
    let ingame      = build_ingame_snapshot(server);
    let entity_ids: Vec<u16> = server.game.entities.iter().map(|e| e.id()).collect();
    let ring_coff   = server.game.ring_coff;
    let map_id      = server.game.map;
    let server_id   = server.id;
    let exe_id      = server.game.exe;
    let game_time_sec = server.game.time_sec;
    let game_time   = server.game.time;
    let mrc = if (map_id as usize) < MAP_LIST.len() { MAP_LIST[map_id as usize].ring_count } else { 0 };

    let mut ctx = EntityCtx {
        outbox,
        server_id,
        map_id,
        map_ring_count: mrc,
        rings: &mut server.game.rings,
        ring_coff,
        ingame_peers: ingame,
        entity_ids,
        exe_id,
        game_time_sec,
        game_time,
        rand: &mut rng,
        spawn_queue: Vec::new(),
    };

    f(&mut server.game.entities, &mut ctx);
}

fn build_ingame_snapshot(server: &Server) -> Vec<(u16, (f32, f32), u8, i8, i8, u8)> {
    server.peers.iter()
        .filter(|p| p.in_game)
        .map(|p| (p.id, p.plr.pos, p.plr.flags, p.surv_char as i8, p.exe_char as i8, p.op))
        .collect()
}

pub fn game_state_tick(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();


    if !server.game.started {
        server.game.start_timeout -= 1.0;
        if server.game.start_timeout <= 0.0 {
            let unready: Option<u16> = server.peers.iter()
                .find(|p| p.in_game && !p.ready)
                .map(|p| p.id);
            if let Some(id) = unready {
                log::warn!("[Server {}] Waiting for players took too long, kicking unready player id={}", server.id, id);
                outbox.push(OutboxMsg::Disconnect(id, DisconnectReason::PacketsNotRecv as u32));
            }
        }
        return;
    }


    if server.game.end > 0.0 {
        server.game.end -= 1.0;
        if server.game.end <= 0.0 {
            game_uninit(server, cfg.states.results_misc.enabled, outbox);
        }
        return;
    }


    server.game.elapsed += 1.0;

    let disable_timer = cfg.states.gameplay.banana.disable_timer;

    // gametimers_ceiling must never apply while disable_timer is on: it's a
    // reimplementation of the "No Timer" mod (README credits), whose entire
    // point is no time-based ending at all. Letting a ceiling still force
    // game_end here would hold the round hostage to an artificial cutoff
    // (and skew whatever duration Results ends up displaying) exactly
    // contrary to that. game_init's own ceiling clamp is gated on
    // !disable_timer the same way.

    let new_sec = if disable_timer {
        (server.game.elapsed / 60.0) as u16
    } else {
        server.game.time -= 1.0;
        (server.game.time / 60.0).max(0.0) as u16
    };
    if new_sec != server.game.time_sec {
        server.game.time_sec = new_sec;


        let ring_coff = server.game.ring_coff as u16;
        if ring_coff > 0 && server.game.time_sec % ring_coff == 0 {
            if cfg.states.gameplay.entities_misc.global.rings.enabled {
                game_spawn(server, outbox, crate::entities::ring::Ring::new());
            }
        }


        let mut pkt = Packet::new(PacketType::SERVER_GAME_TIME_SYNC);
        let _ = pkt.write_u16(server.game.time_sec * 60);
        outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));

        if server.game.time_sec == 0 && !disable_timer {
            let escaped = server.peers.iter()
                .filter(|p| p.in_game && p.id as i32 != server.game.exe)
                .any(|p| p.plr.flags & plrflags::ESCAPED != 0);
            if escaped {
                game_end(server, Ending::SurvWin, false, outbox);
            } else {
                game_end(server, Ending::TimeOver, false, outbox);
            }
            return;
        }
    }


    let past_sudden_death_threshold = if disable_timer {
        server.game.time_sec > cfg.states.gameplay.sudden_death_timer as u16
    } else {
        server.game.time_sec <= cfg.states.gameplay.sudden_death_timer as u16
    };

    if !server.game.sudden_death && past_sudden_death_threshold {
        server.game.sudden_death = true;
        with_status(|s| { s.timeouts += 1; });


        let mut dead_players: Vec<(u16, f64)> = server.peers.iter()
            .filter(|p| p.in_game && p.plr.flags & plrflags::DEAD != 0)
            .map(|p| (p.id, p.plr.death_timer_sec as f64 + p.plr.death_timer / 60.0))
            .collect();
        dead_players.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        log::debug!("Demonization order:");
        for (id, secs) in &dead_players {
            log::debug!("{}: {}", id, secs);
        }
        for (id, _) in dead_players {
            game_demonize(id, server, outbox);
        }
    }


    tick_players(server, outbox);


    tick_bigring(server, outbox);


    let map_idx = server.game.map as usize;
    if map_idx < MAP_LIST.len() {
        let mut ob = Vec::new();
        (MAP_LIST[map_idx].tick)(server, &mut ob);
        outbox.append(&mut ob);
    }


    tick_entities(server, outbox);
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

fn tick_players(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg     = cfg();
    let exe_id  = server.game.exe;
    let delta   = server.delta;
    let in_sd   = server.game.sudden_death;

    let exe_pos: (f32, f32) = server.find_peer(exe_id as u16)
        .map(|p| p.plr.pos)
        .unwrap_or((0.0, 0.0));

    let mut to_disconnect: Vec<u16> = Vec::new();

    for p in server.peers.iter_mut() {
        if !p.in_game { continue; }


        if let Some(last) = p.plr.last_packet {
            if last.elapsed().as_secs_f64() > 30.0 {
                to_disconnect.push(p.id);
                continue;
            }
        }


        if p.plr.cooldown > 0.0 {
            p.plr.cooldown -= 1.0;
        } else {
            p.plr.cooldown = 0.0;
        }


        if delta < 2.5 {
            p.plr.ping_timer += delta;
            p.plr.ping_total += p.plr.ping_last as f64 * delta;

            if p.plr.ping_timer >= 20.0 * 60.0 {
                let avg = p.plr.ping_total / p.plr.ping_timer;
                let limit = cfg.server_config.pairing.ping_limit as f64;
                if limit > 0.0 && avg >= limit {
                    to_disconnect.push(p.id);
                    p.plr.ping_timer = 0.0;
                    p.plr.ping_total = 0.0;
                    continue;
                }
                p.plr.ping_timer = 0.0;
                p.plr.ping_total = 0.0;
            }
        }


        if p.id as i32 != exe_id
            && p.plr.flags & plrflags::DEAD    == 0
            && p.plr.flags & plrflags::ESCAPED == 0
            && p.plr.flags & plrflags::DEMONIZED == 0
        {
            p.plr.stats.survive_time += delta;

            let dist = crate::player::vec2_dist(p.plr.pos, exe_pos);
            if dist < 300.0 {
                p.plr.stats.danger_time += delta;
            }
        }


        if p.plr.flags & plrflags::DEAD != 0
            && p.plr.flags & plrflags::CANTREVIVE == 0
            && p.plr.revival > 0.0
        {
            p.plr.revival -= 0.0025 * delta;

        }


        if p.plr.flags & plrflags::ESCAPED == 0 {
            p.plr.timeout += delta;
            if p.plr.timeout >= 4.0 * 60.0 {
                to_disconnect.push(p.id);
                continue;
            }
        }
    }

    for id in to_disconnect {
        outbox.push(OutboxMsg::Disconnect(id, DisconnectReason::AfkTimeout as u32));
    }

    {
        let map_id = server.game.map;
        let in_game_ids: Vec<u16> = server.peers.iter()
            .filter(|p| p.in_game)
            .map(|p| p.id)
            .collect();
        for id in in_game_ids {
            player_check_zone(id, map_id, server, outbox);
        }
    }

    let sudden_death  = server.game.sudden_death;
    let sd_timer      = cfg.states.gameplay.sudden_death_timer as u16;
    let time_sec      = server.game.time_sec;
    let disable_timer = cfg.states.gameplay.banana.disable_timer;

    let peer_ids: Vec<u16> = server.peers.iter()
        .filter(|p| p.in_game && p.plr.flags & plrflags::DEAD != 0
                              && p.plr.flags & plrflags::CANTREVIVE == 0
                              && p.plr.death_timer_sec > 0)
        .map(|p| p.id)
        .collect();

    for id in peer_ids {

        if sudden_death {
            game_demonize(id, server, outbox);
            continue;
        }

        let mut death_timer_sec = server.find_peer(id).map(|p| p.plr.death_timer_sec).unwrap_or(0);

        // Proximity to the exe freezes the death-timer countdown (see should_dec
        // below); proximity to an already-demonized player only halves its regen
        // rate. Checked every tick, not just on rollover, since demonized_near
        // affects the accumulation rate itself.
        let v_pos = server.find_peer(id).map(|p| p.plr.pos).unwrap_or((0.0, 0.0));
        let mut exe_near = false;
        let mut demonized_near = false;
        if cfg.states.gameplay.exe_camp_penalty {
            for other in server.peers.iter() {
                if !other.in_game { continue; }
                let is_exe = other.id as i32 == exe_id;
                let is_demonized = other.plr.flags & plrflags::DEMONIZED != 0;
                if !is_exe && !is_demonized { continue; }
                if crate::player::vec2_dist(v_pos, other.plr.pos) <= 240.0 {
                    if is_demonized {
                        demonized_near = true;
                    } else {
                        demonized_near = false;
                        exe_near = true;
                        break;
                    }
                }
            }
        }

        let time_to_sd = ticks_until_sudden_death(time_sec, sd_timer, disable_timer);

        // Catch-up sync, every tick -- not gated on this player's own ~1s
        // sub-tick rollover below, and not gated on exe_near either: this is
        // the hard ceiling sync_sudden_death_timers promises (death_timer_sec
        // can never show more time than is actually left), not just an
        // anti-camp correction, so it must apply regardless of each player's
        // own rollover phase relative to the global clock. Server-internal
        // bookkeeping only (still broadcasts the same SERVER_GAME_DEATHTIMER_TICK
        // the client already expects), not a protocol or client-visible-gameplay
        // change.
        if cfg.states.gameplay.sync_sudden_death_timers
            && time_to_sd < death_timer_sec as u16
        {
            death_timer_sec = time_to_sd as u8;
            if let Some(pd) = server.find_peer_mut(id) {
                pd.plr.death_timer_sec = death_timer_sec;
            }
            if death_timer_sec == 0 {
                game_demonize(id, server, outbox);
                continue;
            }
            let mut pkt = Packet::new(PacketType::SERVER_GAME_DEATHTIMER_TICK);
            let _ = pkt.write_u8(exe_near as u8);
            let _ = pkt.write_u16(id);
            let _ = pkt.write_u8(death_timer_sec);
            outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));
        }

        // Checks the accumulator carried over from previous ticks *before*
        // adding this tick's increment (added unconditionally at the end,
        // below) -- not accumulate-then-check. This keeps each dead player's
        // rollover tick aligned to when they actually died, rather than
        // drifting by a tick relative to players who died at a different phase.
        let death_timer_before = server.find_peer(id).map(|p| p.plr.death_timer).unwrap_or(0.0);
        if death_timer_before >= 60.0 {
            if let Some(pd) = server.find_peer_mut(id) {
                pd.plr.death_timer = 0.0;
            }

            let should_dec = !exe_near
                || (time_to_sd < death_timer_sec as u16 && cfg.states.gameplay.sync_sudden_death_timers);

            if should_dec {
                let new_sec = death_timer_sec.saturating_sub(1);
                if let Some(pd) = server.find_peer_mut(id) {
                    pd.plr.death_timer_sec = new_sec;
                }
                if new_sec == 0 {
                    game_demonize(id, server, outbox);
                    continue;
                }
            }

            let dt_sec = server.find_peer(id).map(|p| p.plr.death_timer_sec).unwrap_or(0);
            let mut pkt = Packet::new(PacketType::SERVER_GAME_DEATHTIMER_TICK);
            let _ = pkt.write_u8(exe_near as u8);
            let _ = pkt.write_u16(id);
            let _ = pkt.write_u8(dt_sec);
            outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));
        }

        let increment = if demonized_near { 0.5 } else { 1.0 };
        if let Some(pd) = server.find_peer_mut(id) {
            pd.plr.death_timer += increment;
        }
    }
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
    let mut rng = SmallRng::from_entropy();
    let mut entity_outbox: Vec<OutboxMsg> = Vec::new();

    let mut i = 0;
    while i < server.game.entities.len() {
        let ingame      = build_ingame_snapshot(server);
        let entity_ids  = server.game.entities.iter().map(|e| e.id()).collect::<Vec<_>>();
        let ring_coff   = server.game.ring_coff;
        let map_id      = server.game.map;
        let server_id   = server.id;
        let exe_id      = server.game.exe;
        let gt          = server.game.time_sec;
        let gtime       = server.game.time;
        let mrc         = if (map_id as usize) < MAP_LIST.len() { MAP_LIST[map_id as usize].ring_count } else { 0 };

        let mut ctx = EntityCtx {
            outbox: &mut entity_outbox,
            server_id,
            map_id,
            map_ring_count: mrc,
            rings: &mut server.game.rings,
            ring_coff,
            ingame_peers: ingame,
            entity_ids,
            exe_id,
            game_time_sec: gt,
            game_time: gtime,
            rand: &mut rng,
            spawn_queue: Vec::new(),
        };

        let keep = server.game.entities[i].on_tick(&mut ctx);
        let pending_spawns = std::mem::take(&mut ctx.spawn_queue);
        drop(ctx);

        let mut last_spawned_id = 0u16;
        for pending in pending_spawns {
            last_spawned_id = game_spawn_dyn(server, &mut entity_outbox, pending);
        }
        if last_spawned_id != 0 {
            server.game.entities[i].spawner_set_slug(last_spawned_id);
        }

        if !keep {
            let ingame2     = build_ingame_snapshot(server);
            let entity_ids2 = server.game.entities.iter().map(|e| e.id()).collect::<Vec<_>>();
            let mut ctx2 = EntityCtx {
                outbox: &mut entity_outbox,
                server_id,
                map_id,
                map_ring_count: mrc,
                rings: &mut server.game.rings,
                ring_coff,
                ingame_peers: ingame2,
                entity_ids: entity_ids2,
                exe_id,
                game_time_sec: gt,
                game_time: gtime,
                rand: &mut rng,
                spawn_queue: Vec::new(),
            };
            server.game.entities[i].on_uninit(&mut ctx2);
            server.game.entities.remove(i);
        } else {
            i += 1;
        }
    }

    outbox.append(&mut entity_outbox);
}

pub fn game_spawn_dyn(server: &mut Server, outbox: &mut Vec<OutboxMsg>, mut entity: Box<dyn Entity>) -> u16 {
    let id = server.game.entid;
    server.game.entid = server.game.entid.wrapping_add(1);
    entity.set_id(id);

    let mut rng = SmallRng::from_entropy();
    let ingame      = build_ingame_snapshot(server);
    let entity_ids  = server.game.entities.iter().map(|e| e.id()).collect::<Vec<_>>();
    let ring_coff   = server.game.ring_coff;
    let map_id      = server.game.map;
    let server_id   = server.id;
    let exe_id      = server.game.exe;
    let game_time_sec = server.game.time_sec;
    let game_time   = server.game.time;
    let mrc = if (map_id as usize) < MAP_LIST.len() { MAP_LIST[map_id as usize].ring_count } else { 0 };

    let mut ctx = EntityCtx {
        outbox,
        server_id,
        map_id,
        map_ring_count: mrc,
        rings: &mut server.game.rings,
        ring_coff,
        ingame_peers: ingame,
        entity_ids,
        exe_id,
        game_time_sec,
        game_time,
        rand: &mut rng,
        spawn_queue: Vec::new(),
    };

    let keep = entity.on_init(&mut ctx);
    if keep {
        server.game.entities.push(entity);
    }
    id
}
