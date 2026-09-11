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

/// This is a "promote, don't demote" reset. It never sets in_game back to
/// false for anyone -- a player who's already in_game (was actively playing
/// the round that just ended) stays in_game across the transition; only
/// peers who joined mid-round into the waiting room (in_game already
/// false, set at connect time when the server wasn't in Lobby state) get
/// promoted to in_game=true here, notified via the same
/// SERVER_IDENTITY_RESPONSE their client got on first connect. That
/// asymmetry is why every other Lobby-state entry point (natural countdown,
/// the ".map" force-command, ?start, vote-practice) can call this and then
/// jump straight into mapvote_init/charselect_init with no extra
/// bookkeeping: lobby_init already leaves in_game correct for everyone.
pub fn lobby_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    log::debug!("Entering Lobby state...");

    // A tournament bracket with rounds still left in it (see
    // results::tournament_continues) means this isn't an open Lobby at all --
    // just the pass-through every client needs to leave the Results screen and
    // re-enter its lobby room before the next elimination round. Nothing has to
    // be arranged for us beforehand and nothing is being hidden from us: we ask
    // the tournament what it's doing, right here, same as results_init does.
    let tournament_round = crate::states::results::tournament_continues(server);

    server.state = GameState::Lobby;
    server.lobby.countdown = 0.0;
    server.lobby.prac_countdown = 0.0;
    server.lobby.countdown_sec = NO_COUNTDOWN;
    server.lobby.vote = Vote::default();
    server.lobby.kick_target = None;
    server.lobby.voting_map = -1;
    server.lobby.map = -1;
    server.lobby.exe = 0;
    server.lobby.chars_available = [true; 6];
    server.lobby.legacy_votekick_ongoing = false;
    server.lobby.legacy_votekick_target = None;
    server.lobby.legacy_votekick_timer = 0.0;
    server.lobby.legacy_votekick_votes.clear();
    server.lobby.legacy_practice_ongoing = false;
    server.lobby.legacy_practice_votes.clear();
    // Only tournament_advance sets these again, and only after this call returns.
    server.lobby.tournament_owed = 0;
    server.lobby.tournament_wait = 0.0;

    let mut newly_promoted: Vec<u16> = Vec::new();
    for peer in server.peers.iter_mut() {
        peer.ready = false;
        // A spectator keeps spectating across a tournament round boundary.
        // There is no packet that puts a client back on the waiting screen, so
        // ending their spectate here would leave them on the map with nothing
        // coming to move them off it -- they ride the next round the way they
        // rode this one. Once the bracket is over this is a real Lobby entry
        // and they walk in as an ordinary player, like any other waiter.
        if !tournament_round {
            peer.spectate = crate::states::spectate::Spectate::No;
        }
        peer.plr = crate::player::Player::default();
        peer.surv_char = SurvChar::None;
        peer.exe_char = crate::server::ExeChar::None;
        if !peer.in_game {
            // The bracket is still running, so the waiting room stays the
            // waiting room: the next round is played by exactly this round's
            // survivors. Nobody is let in here, so there is nothing to put back
            // afterwards either -- they get in through this very branch on the
            // first Lobby entry after the bracket ends.
            if tournament_round {
                continue;
            }
            peer.in_game = true;
            newly_promoted.push(peer.id);
        }
    }
    for id in newly_promoted {
        let mut resp = Packet::new(PacketType::SERVER_IDENTITY_RESPONSE);
        let _ = resp.write_u8(1);
        let _ = resp.write_u16(id);
        outbox.push(OutboxMsg::SendTo(id, resp.data().to_vec(), true));
    }

    update_exe_chance(server);
    log::info!("Server is now in Lobby");
}

pub fn lobby_broadcast_init(server: &Server, outbox: &mut Vec<OutboxMsg>) {
    // Addressed to in_game peers rather than broadcast, because lobby_init has
    // just set in_game to exactly "who is in this Lobby". Outside a tournament
    // that is everyone, so this is the same broadcast as ever; between
    // tournament rounds it is the round's participants, and the waiting room
    // lobby_init deliberately kept out is skipped here too.
    //
    // That matters: the client acts on SERVER_GAME_BACK_TO_LOBBY whatever
    // screen it is sitting on -- it just enters the lobby room and re-requests
    // the roster -- so sending it to the waiting room would walk them straight
    // into a Lobby they are not part of. They stay on the waiting screen; the
    // Lobby entry at the end of the bracket reaches them like any other.
    //
    // The exe-chance readout is left out of that same pass-through. The client
    // does nothing with that number except print it on the lobby screen, and
    // nobody stays on that screen to read it here -- the next round's vote
    // starts the moment the last client has re-fetched its roster. Leaving it
    // out does not blank the readout, it just leaves last round's number up
    // until the Lobby entry that ends the bracket refreshes it. The question is
    // the same one lobby_init asked a moment ago, asked the same way, so the
    // two cannot disagree about which kind of Lobby entry this is.
    let tournament_round = crate::states::results::tournament_continues(server);

    let pkt = Packet::new(PacketType::SERVER_GAME_BACK_TO_LOBBY);
    for peer in server.peers.iter().filter(|peer| peer.follows_round()) {
        outbox.push(OutboxMsg::SendTo(peer.id, pkt.data().to_vec(), true));

        if tournament_round { continue; }

        let mut exe_chance_pkt = Packet::new(PacketType::SERVER_LOBBY_EXE_CHANCE);
        let _ = exe_chance_pkt.write_u8(peer.exe_chance);
        outbox.push(OutboxMsg::SendTo(peer.id, exe_chance_pkt.data().to_vec(), true));
    }
}

fn update_exe_chance(server: &mut Server) {
    let last_exe = server.game.exe;
    for peer in server.peers.iter_mut() {
        if peer.id as i32 == last_exe {
            peer.exe_chance = 1;
        } else {
            peer.exe_chance = peer.exe_chance.saturating_add(1);
        }
    }
}

pub fn lobby_state_join(player_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    send_lobby_player_list(player_id, server, outbox);

    if let Some(peer) = server.find_peer(player_id) {
        let nickname = peer.nickname.clone();
        let id = peer.id;
        let lobby_icon = peer.lobby_icon;
        let pet = peer.pet;
        let exe_chance = peer.exe_chance;
        let anon_mode = cfg().states.lobby_misc.anonymous_mode;

        let recipients: Vec<(u16, u8)> = server.peers.iter()
            .filter(|peer| peer.id != id)
            .map(|peer| (peer.id, peer.op))
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

        let mut exe_chance_pkt = Packet::new(PacketType::SERVER_LOBBY_EXE_CHANCE);
        let _ = exe_chance_pkt.write_u8(exe_chance);
        outbox.push(OutboxMsg::SendTo(id, exe_chance_pkt.data().to_vec(), true));
    }

    check_countdown(server, outbox);
}

// Only in_game peers belong on the Lobby roster a target requests. In every
// non-tournament case that's already everyone (lobby_init promotes the whole
// roster before anyone can be in Lobby at all), so this is a no-op filter
// there -- it only matters between tournament rounds, where the waiting room
// stays in_game=false while the bracket is still running.
pub fn send_lobby_player_list(target: u16, server: &Server, outbox: &mut Vec<OutboxMsg>) {
    let anon_mode = cfg().states.lobby_misc.anonymous_mode;
    let target_op = server.peers.iter().find(|peer| peer.id == target).map(|peer| peer.op).unwrap_or(0);
    let anon = anon_mode && target_op < 1;
    for peer in server.peers.iter() {
        if peer.id == target || !peer.in_game {
            continue;
        }
        let mut pkt = Packet::new(PacketType::SERVER_LOBBY_PLAYER);
        let _ = pkt.write_u16(peer.id);
        let _ = pkt.write_u8(if peer.ready { 1 } else { 0 });
        if anon {
            let _ = pkt.write_str("anonymous");
            let _ = pkt.write_u8(0);
            let _ = pkt.write_i8(-1);
        } else {
            let _ = pkt.write_str(&peer.nickname);
            let _ = pkt.write_u8(peer.lobby_icon);
            let _ = pkt.write_i8(peer.pet);
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

pub fn lobby_state_left(player_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if server.lobby.vote.ongoing {
        if let Some(kick_target) = &server.lobby.kick_target {
            let target_id = kick_target.id;
            let cfg = cfg();
            if target_id == player_id && cfg.states.lobby_misc.votekick.autoban_leavers {
                log::info!("Kick target left during vote - vote auto-succeeds");
                let kick_secs = cfg.server_config.pairing.kick_timeout_window as u64;
                if let Some(peer) = server.find_peer(player_id) {
                    let (nick, udid, ip) = (peer.nickname.clone(), peer.udid.clone(), peer.ip.clone());
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
        server.lobby.legacy_votekick_votes.retain(|&id| id != player_id);

        if server.lobby.legacy_votekick_target == Some(player_id) {
            broadcast_chat(outbox, &format!("{}голосование провалено{} (игрок уехал)", COLOR_RED, COLOR_RESET));
            server.lobby.legacy_votekick_votes.clear();
            server.lobby.legacy_votekick_timer = 0.0;
            server.lobby.legacy_votekick_target = None;
            server.lobby.legacy_votekick_ongoing = false;
            server.lobby.kick_target = None;
        } else {
            let target = server.lobby.legacy_votekick_target;
            let remaining_non_target = server.peers.iter()
                .filter(|peer| peer.id != player_id && Some(peer.id) != target)
                .count();
            if server.lobby.legacy_votekick_votes.len() >= remaining_non_target {
                check_votekick_legacy(server, false, outbox);
            }
        }
    }

    if server.lobby.legacy_practice_ongoing {
        server.lobby.legacy_practice_votes.retain(|&id| id != player_id);
    }

    let mut pkt = Packet::new(PacketType::SERVER_PLAYER_LEFT);
    let _ = pkt.write_u16(player_id);
    outbox.push(OutboxMsg::BroadcastEx(pkt.data().to_vec(), true, player_id));

    check_countdown_ex(player_id, server, outbox);
}

pub fn lobby_state_handle(
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
        PacketType::CLIENT_LOBBY_PLAYERS_REQUEST => {
            send_lobby_player_list(player_id, server, outbox);
            let pkt = Packet::new(PacketType::SERVER_LOBBY_CORRECT);
            outbox.push(OutboxMsg::SendTo(player_id, pkt.data().to_vec(), true));
            send_greeting(player_id, server, outbox);

            // Between tournament rounds this request is the hand-off itself:
            // the client has just re-entered its lobby room and taken the
            // roster, and it stops accepting one once the vote starts. So the
            // vote starts here, the moment the last player owing us a request
            // has been served -- same shape as CharSelect starting the game
            // once everyone has picked. The waiting room is not part of a
            // hand-off and never owes anything.
            let is_participant = server.find_peer(player_id).map(|peer| peer.in_game).unwrap_or(false);
            if server.lobby.tournament_owed > 0 && is_participant {
                server.lobby.tournament_owed -= 1;
                if server.lobby.tournament_owed == 0 {
                    server.lobby.tournament_wait = 0.0;
                    mapvote_init(server, outbox);
                }
            }
        }

        PacketType::CLIENT_CHAT_MESSAGE => {
            if !server.chat_rate_allow(player_id) { return; } // anti-flood
            packet.pos = 2;
            let _sender = packet.read_u16();
            let msg = match packet.read_str() {
                Some(text) => text,
                None => return,
            };
            handle_chat(player_id, &msg, server, outbox);
        }

        PacketType::CLIENT_LOBBY_READY_STATE => {
            // Only the Lobby's own occupants have a readiness to report. A
            // spectator passing through between tournament rounds is standing
            // in the same room but is not one of them.
            if !server.find_peer(player_id).map(|peer| peer.in_game).unwrap_or(false) { return; }

            packet.pos = 2;
            let ready = packet.read_u8().unwrap_or(0) != 0;

            if let Some(peer) = server.find_peer_mut(player_id) {
                peer.ready = ready;
            }

            let mut pkt = Packet::new(PacketType::SERVER_LOBBY_READY_STATE);
            let _ = pkt.write_u16(player_id);
            let _ = pkt.write_u8(if ready { 1 } else { 0 });
            outbox.push(OutboxMsg::BroadcastEx(pkt.data().to_vec(), true, player_id));

            check_countdown(server, outbox);
        }

        PacketType::CLIENT_LOBBY_CHOOSEVOTEKICK => {
            packet.pos = 2;
            let target_id = match packet.read_u16() {
                Some(value) => value,
                None => return,
            };
            if cfg().states.lobby_misc.votekick.use_legacy_voting {
                handle_votekick_legacy(player_id, target_id, server, outbox);
            } else {
                handle_votekick(player_id, target_id, server, outbox);
            }
        }

        PacketType::CLIENT_LOBBY_CHOOSEBAN => {
            packet.pos = 2;
            let target_id = match packet.read_u16() {
                Some(value) => value,
                None => return,
            };
            if outranks(player_id, target_id, server) {
                if let Some(nick) = ban_player(target_id, server, outbox) {
                    broadcast_chat(outbox, &format!("\\c{} was banned.", nick));
                }
            }
        }

        PacketType::CLIENT_LOBBY_CHOOSEKICK => {
            packet.pos = 2;
            let target_id = match packet.read_u16() {
                Some(value) => value,
                None => return,
            };
            if outranks(player_id, target_id, server) {
                if let Some(nick) = kick_player(target_id, server, outbox) {
                    broadcast_chat(outbox, &format!("\\c{} was kicked.", nick));
                }
            }
        }

        PacketType::CLIENT_LOBBY_CHOOSEOP => {
            packet.pos = 2;
            let target_id = match packet.read_u16() {
                Some(value) => value,
                None => return,
            };
            let op = server.find_peer(player_id).map(|peer| peer.op).unwrap_or(0);
            if op >= 3 {
                if let Some(nick) = op_player(target_id, server) {
                    broadcast_chat(outbox, &format!("\\c{} was made an operator.", nick));
                }
                send_chat(outbox, target_id, &format!("{}you're an operator now", COLOR_CYAN));
            }
        }

        PacketType::CLIENT_PLAYER_PALETTE => {
            // The palette anti-cheat both rejects impossible recolors and,
            // via its `id == player_id` check, blocks palette-spoofing of another player.
            if cfg().states.gameplay.anticheat.palette_anticheat {
                packet.pos = 2;
                if !crate::anticheat::palette::palette_player_validate(player_id, packet) {
                    outbox.push(OutboxMsg::Disconnect(player_id, DisconnectReason::Other as u32));
                    return;
                }
            }
            packet.pos = 0;
            let data = packet.data().to_vec();
            outbox.push(OutboxMsg::BroadcastEx(data, false, player_id));
        }

        PacketType::CLIENT_PET_PALETTE => {
            // Pet palettes are passthrough (as in upstream): the validator only knows
            // the character palette tables, so it does not apply here.
            packet.pos = 0;
            let data = packet.data().to_vec();
            outbox.push(OutboxMsg::BroadcastEx(data, false, player_id));
        }

        PacketType::CLIENT_PING => {
            let pkt = Packet::new(PacketType::SERVER_PONG);
            outbox.push(OutboxMsg::SendTo(player_id, pkt.data().to_vec(), false));
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
        Some(parsed) => parsed,
        None => return,
    };
    match cmd.name.as_str() {
        "ban" => {
            match find_player_by_name_or_id(cmd.arg(0), server, 0)
                .and_then(|target_id| ban_player(target_id, server, outbox))
            {
                Some(nick) => log::info!("{} was banned.", nick),
                None       => log::info!("player not found."),
            }
        }
        "kick" => {
            match find_player_by_name_or_id(cmd.arg(0), server, 0)
                .and_then(|target_id| kick_player(target_id, server, outbox))
            {
                Some(nick) => log::info!("{} was kicked.", nick),
                None       => log::info!("player not found."),
            }
        }
        "op" => {
            let target = find_player_by_name_or_id(cmd.arg(0), server, 0);
            match target.and_then(|target_id| op_player(target_id, server).map(|nick| (target_id, nick))) {
                Some((target_id, nick)) => {
                    send_chat(outbox, target_id, &format!("{}you're an operator now", COLOR_CYAN));
                    log::info!("{} was made an operator.", nick);
                }
                None => log::info!("player not found."),
            }
        }
        "kick_now" => {
            if let Ok(target_id) = cmd.arg(0).parse::<u16>() {
                outbox.push(OutboxMsg::Disconnect(target_id, DisconnectReason::KickedByHost as u32));
            }
        }
        "ban_now" => {
            if let Ok(target_id) = cmd.arg(0).parse::<u16>() {
                outbox.push(OutboxMsg::Disconnect(target_id, DisconnectReason::BannedByHost as u32));
            }
        }
        "set_op_pid" => {
            if let (Ok(target_id), Ok(level)) = (cmd.arg(0).parse::<u16>(), cmd.arg(1).parse::<u8>()) {
                if let Some(peer) = server.find_peer_mut(target_id) {
                    peer.op = level;
                }
                let shy = cfg().states.lobby_misc.moderation.shy_mode;
                if level > 0 {
                    send_chat(outbox, target_id, &format!("{}you're an operator now", COLOR_CYAN));
                } else if !shy {
                    send_chat(outbox, target_id, &format!("{}you're no longer an operator", COLOR_RED));
                }
            }
        }
        _ => {
            exec_cmd(&cmd, 0, u8::MAX, server, outbox);
        }
    }
}

fn handle_chat(player_id: u16, msg: &str, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let nick = server.find_peer(player_id).map(|peer| peer.nickname.clone()).unwrap_or_default();
    log::info!("{} (id {}): {}", crate::colors::colorize(&nick), player_id, msg);

    let trimmed = msg.trim();

    if trimmed.starts_with(':') || trimmed.starts_with('.') {
        if let Some(cmd) = parse_cmd(trimmed) {
            if cmd.name == "stop" {
                return;
            }
            let op = server.find_peer(player_id).map(|peer| peer.op).unwrap_or(0);
            let handled = exec_cmd(&cmd, player_id, op, server, outbox);
            if handled {
                return;
            }
        }
    }

    if let Some(peer) = server.find_peer(player_id) {
        let id = peer.id;
        let mut pkt = Packet::new(PacketType::CLIENT_CHAT_MESSAGE);
        let _ = pkt.write_u16(id);
        let _ = pkt.write_str(msg);
        outbox.push(OutboxMsg::BroadcastEx(pkt.data().to_vec(), true, id));
    }
}

pub fn send_greeting(player_id: u16, server: &Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();
    let server_count = cfg.server_config.networking.server_count;
    send_chat(outbox, player_id, &cfg.states.lobby_misc.upper_bracket);
    send_chat(outbox, player_id, &format!("hosted by {}{}{}", COLOR_PURPUR, cfg.states.lobby_misc.hosts_name, COLOR_RESET));
    if server_count >= 2 {
        send_chat(outbox, player_id, &format!("server {}{}{} of {}{}{}", COLOR_RED, server.id + 1, COLOR_RESET, COLOR_BLUE, server_count, COLOR_RESET));
    }
    send_chat(outbox, player_id, &cfg.states.lobby_misc.lower_bracket);
    send_chat(outbox, player_id, &format!("{}type .help for command list{}", COLOR_GRAY, COLOR_RESET));
    let loc = &cfg.states.lobby_misc.server_location;
    let ping_limit = cfg.server_config.pairing.ping_limit;
    if !loc.is_empty() && ping_limit != u16::MAX {
        send_chat(outbox, player_id, &format!("{}, required ping: {} or less", loc, ping_limit));
    }
    let motd = cfg.states.lobby_misc.message_of_the_day.clone();
    if !motd.is_empty() {
        send_chat(outbox, player_id, &motd);
    }
    if cfg.states.lobby_misc.authoritarian_mode {
        send_chat(outbox, player_id, &format!("{}authoritarian mode is on", COLOR_RED));
    }
    let op = server.find_peer(player_id).map(|peer| peer.op).unwrap_or(0);
    match op {
        1 => send_chat(outbox, player_id, &format!("{}you're an operator on this server{}", COLOR_CYAN, COLOR_RESET)),
        2 => send_chat(outbox, player_id, &format!("{}you're a moderator on this server{}", COLOR_CYAN, COLOR_RESET)),
        3 => send_chat(outbox, player_id, &format!("{}you've got root perms on this server{}", COLOR_CYAN, COLOR_RESET)),
        _ => {}
    }
}

pub fn exec_cmd_pub(
    cmd: &Cmd,
    player_id: u16,
    op: u8,
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
) -> bool {
    match cmd.name.as_str() {
        "help" | "info" | "credits" | "source" | "stink" | "lobby"
        | "kick" | "ban" | "op" | "status" | "stop" | "spectate" => {
            exec_cmd(cmd, player_id, op, server, outbox)
        }
        _ => false,
    }
}

/// Parses a 1-based map index from `arg` and, if valid, jumps straight to
/// CharSelect with it -- skipping MapVote entirely, as if map_selection were
/// disabled in config or a successful .vp/.votepractice vote had just
/// happened. Shared by .map and :start/?start's optional map argument.
fn start_with_map(arg: &str, player_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let index: i32 = match arg.parse::<i32>() {
        Ok(parsed) => parsed,
        Err(_) => {
            send_chat(outbox, player_id, &format!("{}example:~ .map 1", COLOR_RED));
            return;
        }
    };
    let map_count = crate::maps::MAP_COUNT as i32;
    if index < 1 || index > map_count {
        send_chat(outbox, player_id, &format!("{}map should be between 1 and {}", COLOR_RED, map_count));
        return;
    }
    let map = (index - 1) as i8;
    server.lobby.map = map;
    server.last_map = map;
    crate::states::char_select::charselect_init(server, outbox);
}

fn exec_cmd(
    cmd: &Cmd,
    player_id: u16,
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
                send_chat(outbox, player_id, &format!("{}example:~ .help 1", COLOR_RED));
                return true;
            }
            send_chat(outbox, player_id, &format!("{}help {}page {}{}", COLOR_YELLOW, COLOR_CYAN, page, COLOR_RESET));
            let server_count = cfg.server_config.networking.server_count;
            let map_count = crate::maps::MAP_COUNT;
            match page {
                1 => {
                    if server_count >= 2 {
                        send_chat(outbox, player_id, &format!("{}.lobby{} choose lobby (1-{})", COLOR_GRAY, COLOR_RESET, server_count));
                    }
                    send_chat(outbox, player_id, &format!("{}.info{} server info", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, player_id, &format!("{}:credits{} project credits", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, player_id, &format!("{}:source{} source code link", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, player_id, &format!("{}:status{} server statistics", COLOR_GRAY, COLOR_RESET));
                }
                2 => {
                    send_chat(outbox, player_id, &format!("{}.kick{} kick a player", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, player_id, &format!("{}.ban{} ban a player", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, player_id, &format!("{}.op{} make a player an admin", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, player_id, &format!("{}:stop{} force-end the current round", COLOR_GRAY, COLOR_RESET));
                }
                3 => {
                    send_chat(outbox, player_id, &format!("{}.vk{} vote kick", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, player_id, &format!("{}.vp{} vote practice", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, player_id, &format!("{}:vm{} vote specific map", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, player_id, &format!("{}.y{} vote for", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, player_id, &format!("{}.yes{} vote for", COLOR_GRAY, COLOR_RESET));
                    if cfg.states.lobby_misc.votekick.use_legacy_voting {
                        send_chat(outbox, player_id, &format!("{}.n{} vote against", COLOR_GRAY, COLOR_RESET));
                        send_chat(outbox, player_id, &format!("{}.no{} vote against", COLOR_GRAY, COLOR_RESET));
                        send_chat(outbox, player_id, &format!("{}.practice{} vote for practice", COLOR_GRAY, COLOR_RESET));
                    }
                }
                4 => {
                    send_chat(outbox, player_id, &format!("{}.m{} mute the chat (local command)", COLOR_GRAY, COLOR_RESET));
                    if cfg.states.gameplay.allow_spectators {
                        send_chat(outbox, player_id, &format!("{}.spectate{} watch the running round", COLOR_GRAY, COLOR_RESET));
                    }
                }
                5 => {
                    send_chat(outbox, player_id, &format!("{}:chance{} change your exe chance", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, player_id, &format!("{}:stop{} force end round", COLOR_GRAY, COLOR_RESET));
                    send_chat(outbox, player_id, &format!("{}.map{} choose map (1-{})", COLOR_GRAY, COLOR_RESET, map_count));
                    send_chat(outbox, player_id, &format!("{}:start{} force start round", COLOR_GRAY, COLOR_RESET));
                }
                _ => {
                    send_chat(outbox, player_id, "void");
                }
            }
        }

        "info" => {
            send_chat(outbox, player_id, &format!("-----{}advanced{}server{}-----", COLOR_RED, COLOR_BLUE, COLOR_RESET));
            send_chat(outbox, player_id, &format!("version: {}{}{}", COLOR_BLUE, SERVER_VERSION, COLOR_RESET));
            send_chat(outbox, player_id, &format!(
                "built on {}{}{} at {}{}{} for {}{}{} via {}{}",
                COLOR_PURPUR, env!("BUILD_DATE"), COLOR_RESET,
                COLOR_CYAN, env!("BUILD_TIME"), COLOR_RESET,
                COLOR_RED, env!("BUILD_TARGET_OS"), COLOR_RESET,
                COLOR_YELLOW, env!("BUILD_RUSTC_VERSION"),
            ));
            send_chat(outbox, player_id, &format!("{}(c) {}2023-2026 {}the arctic fox{}", COLOR_CYAN, COLOR_BLUE, COLOR_RED, COLOR_RESET));
            send_chat(outbox, player_id, "------------------------");
        }

        "credits" => {
            send_chat(outbox, player_id, &format!("-----{}credits{}-----", COLOR_YELLOW, COLOR_RESET));
            send_chat(outbox, player_id, &format!("created by: {}the arctic fox{}", COLOR_YELLOW, COLOR_RESET));
            send_chat(outbox, player_id, &format!("contains code referenced from: {}betterserver-oss{}", COLOR_PURPUR, COLOR_RESET));
            send_chat(outbox, player_id, &format!("{}BetterServer-OSS{} (MIT) by {}Team EXE Empire{}", COLOR_CYAN, COLOR_RESET, COLOR_PURPUR, COLOR_RESET));
            if cfg.states.gameplay.gmcycle.overhell {
                send_chat(outbox, player_id, &format!("{}Gamemode Cycle{} by {}IcedCoffee & ColdestTea{}", COLOR_CYAN, COLOR_RESET, COLOR_PURPUR, COLOR_RESET));
            }
            if cfg.states.gameplay.banana.disable_timer {
                send_chat(outbox, player_id, &format!("{}TD2DR No Timer{} by {}Banana8870{}", COLOR_CYAN, COLOR_RESET, COLOR_PURPUR, COLOR_RESET));
            }
            send_chat(outbox, player_id, "------------------------");
        }

        "source" => {
            send_chat(outbox, player_id, &format!("{}source code:{}", COLOR_YELLOW, COLOR_RESET));
            send_chat(outbox, player_id, "https:??github.com?thearcticfox25?AdvancedServer");
            send_chat(outbox, player_id, &format!("license: {}GNU AGPL v3{}", COLOR_CYAN, COLOR_RESET));
        }

        "stink" => {
            let name = cmd.arg(0);
            if name.is_empty() {
                send_chat(outbox, player_id, &format!("{}example:~ .stink baller", COLOR_RED));
            } else {
                broadcast_chat(outbox, &format!("{}{}~, you {}sti{}nk~", COLOR_RED, name, COLOR_BLUE, COLOR_CYAN));
            }
        }

        "spectate" => {
            crate::states::spectate::start(player_id, server, outbox);
        }

        "lobby" => {
            let server_count = cfg.server_config.networking.server_count;
            if server_count <= 1 {
                return true;
            }
            let ind_str = cmd.arg(0);
            let index: i32 = match ind_str.parse() {
                Ok(parsed) => parsed,
                Err(_) => {
                    send_chat(outbox, player_id, &format!("{}example:~ .lobby 1", COLOR_RED));
                    return true;
                }
            };
            if index < 1 || index as u16 > server_count {
                send_chat(outbox, player_id, &format!("{}lobby should be between 1 and {}", COLOR_RED, server_count));
                return true;
            }
            let port = cfg.server_config.networking.port as u32 + index as u32 - 1;
            if let Some(peer) = server.find_peer_mut(player_id) {
                peer.should_timeout = false;
            }
            let mut pkt = Packet::new(PacketType::SERVER_LOBBY_CHANGELOBBY);
            let _ = pkt.write_u32(port);
            outbox.push(OutboxMsg::SendTo(player_id, pkt.data().to_vec(), true));
        }

        "yes" | "y" => {
            if cfg.states.lobby_misc.votekick.use_legacy_voting {
                handle_vote_yes_legacy(player_id, server, outbox);
            } else {
                handle_vote_yes(player_id, server, outbox);
            }
        }

        "no" | "n" => {
            if cfg.states.lobby_misc.votekick.use_legacy_voting {
                handle_vote_no_legacy(player_id, server, outbox);
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
            handle_practice_legacy(player_id, server, outbox);
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
                deny_vote_in_progress(player_id, outbox);
                return true;
            }
            if !cfg.states.lobby_misc.votekick.use_legacy_voting {
                let cooldown = server.find_peer(player_id).map(|peer| peer.vote_cooldown).unwrap_or(0.0);
                if cooldown > 0.0 {
                    send_chat(outbox, player_id, &format!("{}you cannot start another vote for {}s", COLOR_RED, (cooldown / 60.0) as i32));
                    return true;
                }
            }
            let ingame = server.total_count();
            if ingame > 2 {
                let pkt = Packet::new(PacketType::SERVER_LOBBY_CHOOSEVOTEKICK);
                outbox.push(OutboxMsg::SendTo(player_id, pkt.data().to_vec(), true));
            } else {
                send_chat(outbox, player_id, &format!("{}not enough participants.", COLOR_RED));
            }
        }

        "vp" => {
            if cfg.states.lobby_misc.authoritarian_mode || cfg.states.lobby_misc.anonymous_mode {
                return true;
            }
            if cfg.states.lobby_misc.votekick.use_legacy_voting {
                handle_practice_legacy(player_id, server, outbox);
            } else {
                handle_map_vote(player_id, Some(20), server, outbox);
            }
        }

        "vm" => {
            if cfg.states.lobby_misc.authoritarian_mode || cfg.states.lobby_misc.anonymous_mode {
                return true;
            }
            let ind_str = cmd.arg(0);
            let index: i32 = match ind_str.parse::<i32>() {
                Ok(parsed) => parsed,
                Err(_) => {
                    send_chat(outbox, player_id, &format!("{}example:~ :vm 1", COLOR_RED));
                    return true;
                }
            };
            let map_count = crate::maps::MAP_COUNT as i32;
            if index < 1 || index > map_count {
                send_chat(outbox, player_id, &format!("{}map should be between 1 and {}", COLOR_RED, map_count));
                return true;
            }
            handle_map_vote(player_id, Some((index - 1) as usize), server, outbox);
        }

        "status" => {
            let page_str = cmd.arg(0);
            let page: i32 = page_str.parse().unwrap_or(-1);
            if page < 1 {
                send_chat(outbox, player_id, &format!("{}example:~ :status 1", COLOR_RED));
                return true;
            }
            send_chat(outbox, player_id, &format!("{}status {}page {}{}", COLOR_YELLOW, COLOR_CYAN, page, COLOR_RESET));
            crate::status::with_status(|status| {
                match page {
                    1 => {
                        send_chat(outbox, player_id, &format!("game stats: {}{}~:{}{}~:{}{}~ (spoiled: {})", COLOR_CYAN, status.surv_win_rounds, COLOR_RED, status.exe_win_rounds, COLOR_GRAY, status.draw_rounds, status.exe_crashed_rounds));
                    }
                    2 => {
                        send_chat(outbox, player_id, &format!("tails shots and hits: {}{}~:{}{}~", COLOR_RED, status.tails_shots, COLOR_CYAN, status.tails_hits));
                        send_chat(outbox, player_id, &format!("cream rings spawned: {}{}~", COLOR_CYAN, status.cream_rings_spawned));
                        send_chat(outbox, player_id, &format!("eggman mines planted: {}{}~", COLOR_CYAN, status.eggman_mines_placed));
                        send_chat(outbox, player_id, &format!("total stuns: {}", status.total_stuns));
                    }
                    3 => {
                        send_chat(outbox, player_id, &format!("exeller clone {}placed: {}~, {}activated: {}~", COLOR_RED, status.exeller_clones_placed, COLOR_CYAN, status.exeller_clones_activated));
                        send_chat(outbox, player_id, &format!("exetior bring spawn: {}~", status.exetior_bring_spawned));
                        send_chat(outbox, player_id, &format!("damage dealt: {}", status.damage_taken));
                    }
                    4 => {
                        send_chat(outbox, player_id, &format!("{} left during games overall", status.timeouts));
                    }
                    _ => {
                        send_chat(outbox, player_id, "void");
                    }
                }
            });
        }

        "map" => {
            if op < 1 {
                deny_permission(player_id, outbox);
                return true;
            }
            start_with_map(cmd.arg(0), player_id, server, outbox);
        }

        "kick" => {
            if op < 2 {
                deny_permission(player_id, outbox);
                return true;
            }
            if server.total_count() <= 1 {
                send_chat(outbox, player_id, &format!("{}dude are you gonna kick yourself?", COLOR_RED));
                return true;
            }
            let pkt = Packet::new(PacketType::SERVER_LOBBY_CHOOSEKICK);
            outbox.push(OutboxMsg::SendTo(player_id, pkt.data().to_vec(), true));
        }

        "ban" => {
            if op < 2 {
                deny_permission(player_id, outbox);
                return true;
            }
            if server.total_count() <= 1 {
                send_chat(outbox, player_id, &format!("{}dude are you gonna ban yourself?", COLOR_RED));
                return true;
            }
            let pkt = Packet::new(PacketType::SERVER_LOBBY_CHOOSEBAN);
            outbox.push(OutboxMsg::SendTo(player_id, pkt.data().to_vec(), true));
        }

        "op" => {
            if op < 3 {
                deny_permission(player_id, outbox);
                return true;
            }
            if server.total_count() <= 1 {
                send_chat(outbox, player_id, &format!("{}you're already an operator tho??", COLOR_RED));
                return true;
            }
            let pkt = Packet::new(PacketType::SERVER_LOBBY_CHOOSEOP);
            outbox.push(OutboxMsg::SendTo(player_id, pkt.data().to_vec(), true));
        }

        "stop" => {
            if op < 2 {
                deny_permission(player_id, outbox);
                return true;
            }
            // game_state_tick returns before ever reaching its `end > 0.0`
            // check while !server.game.started (still waiting on client
            // ready-sync), so an end scheduled by game_end here can't start
            // counting down until every last client's ready packet arrives.
            // Only defer to game_end once the round has actually started;
            // otherwise there's no real round to end, just go straight to a
            // clean lobby.
            if server.state == GameState::Game && server.game.started && server.game.end == 0.0 {
                game_end(server, Ending::ExeWin, false, outbox);
            } else if server.state != GameState::Lobby {
                let mut lobby_outbox: Vec<OutboxMsg> = Vec::new();
                lobby_init(server, &mut lobby_outbox);
                lobby_broadcast_init(server, &mut lobby_outbox);
                outbox.append(&mut lobby_outbox);
            }
        }

        "start" => {
            if op < 2 {
                deny_permission(player_id, outbox);
                return true;
            }
            // Only valid from Lobby: a state change forced out from under
            // whatever screen players are already on (mid-round/MapVote/
            // CharSelect/Results) would leave their client desynced from the
            // server's state. Use ?stop first to get back to a clean Lobby.
            if server.state != GameState::Lobby {
                return true;
            }
            // No extra bookkeeping needed here: reaching GameState::Lobby at
            // all means lobby_init already ran and left in_game correct for
            // every connected peer.
            let map_arg = cmd.arg(0);
            if map_arg.is_empty() {
                mapvote_init(server, outbox);
            } else {
                // :start <id> / ?start --map/-m <id>: skip MapVote entirely,
                // as if map_selection were disabled or a successful .vp vote
                // had happened.
                start_with_map(map_arg, player_id, server, outbox);
            }
        }

        "chance" => {
            if op < 1 {
                deny_permission(player_id, outbox);
                return true;
            }
            let val_str = cmd.arg(0);
            let val: i32 = match val_str.parse() {
                Ok(parsed) => parsed,
                Err(_) => {
                    send_chat(outbox, player_id, &format!("{}example:~ :chance 74", COLOR_RED));
                    return true;
                }
            };
            if val < 0 || val > 100 {
                send_chat(outbox, player_id, "chance should be between 0 and 100");
                return true;
            }
            if let Some(peer) = server.find_peer_mut(player_id) {
                peer.exe_chance = val as u8;
            }
            let mut pkt = Packet::new(PacketType::SERVER_LOBBY_EXE_CHANCE);
            let _ = pkt.write_u8(val as u8);
            outbox.push(OutboxMsg::SendTo(player_id, pkt.data().to_vec(), true));
            send_chat(outbox, player_id, &format!("{}cheat code activated.{}", COLOR_CYAN, COLOR_RESET));
        }

        _ => {
            return false;
        }
    }

    true
}

/// Whether `actor` may moderate `target`: an operator can only act on someone
/// ranked strictly below them.
fn outranks(actor: u16, target: u16, server: &Server) -> bool {
    let actor_op = server.find_peer(actor).map(|peer| peer.op).unwrap_or(0);
    let target_op = match server.find_peer(target) {
        Some(peer) => peer.op,
        None    => return false,
    };
    actor_op > 0 && target_op < actor_op
}

/// Records a ban for `target_id` and drops their connection.
/// Returns their nickname, or None if they are no longer here.
fn ban_player(target_id: u16, server: &Server, outbox: &mut Vec<OutboxMsg>) -> Option<String> {
    let peer = server.find_peer(target_id)?;
    let (nick, udid, ip) = (peer.nickname.clone(), peer.udid.clone(), peer.ip.clone());
    crate::moderation::ban_add(&nick, &udid, &ip, &nick);
    outbox.push(OutboxMsg::Disconnect(target_id, DisconnectReason::BannedByHost as u32));
    Some(nick)
}

/// Starts the configured kick timeout for `target_id` and drops their connection.
/// Returns their nickname, or None if they are no longer here.
fn kick_player(target_id: u16, server: &Server, outbox: &mut Vec<OutboxMsg>) -> Option<String> {
    let peer = server.find_peer(target_id)?;
    let (nick, udid, ip) = (peer.nickname.clone(), peer.udid.clone(), peer.ip.clone());
    let window = cfg().server_config.pairing.kick_timeout_window as u64;
    crate::moderation::timeout_set(
        &nick, &udid, &ip, crate::moderation::now_unix() + window, &nick,
    );
    outbox.push(OutboxMsg::Disconnect(target_id, DisconnectReason::KickedByHost as u32));
    Some(nick)
}

/// Grants `target_id` operator status at the configured default level, both on
/// record and on the live peer. Returns their nickname, or None if they left.
fn op_player(target_id: u16, server: &mut Server) -> Option<String> {
    let peer = server.find_peer(target_id)?;
    let (nick, udid, ip) = (peer.nickname.clone(), peer.udid.clone(), peer.ip.clone());
    crate::moderation::op_add(&nick, &udid, &ip, &nick);

    let level = cfg().states.lobby_misc.moderation.op_default_level;
    if let Some(peer) = server.find_peer_mut(target_id) {
        peer.op = level;
    }
    Some(nick)
}

/// The standard refusal for a command the player's op level does not reach.
fn deny_permission(player_id: u16, outbox: &mut Vec<OutboxMsg>) {
    send_chat(outbox, player_id, &format!("{}your permission level is too low", COLOR_RED));
}

/// The standard refusal while another vote is still running. Only one vote at a
/// time exists, so starting a second one is always turned away with this.
fn deny_vote_in_progress(player_id: u16, outbox: &mut Vec<OutboxMsg>) {
    send_chat(outbox, player_id, &format!("{}another vote is already in progress.", COLOR_RED));
}

fn handle_map_vote(player_id: u16, map_idx: Option<usize>, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();

    if server.lobby.vote.ongoing {
        deny_vote_in_progress(player_id, outbox);
        return;
    }

    let cooldown = server.find_peer(player_id).map(|peer| peer.vote_cooldown).unwrap_or(0.0);
    if cooldown > 0.0 {
        send_chat(outbox, player_id, &format!("{}you cannot start another vote for {} s", COLOR_RED, (cooldown / 60.0) as i32));
        return;
    }

    let ids: Vec<u16> = server.peers.iter().map(|peer| peer.id).collect();
    if !server.lobby.vote.init(&ids, 0, VoteType::Map) {
        send_chat(outbox, player_id, &format!("{}not enough participants.", COLOR_RED));
        return;
    }

    let voting_map = map_idx.unwrap_or(20).min(crate::maps::MAP_COUNT - 1);
    server.lobby.voting_map = voting_map as i8;

    let map_name = crate::maps::MAP_LIST[voting_map].name;
    let nick = server.find_peer(player_id).map(|peer| peer.nickname.clone()).unwrap_or_default();

    broadcast_chat(outbox, "-----------------------");
    broadcast_chat(outbox, &format!("{}~ {}started map vote for {}{}.", nick, COLOR_YELLOW, map_name, COLOR_RESET));
    broadcast_chat(outbox, &format!("type {}.yes~ or ignore", COLOR_CYAN));
    broadcast_chat(outbox, &format!("results will be summarized in {}{}~ sec", COLOR_GRAY, (crate::vote::VOTE_TIMEOUT_TICKS / 60.0) as i32));
    broadcast_chat(outbox, "-----------------------");

    server.lobby.vote.add(player_id);
    if let Some(peer) = server.find_peer_mut(player_id) {
        peer.vote_cooldown = cfg.states.lobby_misc.votekick.cooldown as f64 * 60.0;
    }
}

fn handle_votekick(
    player_id: u16,
    target_id: u16,
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
) {
    let cfg = cfg();

    if server.lobby.vote.ongoing {
        // Picking the player a vote is already running against just counts as
        // voting for it; picking anyone else has to wait for that vote to end.
        let same_target = server.lobby.kick_target.as_ref()
            .map(|kick_target| kick_target.id == target_id)
            .unwrap_or(false);
        if !same_target {
            deny_vote_in_progress(player_id, outbox);
            return;
        }

        let voter_nick = server.find_peer(player_id).map(|peer| peer.nickname.clone()).unwrap_or_default();
        match server.lobby.vote.add(player_id) {
            VoteState::AlreadyVoted => {
                send_chat(outbox, player_id, &format!("{}you have already voted.", COLOR_RED));
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

    if let Some(target) = server.find_peer(target_id) {
        if target.op > 0 {
            send_chat(outbox, player_id, &format!("{}you're permissionless", COLOR_RED));
            return;
        }
    } else {
        send_chat(outbox, player_id, &format!("{}specified player not found.", COLOR_RED));
        return;
    }

    let cooldown = server.find_peer(player_id).map(|peer| peer.vote_cooldown).unwrap_or(0.0);
    if cooldown > 0.0 {
        send_chat(outbox, player_id, &format!("{}you cannot start another vote for {}s", COLOR_RED, (cooldown / 60.0) as i32));
        return;
    }

    let ids: Vec<u16> = server.peers.iter().map(|peer| peer.id).collect();
    if !server.lobby.vote.init(&ids, target_id, VoteType::Kick) {
        send_chat(outbox, player_id, &format!("{}not enough participants.", COLOR_RED));
        return;
    }

    server.lobby.kick_target = server.find_peer(target_id).map(clone_peer_data);

    server.lobby.vote.add(player_id);

    if let Some(peer) = server.find_peer_mut(player_id) {
        peer.vote_cooldown = cfg.states.lobby_misc.votekick.cooldown as f64 * 60.0;
    }

    let voter_nick = server.find_peer(player_id).map(|peer| peer.nickname.clone()).unwrap_or_default();
    let target_nick = server.find_peer(target_id).map(|peer| peer.nickname.clone()).unwrap_or_default();

    broadcast_chat(outbox, "-----------------------");
    broadcast_chat(outbox, &format!("{}~ {}started kick vote for {}{}~", voter_nick, COLOR_RED, COLOR_RESET, target_nick));
    broadcast_chat(outbox, &format!("type {}.yes~ or ignore", COLOR_CYAN));
    broadcast_chat(outbox, &format!("results will be summarized in {}{}~ sec", COLOR_GRAY, (crate::vote::VOTE_TIMEOUT_TICKS / 60.0) as i32));
    broadcast_chat(outbox, "-----------------------");
}

fn handle_vote_yes(player_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !server.lobby.vote.ongoing {
        return;
    }

    let can_vote = server.find_peer(player_id).map(|peer| peer.can_vote).unwrap_or(false);
    if !can_vote {
        send_chat(outbox, player_id, &format!("{}you can't participate in this vote.", COLOR_RED));
        return;
    }

    if let Some(kick_target) = &server.lobby.kick_target {
        if kick_target.id == player_id {
            send_chat(outbox, player_id, &format!("{}why are you kicking yourself ???", COLOR_RED));
            return;
        }
    }

    let voter_nick = server.find_peer(player_id).map(|peer| peer.nickname.clone()).unwrap_or_default();
    match server.lobby.vote.add(player_id) {
        VoteState::AlreadyVoted => {
            send_chat(outbox, player_id, &format!("{}you have already voted.", COLOR_RED));
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
    let count = server.lobby.vote.votes.len() as u8;
    let total = server.lobby.vote.votetotal;
    let vtype = server.lobby.vote.vote_type;

    let delay_ticks = cfg.states.lobby_misc.votekick.vote_result_delay as f64 * 60.0;
    if server.lobby.vote.succeeded() {
        match vtype {
            VoteType::Kick => {
                broadcast_chat(outbox, &format!("vote kick {}succeeded~ ({}{}~ out of {}{}~)", COLOR_CYAN, COLOR_CYAN, count, COLOR_CYAN, total));
                server.lobby.vote.ongoing = false;

                if let Some(kick_target) = &server.lobby.kick_target {
                    let kick_secs = cfg.server_config.pairing.kick_timeout_window as u64;
                    crate::moderation::timeout_set(&kick_target.nickname, &kick_target.udid, &kick_target.ip,
                        crate::moderation::now_unix() + kick_secs, &kick_target.nickname);
                }

                server.lobby.prac_countdown = delay_ticks;
            }
            VoteType::Map => {
                broadcast_chat(outbox, &format!("map vote {}succeeded~ ({}{}~ out of {}{}~)", COLOR_CYAN, COLOR_CYAN, count, COLOR_CYAN, total));
                server.lobby.vote.ongoing = false;
                server.lobby.prac_countdown = delay_ticks;
            }
        }
    } else {
        let label = if vtype == VoteType::Kick { "vote kick" } else { "map vote" };
        broadcast_chat(outbox, &format!("{} {}failed~ ({}{}~ out of {}{}~)", label, COLOR_RED, COLOR_RED, count, COLOR_RED, total));
        server.lobby.vote.ongoing = false;
        server.lobby.kick_target = None;
    }
}

fn clone_peer_data(source: &PeerData) -> PeerData {
    PeerData {
        id: source.id,
        ip: source.ip.clone(),
        plr: crate::player::Player::default(),
        nickname: source.nickname.clone(),
        udid: source.udid.clone(),
        lobby_icon: source.lobby_icon,
        pet: source.pet,
        verified: source.verified,
        in_game: source.in_game,
        op: source.op,
        ready: source.ready,
        mod_tool: source.mod_tool,
        is_mobile: source.is_mobile,
        auth: source.auth.clone(),
        can_vote: source.can_vote,
        voted: source.voted,
        disconnecting: source.disconnecting,
        surv_char: source.surv_char,
        exe_char: source.exe_char,
        should_timeout: source.should_timeout,
        exe_chance: source.exe_chance,
        timeout: source.timeout,
        vote_cooldown: source.vote_cooldown,
        rtt: source.rtt,
        spectate: source.spectate,
        chat_tokens: source.chat_tokens,
        chat_last: source.chat_last,
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

fn handle_votekick_legacy(player_id: u16, target_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if server.lobby.legacy_votekick_ongoing {
        if server.lobby.legacy_votekick_target == Some(target_id) {
            if !server.lobby.legacy_votekick_votes.contains(&player_id) {
                server.lobby.legacy_votekick_votes.push(player_id);
                let nick = server.find_peer(player_id).map(|peer| peer.nickname.clone()).unwrap_or_default();
                broadcast_chat(outbox, &format!("{} голосует {}за{}", nick, COLOR_CYAN, COLOR_RESET));
                check_votekick_legacy_early(server, outbox);
            }
        } else {
            send_chat(outbox, player_id, &format!("{}голосование в процессе!", COLOR_RED));
        }
        return;
    }

    match server.find_peer(target_id) {
        Some(target) if target.op > 0 => {
            send_chat(outbox, player_id, &format!("{}you're permissionless", COLOR_RED));
            return;
        }
        None => {
            send_chat(outbox, player_id, &format!("{}specified player not found.", COLOR_RED));
            return;
        }
        _ => {}
    }

    server.lobby.legacy_votekick_target = Some(target_id);
    server.lobby.legacy_votekick_votes.clear();
    server.lobby.legacy_votekick_votes.push(player_id);
    server.lobby.legacy_votekick_timer = LEGACY_VOTEKICK_TIMER_TICKS;
    server.lobby.legacy_votekick_ongoing = true;
    server.lobby.kick_target = server.find_peer(target_id).map(clone_peer_data);

    let target_nick = server.find_peer(target_id).map(|peer| peer.nickname.clone()).unwrap_or_default();
    broadcast_chat(outbox, "-----------------------");
    broadcast_chat(outbox, &format!("{}голосование начато для {}{}{}", COLOR_RED, COLOR_BLUE, target_nick, COLOR_RESET));
    broadcast_chat(outbox, &format!("напиши {}.y{} или {}.n{}", COLOR_CYAN, COLOR_RESET, COLOR_RED, COLOR_RESET));
    broadcast_chat(outbox, "-----------------------");
}

fn handle_vote_yes_legacy(player_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if server.lobby.legacy_votekick_timer <= 0.0 {
        return;
    }
    if server.lobby.legacy_votekick_votes.contains(&player_id) {
        return;
    }
    server.lobby.legacy_votekick_votes.push(player_id);

    let nick = server.find_peer(player_id).map(|peer| peer.nickname.clone()).unwrap_or_default();
    broadcast_chat(outbox, &format!("{} голосует {}за{}", nick, COLOR_CYAN, COLOR_RESET));
    check_votekick_legacy_early(server, outbox);
}

fn handle_vote_no_legacy(player_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if server.lobby.legacy_votekick_timer <= 0.0 {
        return;
    }
    server.lobby.legacy_votekick_votes.retain(|&id| id != player_id);

    let nick = server.find_peer(player_id).map(|peer| peer.nickname.clone()).unwrap_or_default();
    broadcast_chat(outbox, &format!("{} голосует {}против{}", nick, COLOR_RED, COLOR_RESET));
    check_votekick_legacy_early(server, outbox);
}

/// Resolves the vote early once every player but the target has voted yes
/// (mirrors `_voteKickVotes.Count >= server.Peers.Count(e => e.Key != target)`).
fn check_votekick_legacy_early(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let target = server.lobby.legacy_votekick_target;
    let non_target = server.peers.iter().filter(|peer| Some(peer.id) != target).count();
    if server.lobby.legacy_votekick_votes.len() >= non_target {
        check_votekick_legacy(server, false, outbox);
    }
}

fn check_votekick_legacy(server: &mut Server, ignore: bool, outbox: &mut Vec<OutboxMsg>) {
    if (server.lobby.legacy_votekick_timer <= 0.0 && !ignore) || !server.lobby.legacy_votekick_ongoing {
        return;
    }

    let target = match server.lobby.legacy_votekick_target {
        Some(found) => found,
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
        .filter(|peer| !server.lobby.legacy_votekick_votes.contains(&peer.id))
        .count();

    if count >= num {
        broadcast_chat(outbox, &format!("{}голосование успешно{} ({}{}{} и {}{}{})", COLOR_CYAN, COLOR_RESET, COLOR_CYAN, count, COLOR_RESET, COLOR_RED, num, COLOR_RESET));

        let cfg = cfg();
        if let Some(target) = server.find_peer(target) {
            let (nick, udid, ip) = (target.nickname.clone(), target.udid.clone(), target.ip.clone());
            let kick_secs = cfg.server_config.pairing.kick_timeout_window as u64;
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

fn handle_practice_legacy(player_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let nick = server.find_peer(player_id).map(|peer| peer.nickname.clone()).unwrap_or_default();

    if server.lobby.legacy_practice_ongoing {
        if server.lobby.legacy_practice_votes.contains(&player_id) {
            return;
        }
        server.lobby.legacy_practice_votes.push(player_id);
        broadcast_chat(outbox, &format!("{} хочет {}попрактиковаться{}", nick, COLOR_YELLOW, COLOR_RESET));

        if server.lobby.legacy_practice_votes.len() >= server.peers.len() {
            server.lobby.legacy_practice_ongoing = false;
            server.lobby.legacy_practice_votes.clear();

            let map: i8 = 20; // Fart Zone, the legacy practice map
            server.lobby.map = map;
            server.last_map = map;
            crate::states::char_select::charselect_init(server, outbox);
        }
        return;
    }

    server.lobby.legacy_practice_votes.clear();
    server.lobby.legacy_practice_votes.push(player_id);
    server.lobby.legacy_practice_ongoing = true;

    broadcast_chat(outbox, "-----------------------");
    broadcast_chat(outbox, &format!("{}{}голосование за практику{} начато {}{}{}-ом{}", COLOR_RED, COLOR_YELLOW, COLOR_RESET, COLOR_BLUE, nick, COLOR_RESET, COLOR_RESET));
    broadcast_chat(outbox, &format!("напиши {}.p{} для захода на тренировочную карту", COLOR_YELLOW, COLOR_RESET));
    broadcast_chat(outbox, "-----------------------");
}

// Only actual Lobby occupants (in_game) get the countdown -- someone left in
// the waiting room (tournament_mode continuing, so lobby_init kept them out)
// isn't watching it count down either. In every non-tournament case every
// peer is always in_game while a countdown is running (lobby_init promotes
// everyone), so this is a no-op filter there.
pub(crate) fn send_countdown(sec: u8, server: &Server, outbox: &mut Vec<OutboxMsg>) {
    let is_counting: u8 = if sec < NO_COUNTDOWN { 1 } else { 0 };
    let mut pkt = Packet::new(PacketType::SERVER_LOBBY_COUNTDOWN);
    let _ = pkt.write_u8(is_counting);
    let _ = pkt.write_u8(sec);
    for peer in server.peers.iter().filter(|peer| peer.in_game) {
        outbox.push(OutboxMsg::SendTo(peer.id, pkt.data().to_vec(), true));
    }
}

fn check_countdown(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    check_countdown_ex(0, server, outbox);
}

fn check_countdown_ex(exclude: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    // A tournament round hand-off is in progress: the next round starts when
    // every participant has fetched its roster (see tournament_advance), not
    // when people are ready. Leave the ordinary countdown out of it.
    if server.lobby.tournament_owed > 0 || server.lobby.tournament_wait > 0.0 { return; }

    let cfg = cfg();
    let total = server.peers.iter().filter(|peer| peer.id != exclude).count();
    let ready_count = server.peers.iter().filter(|peer| peer.ready && peer.id != exclude).count();

    if total < 2 {
        if server.lobby.countdown_sec < NO_COUNTDOWN {
            server.lobby.countdown_sec = NO_COUNTDOWN;
            server.lobby.countdown = 0.0;
            send_countdown(NO_COUNTDOWN, server, outbox);
        }
        return;
    }

    let required_pct = cfg.states.lobby_misc.lobby_ready_required_percentage as f64 / 100.0;
    let required = ((total as f64 * required_pct).ceil() as usize).max(1);

    if ready_count >= required && server.lobby.countdown_sec >= NO_COUNTDOWN {
        let timer = cfg.states.lobby_misc.lobby_start_timer;
        server.lobby.countdown_sec = timer;
        server.lobby.countdown = timer as f64 * 60.0;
        send_countdown(timer, server, outbox);
    } else if ready_count < required && server.lobby.countdown_sec < NO_COUNTDOWN {
        server.lobby.countdown_sec = NO_COUNTDOWN;
        server.lobby.countdown = 0.0;
        send_countdown(NO_COUNTDOWN, server, outbox);
    }
}

fn find_player_by_name_or_id(name_or_id: &str, server: &Server, exclude: u16) -> Option<u16> {
    if let Ok(id) = name_or_id.parse::<u16>() {
        if server.find_peer(id).is_some() && id != exclude {
            return Some(id);
        }
    }
    let lower = name_or_id.to_lowercase();
    server
        .peers
        .iter()
        .find(|peer| peer.id != exclude && peer.nickname.to_lowercase() == lower)
        .map(|peer| peer.id)
}

pub fn lobby_state_tick(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();

    // Fallback for the round hand-off (see tournament_advance): a client that
    // never asks for its roster can't be allowed to stall the bracket, or to
    // leave everyone else sitting unready long enough for the AFK check below
    // to kick the lot of them.
    if server.lobby.tournament_wait > 0.0 {
        server.lobby.tournament_wait -= 1.0;
        if server.lobby.tournament_wait <= 0.0 {
            server.lobby.tournament_wait = 0.0;
            server.lobby.tournament_owed = 0;
            mapvote_init(server, outbox);
            return;
        }
    }

    for peer in server.peers.iter_mut() {
        if peer.vote_cooldown > 0.0 {
            peer.vote_cooldown -= 1.0;
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
    for peer in server.peers.iter_mut() {
        if peer.op >= 2 || peer.ready {
            peer.timeout = 0.0;
            continue;
        }
        if peer.should_timeout {
            peer.timeout += 1.0;
            if peer.timeout >= timeout_ticks {
                to_kick.push(peer.id);
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
            if let Some(kick_target) = server.lobby.kick_target.take() {
                let kick_secs = cfg.server_config.pairing.kick_timeout_window as u64;
                crate::moderation::timeout_set(&kick_target.nickname, &kick_target.udid, &kick_target.ip,
                    crate::moderation::now_unix() + kick_secs, &kick_target.nickname);
                outbox.push(OutboxMsg::Disconnect(kick_target.id, DisconnectReason::KickedByHost as u32));
                broadcast_chat(outbox, &format!("\\c{} was kicked.", kick_target.nickname));
            } else if server.lobby.voting_map >= 0 {
                let map = server.lobby.voting_map;
                server.lobby.voting_map = -1;
                server.lobby.map = map;
                crate::states::char_select::charselect_init(server, outbox);
            }
        }
    }

    if server.lobby.countdown_sec < NO_COUNTDOWN && server.lobby.countdown > 0.0 {
        server.lobby.countdown -= 1.0;

        let new_sec = (server.lobby.countdown / 60.0).ceil() as u8;
        if new_sec < server.lobby.countdown_sec {
            server.lobby.countdown_sec = new_sec;
            send_countdown(new_sec, server, outbox);
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
            .filter(|peer| !peer.ready)
            .map(|peer| peer.id)
            .collect();
        for id in unready {
            outbox.push(OutboxMsg::Disconnect(id, DisconnectReason::KickedByHost as u32));
            server.peers.retain(|peer| peer.id != id);
        }
    }

    mapvote_init(server, outbox);
}
