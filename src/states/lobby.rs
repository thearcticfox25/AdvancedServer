use crate::terminal::cmd::{parse_cmd, Cmd};
use crate::colors::*;
use crate::config::cfg;
use crate::packet::{Packet, PacketType};
use crate::server::{
    broadcast_chat, send_chat, DisconnectReason, Ending, GameState, OutboxMsg, PeerData, Server,
    SurvChar, SERVER_VERSION,
};
use crate::states::game::game_end;
use crate::states::map_vote::mapvote_init;
use crate::states::NO_COUNTDOWN;
use crate::vote::{Vote, VoteState, VoteType};

pub fn lobby_init(server: &mut Server) {
    server.state = GameState::Lobby;
    server.lobby.countdown = 0.0;
    server.lobby.prac_countdown = 0.0;
    server.lobby.countdown_sec = NO_COUNTDOWN;
    server.lobby.vote = Vote::default();
    server.lobby.kick_target = None;
    server.lobby.voting_map = -1;
    server.lobby.map = -1;
    server.lobby.exe = 0;
    server.lobby.avail = [true; 6];
    server.lobby.legacy_votekick_ongoing = false;
    server.lobby.legacy_votekick_target = None;
    server.lobby.legacy_votekick_timer = 0.0;
    server.lobby.legacy_votekick_votes.clear();
    server.lobby.legacy_practice_ongoing = false;
    server.lobby.legacy_practice_votes.clear();

    for p in server.peers.iter_mut() {
        p.in_game = false;
        p.ready = false;
        p.plr = crate::player::Player::default();
        p.surv_char = SurvChar::None;
        p.exe_char = crate::server::ExeChar::None;
    }

    update_exe_chance(server);
}

pub fn lobby_broadcast_init(server: &Server, outbox: &mut Vec<OutboxMsg>) {
    let pkt = Packet::new(PacketType::SERVER_GAME_BACK_TO_LOBBY);
    outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));

    for p in server.peers.iter() {
        let mut pkt2 = Packet::new(PacketType::SERVER_LOBBY_EXE_CHANCE);
        let _ = pkt2.write_u8(p.exe_chance);
        outbox.push(OutboxMsg::SendTo(p.id, pkt2.data().to_vec(), true));
    }
}

fn update_exe_chance(server: &mut Server) {

    let last_exe = server.game.exe;
    for p in server.peers.iter_mut() {
        if p.id as i32 == last_exe {
            p.exe_chance = 1;
        } else {
            p.exe_chance = p.exe_chance.saturating_add(1);
        }
    }
}

pub fn lobby_state_join(v_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    send_lobby_player_list(v_id, server, outbox);

    if let Some(pd) = server.find_peer(v_id) {
        let nickname = pd.nickname.clone();
        let id = pd.id;
        let lobby_icon = pd.lobby_icon;
        let pet = pd.pet;
        let exe_chance = pd.exe_chance;
        let anon_mode = cfg().states.lobby_misc.anonymous_mode;

        let recipients: Vec<(u16, u8)> = server.peers.iter()
            .filter(|p| p.id != id)
            .map(|p| (p.id, p.op))
            .collect();
        for (oid, oop) in recipients {
            let anon = anon_mode && oop < 1;
            let mut pkt = Packet::new(PacketType::SERVER_PLAYER_JOINED);
            let _ = pkt.write_u16(id);
            if anon {
                let _ = pkt.write_str("anonymous");
                let _ = pkt.write_u8(0);
                let _ = pkt.write_i8(-1);
            } else {
                let _ = pkt.write_str(&nickname);
                let _ = pkt.write_u8(lobby_icon);
                let _ = pkt.write_i8(pet);
            }
            outbox.push(OutboxMsg::SendTo(oid, pkt.data().to_vec(), true));
        }

        let mut pkt2 = Packet::new(PacketType::SERVER_LOBBY_EXE_CHANCE);
        let _ = pkt2.write_u8(exe_chance);
        outbox.push(OutboxMsg::SendTo(id, pkt2.data().to_vec(), true));
    }

    check_countdown(server, outbox);
}

fn send_lobby_player_list(target: u16, server: &Server, outbox: &mut Vec<OutboxMsg>) {
    let anon_mode = cfg().states.lobby_misc.anonymous_mode;
    let target_op = server.peers.iter().find(|p| p.id == target).map(|p| p.op).unwrap_or(0);
    let anon = anon_mode && target_op < 1;
    for pd in server.peers.iter() {
        if pd.id == target {
            continue;
        }
        let mut pkt = Packet::new(PacketType::SERVER_LOBBY_PLAYER);
        let _ = pkt.write_u16(pd.id);
        let _ = pkt.write_u8(if pd.ready { 1 } else { 0 });
        if anon {
            let _ = pkt.write_str("anonymous");
            let _ = pkt.write_u8(0);
            let _ = pkt.write_i8(-1);
        } else {
            let _ = pkt.write_str(&pd.nickname);
            let _ = pkt.write_u8(pd.lobby_icon);
            let _ = pkt.write_i8(pd.pet);
        }
        outbox.push(OutboxMsg::SendTo(target, pkt.data().to_vec(), true));
    }

    if server.lobby.countdown_sec < NO_COUNTDOWN {
        let mut pkt = Packet::new(PacketType::SERVER_LOBBY_COUNTDOWN);
        let _ = pkt.write_u8(1);
        let _ = pkt.write_u8(server.lobby.countdown_sec);
        outbox.push(OutboxMsg::SendTo(target, pkt.data().to_vec(), true));
    }
}

pub fn lobby_state_left(v_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if server.lobby.vote.ongoing {
        if let Some(kick_target) = &server.lobby.kick_target {
            let target_id = kick_target.id;
            let cfg = cfg();
            if target_id == v_id && cfg.states.lobby_misc.votekick.autoban_leavers {
                let kick_secs = cfg.server_config.pairing.kick_timeout_window as u64;
                if let Some(p) = server.find_peer(v_id) {
                    let (nick, udid, ip) = (p.nickname.clone(), p.udid.clone(), p.ip.clone());
                    crate::moderation::timeout_set(&nick, &udid, &ip,
                        crate::moderation::now_unix() + kick_secs, &nick);
                }
                broadcast_chat(outbox, &format!("vote kick {}auto-succeeded~ ({}target left~)", COLOR_CYAN, COLOR_CYAN));
                let kick_id = target_id;
                server.lobby.vote.ongoing = false;
                server.lobby.kick_target = None;
                outbox.push(OutboxMsg::Disconnect(kick_id, DisconnectReason::KickedByHost as u32));
            }
        }
    }

    if server.lobby.legacy_votekick_ongoing {
        server.lobby.legacy_votekick_votes.retain(|&id| id != v_id);

        if server.lobby.legacy_votekick_target == Some(v_id) {
            broadcast_chat(outbox, &format!("{}голосование провалено{} (игрок уехал)", COLOR_RED, COLOR_RESET));
            server.lobby.legacy_votekick_votes.clear();
            server.lobby.legacy_votekick_timer = 0.0;
            server.lobby.legacy_votekick_target = None;
            server.lobby.legacy_votekick_ongoing = false;
            server.lobby.kick_target = None;
        } else {
            let target = server.lobby.legacy_votekick_target;
            let remaining_non_target = server.peers.iter()
                .filter(|p| p.id != v_id && Some(p.id) != target)
                .count();
            if server.lobby.legacy_votekick_votes.len() >= remaining_non_target {
                check_votekick_legacy(server, false, outbox);
            }
        }
    }

    if server.lobby.legacy_practice_ongoing {
        server.lobby.legacy_practice_votes.retain(|&id| id != v_id);
    }

    let mut pkt = Packet::new(PacketType::SERVER_PLAYER_LEFT);
    let _ = pkt.write_u16(v_id);
    outbox.push(OutboxMsg::BroadcastEx(pkt.data().to_vec(), true, v_id));

    check_countdown_ex(v_id, server, outbox);
}

pub fn lobby_state_handle(
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
        PacketType::CLIENT_LOBBY_PLAYERS_REQUEST => {
            send_lobby_player_list(v_id, server, outbox);
            let pkt = Packet::new(PacketType::SERVER_LOBBY_CORRECT);
            outbox.push(OutboxMsg::SendTo(v_id, pkt.data().to_vec(), true));
            send_greeting(v_id, server, outbox);
        }

        PacketType::CLIENT_CHAT_MESSAGE => {
            if !server.chat_rate_allow(v_id) { return; } // SEC-L3: anti-flood
            packet.pos = 2;
            let _sender = packet.read_u16();
            let msg = match packet.read_str() {
                Some(s) => s,
                None => return,
            };
            handle_chat(v_id, &msg, server, outbox);
        }

        PacketType::CLIENT_LOBBY_READY_STATE => {
            packet.pos = 2;
            let ready = packet.read_u8().unwrap_or(0) != 0;

            if let Some(pd) = server.find_peer_mut(v_id) {
                pd.ready = ready;
            }

            let mut pkt = Packet::new(PacketType::SERVER_LOBBY_READY_STATE);
            let _ = pkt.write_u16(v_id);
            let _ = pkt.write_u8(if ready { 1 } else { 0 });
            outbox.push(OutboxMsg::BroadcastEx(pkt.data().to_vec(), true, v_id));

            check_countdown(server, outbox);
        }

        PacketType::CLIENT_LOBBY_CHOOSEVOTEKICK => {
            packet.pos = 2;
            let target_id = match packet.read_u16() {
                Some(v) => v,
                None => return,
            };
            if cfg().states.lobby_misc.votekick.use_legacy_voting {
                handle_votekick_legacy(v_id, target_id, server, outbox);
            } else {
                handle_votekick(v_id, target_id, server, outbox);
            }
        }

        PacketType::CLIENT_LOBBY_CHOOSEBAN => {
            packet.pos = 2;
            let target_id = match packet.read_u16() {
                Some(v) => v,
                None => return,
            };
            let op = server.find_peer(v_id).map(|p| p.op).unwrap_or(0);
            if op > 0 {
                if let Some(tp) = server.find_peer(target_id) {
                    if tp.op < op {
                        let nick = tp.nickname.clone();
                        let udid = tp.udid.clone();
                        let ip = tp.ip.clone();
                        crate::moderation::ban_add(&nick, &udid, &ip, &nick);
                        outbox.push(OutboxMsg::Disconnect(
                            target_id,
                            DisconnectReason::BannedByHost as u32,
                        ));
                        broadcast_chat(outbox, &format!("\\c{} was banned.", nick));
                    }
                }
            }
        }

        PacketType::CLIENT_LOBBY_CHOOSEKICK => {
            packet.pos = 2;
            let target_id = match packet.read_u16() {
                Some(v) => v,
                None => return,
            };
            let op = server.find_peer(v_id).map(|p| p.op).unwrap_or(0);
            if op > 0 {
                let cfg = cfg();
                let kick_secs = cfg.server_config.pairing.kick_timeout_window as u64;
                if let Some(tp) = server.find_peer(target_id) {
                    if tp.op < op {
                        let nick = tp.nickname.clone();
                        let udid = tp.udid.clone();
                        let ip   = tp.ip.clone();
                        crate::moderation::timeout_set(&nick, &udid, &ip,
                            crate::moderation::now_unix() + kick_secs, &nick);
                        outbox.push(OutboxMsg::Disconnect(
                            target_id,
                            DisconnectReason::KickedByHost as u32,
                        ));
                        broadcast_chat(outbox, &format!("\\c{} was kicked.", nick));
                    }
                }
            }
        }

        PacketType::CLIENT_LOBBY_CHOOSEOP => {
            packet.pos = 2;
            let target_id = match packet.read_u16() {
                Some(v) => v,
                None => return,
            };
            let op = server.find_peer(v_id).map(|p| p.op).unwrap_or(0);
            if op >= 3 {
                if let Some(tp) = server.find_peer(target_id) {
                    let nick = tp.nickname.clone();
                    let udid = tp.udid.clone();
                    let ip = tp.ip.clone();
                    crate::moderation::op_add(&nick, &udid, &ip, &nick);
                    broadcast_chat(outbox, &format!("\\c{} was made an operator.", nick));
                }
                if let Some(tp) = server.find_peer_mut(target_id) {
                    let cfg = cfg();
                    tp.op = cfg.states.lobby_misc.moderation.op_default_level;
                }
                send_chat(outbox, target_id, &format!("{}you're an operator now", COLOR_CYAN));
            }
        }

        PacketType::CLIENT_PLAYER_PALETTE => {
            // SEC-L1: actually run the palette anti-cheat (previously the validator
            // existed but was never called). It both rejects impossible recolors and,
            // via its `id == v_id` check, blocks palette-spoofing of another player.
            if cfg().states.gameplay.anticheat.palette_anticheat {
                packet.pos = 2;
                if !crate::anticheat::palette::palette_player_validate(v_id, packet) {
                    outbox.push(OutboxMsg::Disconnect(v_id, DisconnectReason::Other as u32));
                    return;
                }
            }
            packet.pos = 0;
            let data = packet.data().to_vec();
            outbox.push(OutboxMsg::BroadcastEx(data, false, v_id));
        }

        PacketType::CLIENT_PET_PALETTE => {
            // Pet palettes are passthrough (as in upstream): the validator only knows
            // the character palette tables, so it does not apply here.
            packet.pos = 0;
            let data = packet.data().to_vec();
            outbox.push(OutboxMsg::BroadcastEx(data, false, v_id));
        }

        PacketType::CLIENT_PING => {
            let mut pkt = Packet::new(PacketType::SERVER_PONG);
            outbox.push(OutboxMsg::SendTo(v_id, pkt.data().to_vec(), false));
        }

        _ => {}
    }
}

pub fn handle_console_cmd(line: &str, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return;
    }
    let input = if trimmed.starts_with(':') || trimmed.starts_with('.') {
        trimmed.to_string()
    } else {
        format!(":{}", trimmed)
    };
    let cmd = match parse_cmd(&input) {
        Some(c) => c,
        None => return,
    };
    match cmd.name.as_str() {
        "ban" => {
            let tid = find_player_by_name_or_id(cmd.arg(0), server, 0);
            if let Some(tid) = tid {
                if let Some(tp) = server.find_peer(tid) {
                    let (nick, udid, ip) = (tp.nickname.clone(), tp.udid.clone(), tp.ip.clone());
                    crate::moderation::ban_add(&nick, &udid, &ip, &nick);
                    outbox.push(OutboxMsg::Disconnect(tid, DisconnectReason::BannedByHost as u32));
                    log::info!("{} was banned.", nick);
                }
            } else {
                log::info!("player not found.");
            }
        }
        "kick" => {
            let tid = find_player_by_name_or_id(cmd.arg(0), server, 0);
            if let Some(tid) = tid {
                if let Some(tp) = server.find_peer(tid) {
                    let nick = tp.nickname.clone();
                    let udid = tp.udid.clone();
                    let ip   = tp.ip.clone();
                    let kick_secs = cfg().server_config.pairing.kick_timeout_window as u64;
                    crate::moderation::timeout_set(&nick, &udid, &ip,
                        crate::moderation::now_unix() + kick_secs, &nick);
                    outbox.push(OutboxMsg::Disconnect(tid, DisconnectReason::KickedByHost as u32));
                    log::info!("{} was kicked.", nick);
                }
            } else {
                log::info!("player not found.");
            }
        }
        "op" => {
            let tid = find_player_by_name_or_id(cmd.arg(0), server, 0);
            if let Some(tid) = tid {
                if let Some(tp) = server.find_peer(tid) {
                    let (nick, tudid, tip) = (tp.nickname.clone(), tp.udid.clone(), tp.ip.clone());
                    crate::moderation::op_add(&nick, &tudid, &tip, &nick);
                    send_chat(outbox, tid, &format!("{}you're an operator now", COLOR_CYAN));
                    log::info!("{} was made an operator.", nick);
                }
                if let Some(tp) = server.find_peer_mut(tid) {
                    tp.op = cfg().states.lobby_misc.moderation.op_default_level;
                }
            } else {
                log::info!("player not found.");
            }
        }
        "kick_now" => {
            if let Ok(tid) = cmd.arg(0).parse::<u16>() {
                outbox.push(OutboxMsg::Disconnect(tid, DisconnectReason::KickedByHost as u32));
            }
        }
        "ban_now" => {
            if let Ok(tid) = cmd.arg(0).parse::<u16>() {
                outbox.push(OutboxMsg::Disconnect(tid, DisconnectReason::BannedByHost as u32));
            }
        }
        "set_op_pid" => {
            if let (Ok(tid), Ok(level)) = (cmd.arg(0).parse::<u16>(), cmd.arg(1).parse::<u8>()) {
                if let Some(p) = server.find_peer_mut(tid) {
                    p.op = level;
                }
                let shy = cfg().states.lobby_misc.moderation.shy_mode;
                if level > 0 {
                    send_chat(outbox, tid, &format!("{}you're an operator now", COLOR_CYAN));
                } else if !shy {
                    send_chat(outbox, tid, &format!("{}you're no longer an operator", COLOR_RED));
                }
            }
        }
        _ => {
            exec_cmd(&cmd, 0, u8::MAX, server, outbox);
        }
    }
}

fn handle_chat(v_id: u16, msg: &str, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let trimmed = msg.trim();

    if trimmed.starts_with(':') || trimmed.starts_with('.') {
        if let Some(cmd) = parse_cmd(trimmed) {
            if cmd.name == "stop" {
                return;
            }
            let op = server.find_peer(v_id).map(|p| p.op).unwrap_or(0);
            let handled = exec_cmd(&cmd, v_id, op, server, outbox);
            if handled {
                return;
            }
        }
    }

    if let Some(pd) = server.find_peer(v_id) {
        let id = pd.id;
        let mut pkt = Packet::new(PacketType::CLIENT_CHAT_MESSAGE);
        let _ = pkt.write_u16(id);
        let _ = pkt.write_str(msg);
        outbox.push(OutboxMsg::BroadcastEx(pkt.data().to_vec(), true, id));
    }
}

pub fn send_greeting(v_id: u16, server: &Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();
    let sc = cfg.server_config.networking.server_count;
    if sc >= 2 {
        send_chat(outbox, v_id, &cfg.states.lobby_misc.upper_bracket);
        send_chat(outbox, v_id, &format!("hosted by {}{}{}", COLOR_PURPUR, cfg.states.lobby_misc.hosts_name, COLOR_RESET));
        send_chat(outbox, v_id, &format!("server {}{}{} of {}{}{}", COLOR_RED, server.id + 1, COLOR_RESET, COLOR_BLUE, sc, COLOR_RESET));
        send_chat(outbox, v_id, &cfg.states.lobby_misc.lower_bracket);
    }
    send_chat(outbox, v_id, &format!("{}type .help for command list{}", COLOR_GRAY, COLOR_RESET));
    let loc = &cfg.states.lobby_misc.server_location;
    let ping_limit = cfg.server_config.pairing.ping_limit;
    if !loc.is_empty() && ping_limit != u16::MAX {
        send_chat(outbox, v_id, &format!("{}, required ping: {} or less", loc, ping_limit));
    }
    let motd = cfg.states.lobby_misc.message_of_the_day.clone();
    if !motd.is_empty() {
        send_chat(outbox, v_id, &motd);
    }
    if cfg.states.lobby_misc.authoritarian_mode {
        send_chat(outbox, v_id, &format!("{}authoritarian mode is on", COLOR_RED));
    }
    let op = server.find_peer(v_id).map(|p| p.op).unwrap_or(0);
    match op {
        1 => send_chat(outbox, v_id, &format!("{}you're an operator on this server{}", COLOR_CYAN, COLOR_RESET)),
        2 => send_chat(outbox, v_id, &format!("{}you're a moderator on this server{}", COLOR_CYAN, COLOR_RESET)),
        3 => send_chat(outbox, v_id, &format!("{}you've got root perms on this server{}", COLOR_CYAN, COLOR_RESET)),
        _ => {}
    }
}

pub fn exec_cmd_pub(
    cmd: &Cmd,
    v_id: u16,
    op: u8,
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
) -> bool {
    match cmd.name.as_str() {
        "help" | "info" | "credits" | "source" | "stink" | "lobby"
        | "kick" | "ban" | "op" | "status" | "stop" => {
            exec_cmd(cmd, v_id, op, server, outbox)
        }
        _ => false,
    }
}

fn exec_cmd(
    cmd: &Cmd,
    v_id: u16,
    op: u8,
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
) -> bool {
    let cfg = cfg();

    match cmd.name.as_str() {
        "help" => {
            let page_str = cmd.arg(0);
            let page: i32 = page_str.parse().unwrap_or(-1);
            if page < 1 {
                send_chat(outbox, v_id, &format!("{}example:~ .help 1", COLOR_RED));
                return true;
            }
            send_chat(outbox, v_id, &format!("{}help {}page {}{}", COLOR_YELLOW, COLOR_CYAN, page, COLOR_RESET));
            let sc = cfg.server_config.networking.server_count;
            let mc = crate::maps::MAP_COUNT;
            match page {
                1 => {
                    if sc >= 2 {
                        send_chat(outbox, v_id, &format!("{}.lobby{} choose lobby (1-{})", COLOR_GRAY, COLOR_RESET, sc));
                    }
                    send_chat(outbox, v_id, &format!("{}.info{} server info", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, v_id, &format!("{}:credits{} project credits", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, v_id, &format!("{}:source{} source code link", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, v_id, &format!("{}:status{} server statistics", COLOR_GRAY, COLOR_RESET));
                }
                2 => {
                    send_chat(outbox, v_id, &format!("{}.kick{} kick a player", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, v_id, &format!("{}.ban{} ban a player", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, v_id, &format!("{}.op{} make a player an admin", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, v_id, &format!("{}:stop{} force-end the current round", COLOR_GRAY, COLOR_RESET));
                }
                3 => {
                    send_chat(outbox, v_id, &format!("{}.vk{} vote kick", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, v_id, &format!("{}.vp{} vote practice", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, v_id, &format!("{}:vm{} vote specific map", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, v_id, &format!("{}.y{} vote for", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, v_id, &format!("{}.yes{} vote for", COLOR_GRAY, COLOR_RESET));
                    if cfg.states.lobby_misc.votekick.use_legacy_voting {
                        send_chat(outbox, v_id, &format!("{}.n{} vote against", COLOR_GRAY, COLOR_RESET));
                        send_chat(outbox, v_id, &format!("{}.no{} vote against", COLOR_GRAY, COLOR_RESET));
                        send_chat(outbox, v_id, &format!("{}.practice{} vote for practice", COLOR_GRAY, COLOR_RESET));
                    }
                }
                4 => {
                    send_chat(outbox, v_id, &format!("{}.m{} mute the chat (local command)", COLOR_GRAY, COLOR_RESET));
                }
                5 => {
                    send_chat(outbox, v_id, &format!("{}:chance{} change your exe chance", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, v_id, &format!("{}:stop{} force end round", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, v_id, &format!("{}.map{} choose map (1-{})", COLOR_GRAY, COLOR_RESET, mc));
                    send_chat(outbox, v_id, &format!("{}:start{} force start round", COLOR_GRAY, COLOR_RESET));
                }
                _ => {
                    send_chat(outbox, v_id, "void");
                }
            }
        }

        "info" => {
            send_chat(outbox, v_id, &format!("-----{}advanced{}server{}-----", COLOR_RED, COLOR_BLUE, COLOR_RESET));
            send_chat(outbox, v_id, &format!("version: {}{}{}", COLOR_BLUE, SERVER_VERSION, COLOR_RESET));
            send_chat(outbox, v_id, &format!(
                "built on {}{}{} at {}{}{} for {}{}{} via {}{}",
                COLOR_PURPUR, env!("BUILD_DATE"), COLOR_RESET,
                COLOR_CYAN, env!("BUILD_TIME"), COLOR_RESET,
                COLOR_RED, env!("BUILD_TARGET_OS"), COLOR_RESET,
                COLOR_YELLOW, env!("BUILD_RUSTC_VERSION"),
            ));
            send_chat(outbox, v_id, &format!("{}(c) {}2023-2026 {}the arctic fox{}", COLOR_CYAN, COLOR_BLUE, COLOR_RED, COLOR_RESET));
            send_chat(outbox, v_id, "------------------------");
        }

        "credits" => {
            send_chat(outbox, v_id, &format!("-----{}credits{}-----", COLOR_YELLOW, COLOR_RESET));
            send_chat(outbox, v_id, &format!("created by: {}the arctic fox{}", COLOR_YELLOW, COLOR_RESET));
            send_chat(outbox, v_id, &format!("contains code referenced from: {}betterserver-oss{}", COLOR_PURPUR, COLOR_RESET));
            send_chat(outbox, v_id, &format!("{}BetterServer-OSS{} (MIT) by {}Team EXE Empire{}", COLOR_CYAN, COLOR_RESET, COLOR_PURPUR, COLOR_RESET));
            if cfg.states.gameplay.gmcycle.overhell {
                send_chat(outbox, v_id, &format!("{}Gamemode Cycle{} by {}IcedCoffee & ColdestTea{}", COLOR_CYAN, COLOR_RESET, COLOR_PURPUR, COLOR_RESET));
            }
            if cfg.states.gameplay.banana.disable_timer {
                send_chat(outbox, v_id, &format!("{}TD2DR No Timer{} by {}Banana8870{}", COLOR_CYAN, COLOR_RESET, COLOR_PURPUR, COLOR_RESET));
            }
            send_chat(outbox, v_id, "------------------------");
        }

        "source" => {
            send_chat(outbox, v_id, &format!("{}source code:{}", COLOR_YELLOW, COLOR_RESET));
            send_chat(outbox, v_id, "https:??github.com?thearcticfox25?AdvancedServer");
            send_chat(outbox, v_id, &format!("license: {}GNU AGPL v3{}", COLOR_CYAN, COLOR_RESET));
        }

        "stink" => {
            let name = cmd.arg(0);
            if name.is_empty() {
                send_chat(outbox, v_id, &format!("{}example:~ .stink baller", COLOR_RED));
            } else {
                broadcast_chat(outbox, &format!("{}{}~, you {}sti{}nk~", COLOR_RED, name, COLOR_BLUE, COLOR_CYAN));
            }
        }

        "lobby" => {
            let sc = cfg.server_config.networking.server_count;
            if sc <= 1 {
                return true;
            }
            let ind_str = cmd.arg(0);
            let ind: i32 = match ind_str.parse() {
                Ok(v) => v,
                Err(_) => {
                    send_chat(outbox, v_id, &format!("{}example:~ .lobby 1", COLOR_RED));
                    return true;
                }
            };
            if ind < 1 || ind as u16 > sc {
                send_chat(outbox, v_id, &format!("{}lobby should be between 1 and {}", COLOR_RED, sc));
                return true;
            }
            let port = cfg.server_config.networking.port as u32 + ind as u32 - 1;
            if let Some(pd) = server.find_peer_mut(v_id) {
                pd.should_timeout = false;
            }
            let mut pkt = Packet::new(PacketType::SERVER_LOBBY_CHANGELOBBY);
            let _ = pkt.write_u32(port);
            outbox.push(OutboxMsg::SendTo(v_id, pkt.data().to_vec(), true));
        }

        "yes" | "y" => {
            if cfg.states.lobby_misc.votekick.use_legacy_voting {
                handle_vote_yes_legacy(v_id, server, outbox);
            } else {
                handle_vote_yes(v_id, server, outbox);
            }
        }

        "no" | "n" => {
            if cfg.states.lobby_misc.votekick.use_legacy_voting {
                handle_vote_no_legacy(v_id, server, outbox);
            } else {
                return false;
            }
        }

        "practice" | "p" => {
            if !cfg.states.lobby_misc.votekick.use_legacy_voting {
                return false;
            }
            if cfg.states.lobby_misc.authoritarian_mode || cfg.states.lobby_misc.anonymous_mode {
                return true;
            }
            handle_practice_legacy(v_id, server, outbox);
        }

        "vk" => {
            if cfg.states.lobby_misc.authoritarian_mode || cfg.states.lobby_misc.anonymous_mode {
                return true;
            }
            let ongoing = if cfg.states.lobby_misc.votekick.use_legacy_voting {
                server.lobby.legacy_votekick_ongoing
            } else {
                server.lobby.vote.ongoing
            };
            if ongoing {
                send_chat(outbox, v_id, &format!("{}another vote is already in progress.", COLOR_RED));
                return true;
            }
            if !cfg.states.lobby_misc.votekick.use_legacy_voting {
                let cooldown = server.find_peer(v_id).map(|p| p.vote_cooldown).unwrap_or(0.0);
                if cooldown > 0.0 {
                    send_chat(outbox, v_id, &format!("{}you cannot start another vote for {}s", COLOR_RED, (cooldown / 60.0) as i32));
                    return true;
                }
            }
            let ingame = server.total_count();
            if ingame > 2 {
                let mut pkt = Packet::new(PacketType::SERVER_LOBBY_CHOOSEVOTEKICK);
                outbox.push(OutboxMsg::SendTo(v_id, pkt.data().to_vec(), true));
            } else {
                send_chat(outbox, v_id, &format!("{}not enough participants.", COLOR_RED));
            }
        }

        "vp" => {
            if cfg.states.lobby_misc.authoritarian_mode || cfg.states.lobby_misc.anonymous_mode {
                return true;
            }
            if cfg.states.lobby_misc.votekick.use_legacy_voting {
                handle_practice_legacy(v_id, server, outbox);
            } else {
                handle_map_vote(v_id, Some(20), server, outbox);
            }
        }

        "vm" => {
            if cfg.states.lobby_misc.authoritarian_mode || cfg.states.lobby_misc.anonymous_mode {
                return true;
            }
            let ind_str = cmd.arg(0);
            let ind: i32 = match ind_str.parse::<i32>() {
                Ok(v) => v,
                Err(_) => {
                    send_chat(outbox, v_id, &format!("{}example:~ :vm 1", COLOR_RED));
                    return true;
                }
            };
            let mc = crate::maps::MAP_COUNT as i32;
            if ind < 1 || ind > mc {
                send_chat(outbox, v_id, &format!("{}map should be between 1 and {}", COLOR_RED, mc));
                return true;
            }
            handle_map_vote(v_id, Some((ind - 1) as usize), server, outbox);
        }

        "status" => {
            let page_str = cmd.arg(0);
            let page: i32 = page_str.parse().unwrap_or(-1);
            if page < 1 {
                send_chat(outbox, v_id, &format!("{}example:~ :status 1", COLOR_RED));
                return true;
            }
            send_chat(outbox, v_id, &format!("{}status {}page {}{}", COLOR_YELLOW, COLOR_CYAN, page, COLOR_RESET));
            crate::status::with_status(|s| {
                match page {
                    1 => {
                        send_chat(outbox, v_id, &format!("game stats: {}{}~:{}{}~:{}{}~ (spoiled: {})", COLOR_CYAN, s.surv_win_rounds, COLOR_RED, s.exe_win_rounds, COLOR_GRAY, s.draw_rounds, s.exe_crashed_rounds));
                    }
                    2 => {
                        send_chat(outbox, v_id, &format!("tails shots and hits: {}{}~:{}{}~", COLOR_RED, s.tails_shots, COLOR_CYAN, s.tails_hits));
                        send_chat(outbox, v_id, &format!("cream rings spawned: {}{}~", COLOR_CYAN, s.cream_rings_spawned));
                        send_chat(outbox, v_id, &format!("eggman mines planted: {}{}~", COLOR_CYAN, s.eggman_mines_placed));
                        send_chat(outbox, v_id, &format!("total stuns: {}", s.total_stuns));
                    }
                    3 => {
                        send_chat(outbox, v_id, &format!("exeller clone {}placed: {}~, {}activated: {}~", COLOR_RED, s.exeller_clones_placed, COLOR_CYAN, s.exeller_clones_activated));
                        send_chat(outbox, v_id, &format!("exetior bring spawn: {}~", s.exetior_bring_spawned));
                        send_chat(outbox, v_id, &format!("damage dealt: {}", s.damage_taken));
                    }
                    4 => {
                        send_chat(outbox, v_id, &format!("{} left during games overall", s.timeouts));
                    }
                    _ => {
                        send_chat(outbox, v_id, "void");
                    }
                }
            });
        }

        "map" => {
            if op < 1 {
                send_chat(outbox, v_id, &format!("{}your permission level is too low", COLOR_RED));
                return true;
            }
            let ind_str = cmd.arg(0);
            let ind: i32 = match ind_str.parse::<i32>() {
                Ok(v) => v,
                Err(_) => {
                    send_chat(outbox, v_id, &format!("{}example:~ .map 1", COLOR_RED));
                    return true;
                }
            };
            let mc = crate::maps::MAP_COUNT as i32;
            if ind < 1 || ind > mc {
                send_chat(outbox, v_id, &format!("{}map should be between 1 and {}", COLOR_RED, mc));
                return true;
            }
            let map = (ind - 1) as i8;
            server.lobby.map = map;
            server.last_map = map;
            for p in server.peers.iter_mut() {
                p.in_game = true;
            }
            crate::states::char_select::charselect_init(server, outbox);
        }

        "kick" => {
            if op < 2 {
                send_chat(outbox, v_id, &format!("{}your permission level is too low", COLOR_RED));
                return true;
            }
            if server.total_count() <= 1 {
                send_chat(outbox, v_id, &format!("{}dude are you gonna kick yourself?", COLOR_RED));
                return true;
            }
            let mut pkt = Packet::new(PacketType::SERVER_LOBBY_CHOOSEKICK);
            outbox.push(OutboxMsg::SendTo(v_id, pkt.data().to_vec(), true));
        }

        "ban" => {
            if op < 2 {
                send_chat(outbox, v_id, &format!("{}your permission level is too low", COLOR_RED));
                return true;
            }
            if server.total_count() <= 1 {
                send_chat(outbox, v_id, &format!("{}dude are you gonna ban yourself?", COLOR_RED));
                return true;
            }
            let mut pkt = Packet::new(PacketType::SERVER_LOBBY_CHOOSEBAN);
            outbox.push(OutboxMsg::SendTo(v_id, pkt.data().to_vec(), true));
        }

        "op" => {
            if op < 3 {
                send_chat(outbox, v_id, &format!("{}your permission level is too low", COLOR_RED));
                return true;
            }
            if server.total_count() <= 1 {
                send_chat(outbox, v_id, &format!("{}you're already an operator tho??", COLOR_RED));
                return true;
            }
            let mut pkt = Packet::new(PacketType::SERVER_LOBBY_CHOOSEOP);
            outbox.push(OutboxMsg::SendTo(v_id, pkt.data().to_vec(), true));
        }

        "stop" => {
            if op < 2 {
                send_chat(outbox, v_id, &format!("{}your permission level is too low", COLOR_RED));
                return true;
            }
            if server.state == GameState::Game && server.game.end == 0.0 {

                game_end(server, Ending::ExeWin, false, outbox);
            } else if server.state != GameState::Lobby {
                let mut ob: Vec<OutboxMsg> = Vec::new();
                lobby_init(server);
                lobby_broadcast_init(server, &mut ob);
                outbox.append(&mut ob);
            }
        }

        "start" => {
            if op < 2 {
                send_chat(outbox, v_id, &format!("{}your permission level is too low", COLOR_RED));
                return true;
            }
            mapvote_init(server, outbox);
        }

        "chance" => {
            if op < 1 {
                send_chat(outbox, v_id, &format!("{}your permission level is too low", COLOR_RED));
                return true;
            }
            let val_str = cmd.arg(0);
            let val: i32 = match val_str.parse() {
                Ok(v) => v,
                Err(_) => {
                    send_chat(outbox, v_id, &format!("{}example:~ :chance 74", COLOR_RED));
                    return true;
                }
            };
            if val < 0 || val > 100 {
                send_chat(outbox, v_id, "chance should be between 0 and 100");
                return true;
            }
            if let Some(pd) = server.find_peer_mut(v_id) {
                pd.exe_chance = val as u8;
            }
            let mut pkt = Packet::new(PacketType::SERVER_LOBBY_EXE_CHANCE);
            let _ = pkt.write_u8(val as u8);
            outbox.push(OutboxMsg::SendTo(v_id, pkt.data().to_vec(), true));
            send_chat(outbox, v_id, &format!("{}cheat code activated.{}", COLOR_CYAN, COLOR_RESET));
        }

        _ => {
            return false;
        }
    }

    true
}

fn handle_map_vote(v_id: u16, map_idx: Option<usize>, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();

    if server.lobby.vote.ongoing {
        send_chat(outbox, v_id, &format!("{}another vote is already in progress.", COLOR_RED));
        return;
    }

    let cooldown = server.find_peer(v_id).map(|p| p.vote_cooldown).unwrap_or(0.0);
    if cooldown > 0.0 {
        send_chat(outbox, v_id, &format!("{}you cannot start another vote for {} s", COLOR_RED, (cooldown / 60.0) as i32));
        return;
    }

    let ids: Vec<u16> = server.peers.iter().map(|p| p.id).collect();
    if !server.lobby.vote.init(&ids, 0, VoteType::Map) {
        send_chat(outbox, v_id, &format!("{}not enough participants.", COLOR_RED));
        return;
    }

    let voting_map = map_idx.unwrap_or(20).min(crate::maps::MAP_COUNT - 1);
    server.lobby.voting_map = voting_map as i8;

    let map_name = crate::maps::MAP_LIST[voting_map].name;
    let nick = server.find_peer(v_id).map(|p| p.nickname.clone()).unwrap_or_default();

    broadcast_chat(outbox, "-----------------------");
    broadcast_chat(outbox, &format!("{}~ {}started map vote for {}{}.", nick, COLOR_YELLOW, map_name, COLOR_RESET));
    broadcast_chat(outbox, &format!("type {}.yes~ or ignore", COLOR_CYAN));
    broadcast_chat(outbox, &format!("results will be summarized in {}{}~ sec", COLOR_GRAY, cfg.states.lobby_misc.votekick.cooldown));
    broadcast_chat(outbox, "-----------------------");

    server.lobby.vote.add(v_id);
    if let Some(pd) = server.find_peer_mut(v_id) {
        pd.vote_cooldown = cfg.states.lobby_misc.votekick.cooldown as f64 * 60.0;
    }
}

fn handle_votekick(
    v_id: u16,
    target_id: u16,
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
) {
    let cfg = cfg();

    if server.lobby.vote.ongoing {
        if let Some(kt) = &server.lobby.kick_target {
            if kt.id == target_id {

            } else {
                send_chat(outbox, v_id, &format!("{}another vote is already in progress.", COLOR_RED));
                return;
            }
        } else {
            send_chat(outbox, v_id, &format!("{}another vote is already in progress.", COLOR_RED));
            return;
        }

        let voter_nick = server.find_peer(v_id).map(|p| p.nickname.clone()).unwrap_or_default();
        match server.lobby.vote.add(v_id) {
            VoteState::AlreadyVoted => {
                send_chat(outbox, v_id, &format!("{}you have already voted.", COLOR_RED));
            }
            VoteState::Success => {
                broadcast_chat(outbox, &format!("{}~ {}voted~.", voter_nick, COLOR_CYAN));
            }
            VoteState::Full => {
                check_vote_result(server, outbox);
            }
        }
        return;
    }


    if let Some(tp) = server.find_peer(target_id) {
        if tp.op >= 2 {
            send_chat(outbox, v_id, &format!("{}you're permissionless", COLOR_RED));
            return;
        }
    } else {
        send_chat(outbox, v_id, &format!("{}specified player not found.", COLOR_RED));
        return;
    }

    let cooldown = server.find_peer(v_id).map(|p| p.vote_cooldown).unwrap_or(0.0);
    if cooldown > 0.0 {
        send_chat(outbox, v_id, &format!("{}you cannot start another vote for {}s", COLOR_RED, (cooldown / 60.0) as i32));
        return;
    }

    let ids: Vec<u16> = server.peers.iter().map(|p| p.id).collect();
    if !server.lobby.vote.init(&ids, target_id, VoteType::Kick) {
        send_chat(outbox, v_id, &format!("{}not enough participants.", COLOR_RED));
        return;
    }

    server.lobby.kick_target = server.find_peer(target_id).map(clone_peer_data);

    server.lobby.vote.add(v_id);

    if let Some(pd) = server.find_peer_mut(v_id) {
        pd.vote_cooldown = cfg.states.lobby_misc.votekick.cooldown as f64 * 60.0;
    }

    let voter_nick = server.find_peer(v_id).map(|p| p.nickname.clone()).unwrap_or_default();
    let target_nick = server.find_peer(target_id).map(|p| p.nickname.clone()).unwrap_or_default();

    broadcast_chat(outbox, "-----------------------");
    broadcast_chat(outbox, &format!("{}~ {}started kick vote for {}{}~", voter_nick, COLOR_RED, COLOR_RESET, target_nick));
    broadcast_chat(outbox, &format!("type {}.yes~ or ignore", COLOR_CYAN));
    broadcast_chat(outbox, &format!("results will be summarized in {}{}~ sec", COLOR_GRAY, cfg.states.lobby_misc.votekick.cooldown));
    broadcast_chat(outbox, "-----------------------");
}

fn handle_vote_yes(v_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !server.lobby.vote.ongoing {
        return;
    }

    let can_vote = server.find_peer(v_id).map(|p| p.can_vote).unwrap_or(false);
    if !can_vote {
        send_chat(outbox, v_id, &format!("{}you can't participate in this vote.", COLOR_RED));
        return;
    }

    if let Some(kt) = &server.lobby.kick_target {
        if kt.id == v_id {
            send_chat(outbox, v_id, &format!("{}why are you kicking yourself ???", COLOR_RED));
            return;
        }
    }

    let voter_nick = server.find_peer(v_id).map(|p| p.nickname.clone()).unwrap_or_default();
    match server.lobby.vote.add(v_id) {
        VoteState::AlreadyVoted => {
            send_chat(outbox, v_id, &format!("{}you have already voted.", COLOR_RED));
        }
        VoteState::Success => {
            broadcast_chat(outbox, &format!("{}~ {}voted~.", voter_nick, COLOR_CYAN));
        }
        VoteState::Full => {
            check_vote_result(server, outbox);
        }
    }
}

fn check_vote_result(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();
    let cnt = server.lobby.vote.votes.len() as u8;
    let tot = server.lobby.vote.votetotal;
    let vtype = server.lobby.vote.vote_type;

    let delay_ticks = cfg.states.lobby_misc.votekick.vote_result_delay as f64 * 60.0;
    if server.lobby.vote.succeeded() {
        match vtype {
            VoteType::Kick => {
                broadcast_chat(outbox, &format!("vote kick {}succeeded~ ({}{}~ out of {}{}~)", COLOR_CYAN, COLOR_CYAN, cnt, COLOR_CYAN, tot));
                server.lobby.vote.ongoing = false;

                if let Some(kt) = &server.lobby.kick_target {
                    let kick_secs = cfg.server_config.pairing.kick_timeout_window as u64;
                    if cfg.states.lobby_misc.votekick.autoban_leavers {
                        crate::moderation::ban_add(&kt.nickname, &kt.udid, &kt.ip, &kt.nickname);
                    }
                    crate::moderation::timeout_set(&kt.nickname, &kt.udid, &kt.ip,
                        crate::moderation::now_unix() + kick_secs, &kt.nickname);
                }

                server.lobby.prac_countdown = delay_ticks;
            }
            VoteType::Map => {
                broadcast_chat(outbox, &format!("map vote {}succeeded~ ({}{}~ out of {}{}~)", COLOR_CYAN, COLOR_CYAN, cnt, COLOR_CYAN, tot));
                server.lobby.vote.ongoing = false;
                server.lobby.prac_countdown = delay_ticks;

            }
        }
    } else {
        let label = if vtype == VoteType::Kick { "vote kick" } else { "map vote" };
        broadcast_chat(outbox, &format!("{} {}failed~ ({}{}~ out of {}{}~)", label, COLOR_RED, COLOR_RED, cnt, COLOR_RED, tot));
        server.lobby.vote.ongoing = false;
        server.lobby.kick_target = None;
    }
}

fn clone_peer_data(tp: &PeerData) -> PeerData {
    PeerData {
        id: tp.id,
        ip: tp.ip.clone(),
        plr: crate::player::Player::default(),
        nickname: tp.nickname.clone(),
        udid: tp.udid.clone(),
        lobby_icon: tp.lobby_icon,
        pet: tp.pet,
        verified: tp.verified,
        in_game: tp.in_game,
        op: tp.op,
        ready: tp.ready,
        mod_tool: tp.mod_tool,
        is_mobile: tp.is_mobile,
        auth: tp.auth.clone(),
        can_vote: tp.can_vote,
        voted: tp.voted,
        disconnecting: tp.disconnecting,
        surv_char: tp.surv_char,
        exe_char: tp.exe_char,
        should_timeout: tp.should_timeout,
        exe_chance: tp.exe_chance,
        timeout: tp.timeout,
        vote_cooldown: tp.vote_cooldown,
        rtt: tp.rtt,
        chat_tokens: tp.chat_tokens,
        chat_last: tp.chat_last,
    }
}

// --- legacy (v1.0.0 C#) voting system -------------------------------------
//
// Ported from AdvancedServer-100-archive/State/Lobby.cs. Unlike the current
// (v1.1.0.1) majority-vote engine, the legacy votekick requires unanimous
// agreement from everyone but the target to resolve early, has an explicit
// .n/.no "retract my yes vote" command, and a fixed 15s (900 ticks @ 60tps)
// timer after which the vote passes if yes-votes >= players who haven't
// voted yet. Legacy votepractice has no timer or majority at all: every
// single player must send .practice/.p before it instantly (no delay) sends
// everyone to the Fart Zone practice map.

const LEGACY_VOTEKICK_TIMER_TICKS: f64 = 900.0;

fn handle_votekick_legacy(v_id: u16, target_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if server.lobby.legacy_votekick_ongoing {
        if server.lobby.legacy_votekick_target == Some(target_id) {
            if !server.lobby.legacy_votekick_votes.contains(&v_id) {
                server.lobby.legacy_votekick_votes.push(v_id);
                let nick = server.find_peer(v_id).map(|p| p.nickname.clone()).unwrap_or_default();
                broadcast_chat(outbox, &format!("{} голосует {}за{}", nick, COLOR_CYAN, COLOR_RESET));
                check_votekick_legacy_early(server, outbox);
            }
        } else {
            send_chat(outbox, v_id, &format!("{}голосование в процессе!", COLOR_RED));
        }
        return;
    }

    match server.find_peer(target_id) {
        Some(tp) if tp.op >= 2 => {
            send_chat(outbox, v_id, &format!("{}you're permissionless", COLOR_RED));
            return;
        }
        None => {
            send_chat(outbox, v_id, &format!("{}specified player not found.", COLOR_RED));
            return;
        }
        _ => {}
    }

    server.lobby.legacy_votekick_target = Some(target_id);
    server.lobby.legacy_votekick_votes.clear();
    server.lobby.legacy_votekick_votes.push(v_id);
    server.lobby.legacy_votekick_timer = LEGACY_VOTEKICK_TIMER_TICKS;
    server.lobby.legacy_votekick_ongoing = true;
    server.lobby.kick_target = server.find_peer(target_id).map(clone_peer_data);

    let target_nick = server.find_peer(target_id).map(|p| p.nickname.clone()).unwrap_or_default();
    broadcast_chat(outbox, "-----------------------");
    broadcast_chat(outbox, &format!("{}голосование начато для {}{}{}", COLOR_RED, COLOR_BLUE, target_nick, COLOR_RESET));
    broadcast_chat(outbox, &format!("напиши {}.y{} или {}.n{}", COLOR_CYAN, COLOR_RESET, COLOR_RED, COLOR_RESET));
    broadcast_chat(outbox, "-----------------------");
}

fn handle_vote_yes_legacy(v_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if server.lobby.legacy_votekick_timer <= 0.0 {
        return;
    }
    if server.lobby.legacy_votekick_votes.contains(&v_id) {
        return;
    }
    server.lobby.legacy_votekick_votes.push(v_id);

    let nick = server.find_peer(v_id).map(|p| p.nickname.clone()).unwrap_or_default();
    broadcast_chat(outbox, &format!("{} голосует {}за{}", nick, COLOR_CYAN, COLOR_RESET));
    check_votekick_legacy_early(server, outbox);
}

fn handle_vote_no_legacy(v_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if server.lobby.legacy_votekick_timer <= 0.0 {
        return;
    }
    server.lobby.legacy_votekick_votes.retain(|&id| id != v_id);

    let nick = server.find_peer(v_id).map(|p| p.nickname.clone()).unwrap_or_default();
    broadcast_chat(outbox, &format!("{} голосует {}против{}", nick, COLOR_RED, COLOR_RESET));
    check_votekick_legacy_early(server, outbox);
}

/// Resolves the vote early once every player but the target has voted yes
/// (mirrors `_voteKickVotes.Count >= server.Peers.Count(e => e.Key != target)`).
fn check_votekick_legacy_early(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let target = server.lobby.legacy_votekick_target;
    let non_target = server.peers.iter().filter(|p| Some(p.id) != target).count();
    if server.lobby.legacy_votekick_votes.len() >= non_target {
        check_votekick_legacy(server, false, outbox);
    }
}

fn check_votekick_legacy(server: &mut Server, ignore: bool, outbox: &mut Vec<OutboxMsg>) {
    if (server.lobby.legacy_votekick_timer <= 0.0 && !ignore) || !server.lobby.legacy_votekick_ongoing {
        return;
    }

    let target = match server.lobby.legacy_votekick_target {
        Some(t) => t,
        None => return,
    };

    if server.find_peer(target).is_none() {
        broadcast_chat(outbox, &format!("{}голосование провалено{} (игрок уехал)", COLOR_RED, COLOR_RESET));
        server.lobby.legacy_votekick_votes.clear();
        server.lobby.legacy_votekick_timer = 0.0;
        server.lobby.legacy_votekick_target = None;
        server.lobby.legacy_votekick_ongoing = false;
        server.lobby.kick_target = None;
        return;
    }

    let count = server.lobby.legacy_votekick_votes.len();
    let num = server.peers.iter()
        .filter(|p| !server.lobby.legacy_votekick_votes.contains(&p.id))
        .count();

    if count >= num {
        broadcast_chat(outbox, &format!("{}голосование успешно{} ({}{}{} и {}{}{})", COLOR_CYAN, COLOR_RESET, COLOR_CYAN, count, COLOR_RESET, COLOR_RED, num, COLOR_RESET));

        let cfg = cfg();
        if let Some(tp) = server.find_peer(target) {
            let (nick, udid, ip) = (tp.nickname.clone(), tp.udid.clone(), tp.ip.clone());
            let kick_secs = cfg.server_config.pairing.kick_timeout_window as u64;
            if cfg.states.lobby_misc.votekick.autoban_leavers {
                crate::moderation::ban_add(&nick, &udid, &ip, &nick);
            }
            crate::moderation::timeout_set(&nick, &udid, &ip,
                crate::moderation::now_unix() + kick_secs, &nick);
        }
        outbox.push(OutboxMsg::Disconnect(target, DisconnectReason::KickedByHost as u32));
    } else {
        broadcast_chat(outbox, &format!("{}голосование провалено{} ({}{}{} против {}{}{})", COLOR_RED, COLOR_RESET, COLOR_CYAN, count, COLOR_RESET, COLOR_RED, num, COLOR_RESET));
    }

    server.lobby.legacy_votekick_votes.clear();
    server.lobby.legacy_votekick_timer = 0.0;
    server.lobby.legacy_votekick_target = None;
    server.lobby.legacy_votekick_ongoing = false;
    server.lobby.kick_target = None;
}

fn handle_practice_legacy(v_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let nick = server.find_peer(v_id).map(|p| p.nickname.clone()).unwrap_or_default();

    if server.lobby.legacy_practice_ongoing {
        if server.lobby.legacy_practice_votes.contains(&v_id) {
            return;
        }
        server.lobby.legacy_practice_votes.push(v_id);
        broadcast_chat(outbox, &format!("{} хочет {}попрактиковаться{}", nick, COLOR_YELLOW, COLOR_RESET));

        if server.lobby.legacy_practice_votes.len() >= server.peers.len() {
            server.lobby.legacy_practice_ongoing = false;
            server.lobby.legacy_practice_votes.clear();

            let map: i8 = 20; // Fart Zone, the legacy practice map
            server.lobby.map = map;
            server.last_map = map;
            for p in server.peers.iter_mut() {
                p.in_game = true;
            }
            crate::states::char_select::charselect_init(server, outbox);
        }
        return;
    }

    server.lobby.legacy_practice_votes.clear();
    server.lobby.legacy_practice_votes.push(v_id);
    server.lobby.legacy_practice_ongoing = true;

    broadcast_chat(outbox, "-----------------------");
    broadcast_chat(outbox, &format!("{}{}голосование за практику{} начато {}{}{}-ом{}", COLOR_RED, COLOR_YELLOW, COLOR_RESET, COLOR_BLUE, nick, COLOR_RESET, COLOR_RESET));
    broadcast_chat(outbox, &format!("напиши {}.p{} для захода на тренировочную карту", COLOR_YELLOW, COLOR_RESET));
    broadcast_chat(outbox, "-----------------------");
}

fn force_start(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if server.total_count() < 2 {
        return;
    }
    for p in server.peers.iter_mut() {
        p.ready = true;
        p.in_game = true;
    }
    mapvote_init(server, outbox);
}

fn send_countdown(sec: u8, outbox: &mut Vec<OutboxMsg>) {
    let is_counting: u8 = if sec < NO_COUNTDOWN { 1 } else { 0 };
    let mut pkt = Packet::new(PacketType::SERVER_LOBBY_COUNTDOWN);
    let _ = pkt.write_u8(is_counting);
    let _ = pkt.write_u8(sec);
    outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));
}

fn check_countdown(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    check_countdown_ex(0, server, outbox);
}

fn check_countdown_ex(exclude: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();
    let total = server.peers.iter().filter(|p| p.id != exclude).count();
    let ready_count = server.peers.iter().filter(|p| p.ready && p.id != exclude).count();

    if total < 2 {
        if server.lobby.countdown_sec < NO_COUNTDOWN {
            server.lobby.countdown_sec = NO_COUNTDOWN;
            server.lobby.countdown = 0.0;
            send_countdown(NO_COUNTDOWN, outbox);
        }
        return;
    }

    let required_pct = cfg.states.lobby_misc.lobby_ready_required_percentage as f64 / 100.0;
    let required = ((total as f64 * required_pct).ceil() as usize).max(1);

    if ready_count >= required && server.lobby.countdown_sec >= NO_COUNTDOWN {
        let timer = cfg.states.lobby_misc.lobby_start_timer;
        server.lobby.countdown_sec = timer;
        server.lobby.countdown = timer as f64 * 60.0;
        send_countdown(timer, outbox);
    } else if ready_count < required && server.lobby.countdown_sec < NO_COUNTDOWN {
        server.lobby.countdown_sec = NO_COUNTDOWN;
        server.lobby.countdown = 0.0;
        send_countdown(NO_COUNTDOWN, outbox);
    }
}

fn find_player_by_name_or_id(s: &str, server: &Server, exclude: u16) -> Option<u16> {
    if let Ok(id) = s.parse::<u16>() {
        if server.find_peer(id).is_some() && id != exclude {
            return Some(id);
        }
    }
    let lower = s.to_lowercase();
    server
        .peers
        .iter()
        .find(|p| p.id != exclude && p.nickname.to_lowercase() == lower)
        .map(|p| p.id)
}

pub fn lobby_state_tick(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();

    for p in server.peers.iter_mut() {
        if p.vote_cooldown > 0.0 {
            p.vote_cooldown -= 1.0;
        }
    }

    if server.lobby.vote.ongoing {
        if !server.lobby.vote.tick(1.0) {
            check_vote_result(server, outbox);
        }
    }

    if server.lobby.legacy_votekick_ongoing {
        server.lobby.legacy_votekick_timer -= 1.0;
        if server.lobby.legacy_votekick_timer <= 0.0 {
            check_votekick_legacy(server, true, outbox);
        }
    }

    let timeout_ticks = cfg.states.lobby_misc.lobby_timeout_timer as f64 * 60.0;
    let mut to_kick: Vec<u16> = Vec::new();
    for p in server.peers.iter_mut() {
        if p.op >= 2 || p.ready {
            p.timeout = 0.0;
            continue;
        }
        if p.should_timeout {
            p.timeout += 1.0;
            if p.timeout >= timeout_ticks {
                to_kick.push(p.id);
            }
        }
    }
    for id in to_kick {
        outbox.push(OutboxMsg::Disconnect(id, DisconnectReason::AfkTimeout as u32));
    }

    if server.lobby.prac_countdown > 0.0 {
        server.lobby.prac_countdown -= 1.0;
        if server.lobby.prac_countdown <= 0.0 {
            server.lobby.prac_countdown = 0.0;
            if let Some(kt) = server.lobby.kick_target.take() {

                let kick_secs = cfg.server_config.pairing.kick_timeout_window as u64;
                crate::moderation::timeout_set(&kt.nickname, &kt.udid, &kt.ip,
                    crate::moderation::now_unix() + kick_secs, &kt.nickname);
                outbox.push(OutboxMsg::Disconnect(kt.id, DisconnectReason::KickedByHost as u32));
                broadcast_chat(outbox, &format!("\\c{} was kicked.", kt.nickname));
            } else if server.lobby.voting_map >= 0 {

                let map = server.lobby.voting_map;
                server.lobby.voting_map = -1;
                server.lobby.map = map;
                for p in server.peers.iter_mut() {
                    p.in_game = true;
                }
                crate::states::char_select::charselect_init(server, outbox);
            }
        }
    }

    if server.lobby.countdown_sec < NO_COUNTDOWN && server.lobby.countdown > 0.0 {
        server.lobby.countdown -= 1.0;

        let new_sec = (server.lobby.countdown / 60.0).ceil() as u8;
        if new_sec < server.lobby.countdown_sec {
            server.lobby.countdown_sec = new_sec;
            send_countdown(new_sec, outbox);
        }

        if server.lobby.countdown <= 0.0 {
            server.lobby.countdown_sec = 0;
            start_game_from_lobby(server, outbox);
        }
    }
}

fn start_game_from_lobby(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();

    if cfg.states.lobby_misc.kick_unready_before_starting {
        let unready: Vec<u16> = server
            .peers
            .iter()
            .filter(|p| !p.ready)
            .map(|p| p.id)
            .collect();
        for id in unready {
            outbox.push(OutboxMsg::Disconnect(id, DisconnectReason::KickedByHost as u32));
            server.peers.retain(|p| p.id != id);
        }
    }

    for p in server.peers.iter_mut() {
        p.in_game = true;
    }

    mapvote_init(server, outbox);
}
