use crate::config::cfg;
use crate::packet::{Packet, PacketType};
use crate::server::{
    ExeChar, GameState, OutboxMsg, Server, SurvChar,
};
use crate::states::game::game_init;

use rand::Rng;

pub fn charselect_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    log::debug!("Entering CharSelect state...");
    let cfg = cfg();
    server.state = GameState::CharSelect;

    server.lobby.chars_available = [true; 6];
    for peer in server.peers.iter_mut() {
        peer.surv_char = SurvChar::None;
        peer.exe_char = ExeChar::None;
    }

    let exe_id = pick_exe(server);
    server.lobby.exe = exe_id;
    if let Some(peer) = server.find_peer(exe_id) {
        log::info!("{} (id {}, c {}) is exe!", crate::colors::colorize(&peer.nickname), exe_id, peer.exe_chance);
    }

    let timer = cfg.states.character_selection.charselect_timer;
    let map   = server.lobby.map;

    let mut pkt_exe = Packet::new(PacketType::SERVER_LOBBY_EXE);
    let _ = pkt_exe.write_u16(exe_id);
    let _ = pkt_exe.write_u16(map as u16);
    outbox.push(OutboxMsg::Broadcast(pkt_exe.data().to_vec(), true));

    let mut pkt_time = Packet::new(PacketType::SERVER_CHAR_TIME_SYNC);
    let _ = pkt_time.write_u8(timer);
    outbox.push(OutboxMsg::Broadcast(pkt_time.data().to_vec(), true));

    // Anyone who was spectating the round before this one is still spectating.
    // The two broadcasts above already carried their client here with everyone
    // else; this gives them the body the next round needs them to have.
    crate::states::spectate::rejoin(server, outbox);

    for peer in server.peers.iter_mut() {
        if peer.id == exe_id {
            peer.exe_chance = 1;
        } else if peer.exe_chance < 100 {
            peer.exe_chance += 1;
        }
    }

    server.lobby.countdown = 60.0;
    server.lobby.countdown_sec = timer;

    log::info!("Server is now in Character Select");

    if !cfg.states.character_selection.enable {
        let mut rng = rand::thread_rng();

        for peer in server.peers.iter_mut() {
            if !peer.in_game { continue; }
            if peer.id == exe_id {
                let roll = rng.gen_range(0i8..4i8);
                peer.exe_char = ExeChar::from_i8(roll);
            } else {
                let roll = rng.gen_range(0i8..6i8);
                peer.surv_char = SurvChar::from_i8(roll);
            }
        }

        let peers_data: Vec<(u16, ExeChar, SurvChar)> = server.peers.iter()
            .filter(|peer| peer.in_game)
            .map(|peer| (peer.id, peer.exe_char, peer.surv_char))
            .collect();

        for &(pid, exe_char, surv_char) in &peers_data {
            if pid == exe_id {
                let mut resp = Packet::new(PacketType::SERVER_LOBBY_EXECHARACTER_RESPONSE);
                let _ = resp.write_u8(exe_char as i8 as u8);
                outbox.push(OutboxMsg::SendTo(pid, resp.data().to_vec(), true));
            } else {
                let mut resp = Packet::new(PacketType::SERVER_LOBBY_CHARACTER_RESPONSE);
                let _ = resp.write_u8(surv_char as i8 as u8 + 1);
                let _ = resp.write_u8(1);
                outbox.push(OutboxMsg::SendTo(pid, resp.data().to_vec(), true));
            }

            let char_val = if pid == exe_id {
                exe_char as i8 as u8
            } else {
                surv_char as i8 as u8 + 1
            };
            let mut char_change_pkt = Packet::new(PacketType::SERVER_LOBBY_CHARACTER_CHANGE);
            let _ = char_change_pkt.write_u16(pid);
            let _ = char_change_pkt.write_u8(char_val);
            outbox.push(OutboxMsg::Broadcast(char_change_pkt.data().to_vec(), true));
        }

        let exe = server.lobby.exe as i32;
        game_init(exe, map, server, outbox);
        return;
    }
}

fn pick_exe(server: &Server) -> u16 {
    let mut ingame: Vec<&crate::server::PeerData> = server.peers.iter()
        .filter(|peer| peer.in_game)
        .collect();

    if ingame.is_empty() {
        return server.peers.first().map(|peer| peer.id).unwrap_or(1);
    }

    for peer in &ingame {
        if peer.exe_chance >= 100 && !peer.mod_tool {
            return peer.id;
        }
    }

    let eligible: Vec<&crate::server::PeerData> = ingame.iter()
        .copied()
        .filter(|peer| !peer.mod_tool)
        .collect();
    if !eligible.is_empty() {
        ingame = eligible;
    }

    let total_weight: u32 = ingame.iter().map(|peer| peer.exe_chance as u32).sum();
    let weight = if total_weight == 0 { 1 } else { total_weight };

    let mut rng = rand::thread_rng();
    let mut roll: u32 = rng.gen_range(0..weight);

    for peer in &ingame {
        if roll < peer.exe_chance as u32 {
            return peer.id;
        }
        roll -= peer.exe_chance as u32;
    }

    ingame.last().map(|peer| peer.id).unwrap_or(1)
}

fn charselect_check_state(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let all_selected = server.peers.iter()
        .filter(|peer| peer.in_game)
        .all(|peer| peer.exe_char != ExeChar::None || peer.surv_char != SurvChar::None);

    if all_selected {
        let cfg = cfg();
        if cfg.states.character_selection.charselect_mod_unlocked && !cfg.states.gameplay.hide_player_characters {
            let survivors: Vec<(u16, SurvChar)> = server.peers.iter()
                .filter(|peer| peer.in_game && peer.id != server.lobby.exe)
                .map(|peer| (peer.id, peer.surv_char))
                .collect();

            for (pid, surv_char) in survivors {
                let mut char_change_pkt = Packet::new(PacketType::SERVER_LOBBY_CHARACTER_CHANGE);
                let _ = char_change_pkt.write_u16(pid);
                let _ = char_change_pkt.write_u8(surv_char as i8 as u8 + 1);
                outbox.push(OutboxMsg::Broadcast(char_change_pkt.data().to_vec(), true));
            }
        }

        let map = server.lobby.map;
        let exe = server.lobby.exe as i32;
        game_init(exe, map, server, outbox);
    }
}

pub fn charselect_state_join(player_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    use crate::states::waiting_room as wr;
    let exe_id = server.lobby.exe as i32;
    let map    = server.lobby.map;
    wr::send_waiting_player_list(player_id, exe_id, server, outbox);
    wr::announce_waiter_joined(player_id, server, outbox);
    wr::send_waiting_room_greeting(player_id, Some(map), server, outbox);
}

pub fn charselect_state_left(player_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if let Some(peer) = server.find_peer(player_id) {
        if peer.surv_char != SurvChar::None {
            server.lobby.chars_available[peer.surv_char as usize] = true;
        }
    }

    let mut pkt = Packet::new(PacketType::SERVER_PLAYER_LEFT);
    let _ = pkt.write_u16(player_id);
    outbox.push(OutboxMsg::BroadcastEx(pkt.data().to_vec(), true, player_id));

    let min_to_continue = cfg().states.lobby_misc.min_players_required.max(1) as usize;
    let remaining = server.peers.iter().filter(|peer| peer.in_game && peer.id != player_id).count();
    if remaining < min_to_continue || player_id == server.lobby.exe {
        crate::states::lobby::lobby_init(server, outbox);
        crate::states::lobby::lobby_broadcast_init(server, outbox);
        return;
    }

    charselect_check_state(server, outbox);
}

pub fn charselect_state_handle(
    player_id: u16,
    packet: &mut Packet,
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
) {
    let packet_type = match packet.packet_type() {
        Some(found) => found,
        None => return,
    };

    let cfg = cfg();

    match packet_type {
        PacketType::CLIENT_REQUEST_CHARACTER => {
            if !server.find_peer(player_id).map(|peer| peer.in_game).unwrap_or(false) { return; }

            if server.find_peer(player_id).map(|peer| peer.surv_char != SurvChar::None).unwrap_or(false) { return; }

            if server.lobby.exe == player_id { return; }

            packet.pos = 2;
            let char_1based = match packet.read_u8() { Some(value) => value, None => return };
            let char_index = (char_1based as usize).wrapping_sub(1);
            if char_index >= 6 { return; }

            let mod_unlocked = cfg.states.character_selection.charselect_mod_unlocked;
            let hide_chars   = cfg.states.gameplay.hide_player_characters;

            let available = server.lobby.chars_available[char_index];

            if available && !mod_unlocked {
                server.lobby.chars_available[char_index] = false;
            }

            let mut resp = Packet::new(PacketType::SERVER_LOBBY_CHARACTER_RESPONSE);
            let _ = resp.write_u8(char_1based);
            let _ = resp.write_u8(if available { 1 } else { 0 });
            outbox.push(OutboxMsg::SendTo(player_id, resp.data().to_vec(), true));

            if available || mod_unlocked {
                if let Some(peer) = server.find_peer_mut(player_id) {
                    peer.surv_char = SurvChar::from_i8(char_index as i8);
                }
                if let Some(peer) = server.find_peer(player_id) {
                    log::info!("{} (id {}) choses [{:?}]!", crate::colors::colorize(&peer.nickname), player_id, peer.surv_char);
                }

                if !hide_chars && !mod_unlocked {
                    let mut char_change_pkt = Packet::new(PacketType::SERVER_LOBBY_CHARACTER_CHANGE);
                    let _ = char_change_pkt.write_u16(player_id);
                    let _ = char_change_pkt.write_u8(char_1based);
                    outbox.push(OutboxMsg::Broadcast(char_change_pkt.data().to_vec(), true));
                }

                charselect_check_state(server, outbox);
            }
        }

        PacketType::CLIENT_REQUEST_EXECHARACTER => {
            if !server.find_peer(player_id).map(|peer| peer.in_game).unwrap_or(false) { return; }
            let is_exe = server.lobby.exe == player_id;
            if !is_exe && !cfg.states.character_selection.allow_foreign_characters { return; }

            packet.pos = 2;
            let char_1based = match packet.read_u8() { Some(value) => value, None => return };
            let char_0based = (char_1based as i8).wrapping_sub(1);

            if let Some(peer) = server.find_peer_mut(player_id) {
                peer.exe_char = ExeChar::from_i8(char_0based);
            }
            if let Some(peer) = server.find_peer(player_id) {
                log::info!("{} (id {}) choses [{:?}]!", crate::colors::colorize(&peer.nickname), player_id, peer.exe_char);
            }

            let mut resp = Packet::new(PacketType::SERVER_LOBBY_EXECHARACTER_RESPONSE);
            let _ = resp.write_u8(char_0based as u8);
            outbox.push(OutboxMsg::SendTo(player_id, resp.data().to_vec(), true));

            if !cfg.states.gameplay.hide_player_characters {
                let mut char_change_pkt = Packet::new(PacketType::SERVER_LOBBY_CHARACTER_CHANGE);
                let _ = char_change_pkt.write_u16(player_id);
                let _ = char_change_pkt.write_u8(char_0based as u8);
                outbox.push(OutboxMsg::Broadcast(char_change_pkt.data().to_vec(), true));
            }

            charselect_check_state(server, outbox);
        }

        PacketType::CLIENT_CHAT_MESSAGE => {
            if !server.chat_rate_allow(player_id) { return; } // anti-flood
            packet.pos = 2;
            let _sender = packet.read_u16();
            let msg = match packet.read_str() { Some(text) => text, None => return };
            let in_game = server.find_peer(player_id).map(|peer| peer.in_game).unwrap_or(false);
            if !in_game {
                crate::states::waiting_room::handle_waiter_chat(player_id, &msg, server, outbox);
            } else {
                let nick = server.find_peer(player_id).map(|peer| peer.nickname.clone()).unwrap_or_default();
                log::info!("{} (id {}): {}", crate::colors::colorize(&nick), player_id, msg);
                let mut pkt = Packet::new(PacketType::CLIENT_CHAT_MESSAGE);
                let _ = pkt.write_u16(player_id);
                let _ = pkt.write_str(&msg);
                outbox.push(OutboxMsg::BroadcastEx(pkt.data().to_vec(), true, player_id));
            }
        }

        PacketType::CLIENT_PING => {
            let pkt = Packet::new(PacketType::SERVER_PONG);
            outbox.push(OutboxMsg::SendTo(player_id, pkt.data().to_vec(), false));
        }

        _ => {}
    }
}

pub fn charselect_state_tick(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if server.lobby.countdown <= 0.0 {
        server.lobby.countdown += 60.0;

        if server.lobby.countdown_sec == 0 {
            finish_charselect(server, outbox);
            return;
        }
        server.lobby.countdown_sec -= 1;

        if server.lobby.countdown_sec == 0 {
            let to_kick: Vec<u16> = server.peers.iter()
                .filter(|peer| peer.in_game && peer.exe_char == ExeChar::None && peer.surv_char == SurvChar::None)
                .map(|peer| peer.id)
                .collect();
            for id in to_kick {
                outbox.push(OutboxMsg::Disconnect(id, crate::server::DisconnectReason::AfkTimeout as u32));
            }
        }

        let mut pkt = Packet::new(PacketType::SERVER_CHAR_TIME_SYNC);
        let _ = pkt.write_u8(server.lobby.countdown_sec);
        outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));
    }

    server.lobby.countdown -= 1.0;
}

fn finish_charselect(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let exe_id = server.lobby.exe;
    let mut free_chars = server.lobby.chars_available;
    let mut rng = rand::thread_rng();

    for peer in server.peers.iter_mut() {
        if !peer.in_game { continue; }
        if peer.id == exe_id { continue; }
        if peer.surv_char != SurvChar::None { continue; }

        let available: Vec<usize> = (0..6).filter(|&char_id| free_chars[char_id]).collect();
        if !available.is_empty() {
            let idx = rng.gen_range(0..available.len());
            let char_index = available[idx];
            peer.surv_char = SurvChar::from_i8(char_index as i8);
            free_chars[char_index] = false;
        } else {
            peer.surv_char = SurvChar::Tails;
        }
    }
    server.lobby.chars_available = free_chars;

    if let Some(peer) = server.peers.iter_mut().find(|peer| peer.id == exe_id) {
        if peer.exe_char == ExeChar::None {
            peer.exe_char = ExeChar::Original;
        }
    }

    let map = server.lobby.map;
    let exe = server.lobby.exe as i32;
    game_init(exe, map, server, outbox);
}
