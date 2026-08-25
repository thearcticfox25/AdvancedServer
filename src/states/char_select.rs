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


    server.lobby.avail = [true; 6];
    for p in server.peers.iter_mut() {
        p.surv_char = SurvChar::None;
        p.exe_char = ExeChar::None;
    }


    let exe_id = pick_exe(server);
    server.lobby.exe = exe_id;
    if let Some(p) = server.find_peer(exe_id) {
        log::info!("{} (id {}, c {}) is exe!", crate::colors::colorize(&p.nickname), exe_id, p.exe_chance);
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


    for p in server.peers.iter_mut() {
        if p.id == exe_id {
            p.exe_chance = 1;
        } else if p.exe_chance < 100 {
            p.exe_chance += 1;
        }
    }


    server.lobby.countdown = 60.0;
    server.lobby.countdown_sec = timer;

    log::info!("Server is now in Character Select");

    if !cfg.states.character_selection.enable {
        let mut rng = rand::thread_rng();

        for p in server.peers.iter_mut() {
            if !p.in_game { continue; }
            if p.id == exe_id {
                let c = rng.gen_range(0i8..4i8);
                p.exe_char = ExeChar::from_i8(c);
            } else {
                let c = rng.gen_range(0i8..6i8);
                p.surv_char = SurvChar::from_i8(c);
            }
        }

        let peers_data: Vec<(u16, ExeChar, SurvChar)> = server.peers.iter()
            .filter(|p| p.in_game)
            .map(|p| (p.id, p.exe_char, p.surv_char))
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
            let mut chg = Packet::new(PacketType::SERVER_LOBBY_CHARACTER_CHANGE);
            let _ = chg.write_u16(pid);
            let _ = chg.write_u8(char_val);
            outbox.push(OutboxMsg::Broadcast(chg.data().to_vec(), true));
        }

        let exe = server.lobby.exe as i32;
        game_init(exe, map, server, outbox);
        return;
    }
}

fn pick_exe(server: &Server) -> u16 {
    let mut ingame: Vec<&crate::server::PeerData> = server.peers.iter()
        .filter(|p| p.in_game)
        .collect();

    if ingame.is_empty() {
        return server.peers.first().map(|p| p.id).unwrap_or(1);
    }

    for p in &ingame {
        if p.exe_chance >= 100 && !p.mod_tool {
            return p.id;
        }
    }

    let eligible: Vec<&crate::server::PeerData> = ingame.iter()
        .copied()
        .filter(|p| !p.mod_tool)
        .collect();
    if !eligible.is_empty() {
        ingame = eligible;
    }

    let total_weight: u32 = ingame.iter().map(|p| p.exe_chance as u32).sum();
    let weight = if total_weight == 0 { 1 } else { total_weight };

    let mut rng = rand::thread_rng();
    let mut roll: u32 = rng.gen_range(0..weight);

    for p in &ingame {
        if roll < p.exe_chance as u32 {
            return p.id;
        }
        roll -= p.exe_chance as u32;
    }

    ingame.last().map(|p| p.id).unwrap_or(1)
}

fn charselect_check_state(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let all_selected = server.peers.iter()
        .filter(|p| p.in_game)
        .all(|p| p.exe_char != ExeChar::None || p.surv_char != SurvChar::None);

    if all_selected {
        let cfg = cfg();
        if cfg.states.character_selection.charselect_mod_unlocked && !cfg.states.gameplay.hide_player_characters {
            let survivors: Vec<(u16, SurvChar)> = server.peers.iter()
                .filter(|p| p.in_game && p.id != server.lobby.exe)
                .map(|p| (p.id, p.surv_char))
                .collect();

            for (pid, surv_char) in survivors {
                let mut chg = Packet::new(PacketType::SERVER_LOBBY_CHARACTER_CHANGE);
                let _ = chg.write_u16(pid);
                let _ = chg.write_u8(surv_char as i8 as u8 + 1);
                outbox.push(OutboxMsg::Broadcast(chg.data().to_vec(), true));
            }
        }

        let map = server.lobby.map;
        let exe = server.lobby.exe as i32;
        game_init(exe, map, server, outbox);
    }
}

pub fn charselect_state_join(v_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    use crate::states::waiting_room as wr;
    let exe_id = server.lobby.exe as i32;
    let map    = server.lobby.map;
    wr::send_waiting_player_list(v_id, exe_id, server, outbox);
    wr::announce_waiter_joined(v_id, server, outbox);
    wr::send_waiting_room_greeting(v_id, Some(map), server, outbox);
}

pub fn charselect_state_left(v_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {

    if let Some(pd) = server.find_peer(v_id) {
        if pd.surv_char != SurvChar::None {
            server.lobby.avail[pd.surv_char as usize] = true;
        }
    }

    let mut pkt = Packet::new(PacketType::SERVER_PLAYER_LEFT);
    let _ = pkt.write_u16(v_id);
    outbox.push(OutboxMsg::BroadcastEx(pkt.data().to_vec(), true, v_id));

    let min_to_continue = cfg().states.lobby_misc.min_players_required.max(1) as usize;
    let remaining = server.peers.iter().filter(|p| p.in_game && p.id != v_id).count();
    if remaining < min_to_continue || v_id == server.lobby.exe {
        crate::states::lobby::lobby_init(server, outbox);
        crate::states::lobby::lobby_broadcast_init(server, outbox);
        return;
    }


    charselect_check_state(server, outbox);
}

pub fn charselect_state_handle(
    v_id: u16,
    packet: &mut Packet,
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
) {
    let ptype = match packet.packet_type() {
        Some(t) => t,
        None => return,
    };

    let cfg = cfg();

    match ptype {
        PacketType::CLIENT_REQUEST_CHARACTER => {

            if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }

            if server.find_peer(v_id).map(|p| p.surv_char != SurvChar::None).unwrap_or(false) { return; }

            if server.lobby.exe == v_id { return; }

            packet.pos = 2;
            let char_1based = match packet.read_u8() { Some(v) => v, None => return };
            let cidx = (char_1based as usize).wrapping_sub(1);
            if cidx >= 6 { return; }

            let mod_unlocked = cfg.states.character_selection.charselect_mod_unlocked;
            let hide_chars   = cfg.states.gameplay.hide_player_characters;

            let avail = server.lobby.avail[cidx];

            if avail && !mod_unlocked {
                server.lobby.avail[cidx] = false;
            }


            let mut resp = Packet::new(PacketType::SERVER_LOBBY_CHARACTER_RESPONSE);
            let _ = resp.write_u8(char_1based);
            let _ = resp.write_u8(if avail { 1 } else { 0 });
            outbox.push(OutboxMsg::SendTo(v_id, resp.data().to_vec(), true));

            if avail || mod_unlocked {
                if let Some(pd) = server.find_peer_mut(v_id) {
                    pd.surv_char = SurvChar::from_i8(cidx as i8);
                }
                if let Some(p) = server.find_peer(v_id) {
                    log::info!("{} (id {}) choses [{:?}]!", crate::colors::colorize(&p.nickname), v_id, p.surv_char);
                }

                if !hide_chars && !mod_unlocked {
                    let mut chg = Packet::new(PacketType::SERVER_LOBBY_CHARACTER_CHANGE);
                    let _ = chg.write_u16(v_id);
                    let _ = chg.write_u8(char_1based);
                    outbox.push(OutboxMsg::Broadcast(chg.data().to_vec(), true));
                }

                charselect_check_state(server, outbox);
            }
        }

        PacketType::CLIENT_REQUEST_EXECHARACTER => {

            if !server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false) { return; }
            let is_exe = server.lobby.exe == v_id;
            if !is_exe && !cfg.states.character_selection.allow_foreign_characters { return; }

            packet.pos = 2;
            let char_1based = match packet.read_u8() { Some(v) => v, None => return };
            let char_0based = (char_1based as i8).wrapping_sub(1);

            if let Some(pd) = server.find_peer_mut(v_id) {
                pd.exe_char = ExeChar::from_i8(char_0based);
            }
            if let Some(p) = server.find_peer(v_id) {
                log::info!("{} (id {}) choses [{:?}]!", crate::colors::colorize(&p.nickname), v_id, p.exe_char);
            }


            let mut resp = Packet::new(PacketType::SERVER_LOBBY_EXECHARACTER_RESPONSE);
            let _ = resp.write_u8(char_0based as u8);
            outbox.push(OutboxMsg::SendTo(v_id, resp.data().to_vec(), true));

            if !cfg.states.gameplay.hide_player_characters {
                let mut chg = Packet::new(PacketType::SERVER_LOBBY_CHARACTER_CHANGE);
                let _ = chg.write_u16(v_id);
                let _ = chg.write_u8(char_0based as u8);
                outbox.push(OutboxMsg::Broadcast(chg.data().to_vec(), true));
            }

            charselect_check_state(server, outbox);
        }

        PacketType::CLIENT_CHAT_MESSAGE => {
            if !server.chat_rate_allow(v_id) { return; } // anti-flood
            packet.pos = 2;
            let _sender = packet.read_u16();
            let msg = match packet.read_str() { Some(s) => s, None => return };
            let in_game = server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false);
            if !in_game {
                crate::states::waiting_room::handle_waiter_chat(v_id, &msg, server, outbox);
            } else {
                let nick = server.find_peer(v_id).map(|p| p.nickname.clone()).unwrap_or_default();
                log::info!("{} (id {}): {}", crate::colors::colorize(&nick), v_id, msg);
                let mut pkt = Packet::new(PacketType::CLIENT_CHAT_MESSAGE);
                let _ = pkt.write_u16(v_id);
                let _ = pkt.write_str(&msg);
                outbox.push(OutboxMsg::BroadcastEx(pkt.data().to_vec(), true, v_id));
            }
        }

        PacketType::CLIENT_PING => {
            let mut pkt = Packet::new(PacketType::SERVER_PONG);
            outbox.push(OutboxMsg::SendTo(v_id, pkt.data().to_vec(), false));
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
                .filter(|p| p.in_game && p.exe_char == ExeChar::None && p.surv_char == SurvChar::None)
                .map(|p| p.id)
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
    let mut avail_copy = server.lobby.avail;
    let mut rng = rand::thread_rng();

    for p in server.peers.iter_mut() {
        if !p.in_game { continue; }
        if p.id == exe_id { continue; }
        if p.surv_char != SurvChar::None { continue; }

        let available: Vec<usize> = (0..6).filter(|&i| avail_copy[i]).collect();
        if !available.is_empty() {
            let idx = rng.gen_range(0..available.len());
            let ci = available[idx];
            p.surv_char = SurvChar::from_i8(ci as i8);
            avail_copy[ci] = false;
        } else {
            p.surv_char = SurvChar::Tails;
        }
    }
    server.lobby.avail = avail_copy;


    if let Some(pd) = server.peers.iter_mut().find(|p| p.id == exe_id) {
        if pd.exe_char == ExeChar::None {
            pd.exe_char = ExeChar::Original;
        }
    }

    let map = server.lobby.map;
    let exe = server.lobby.exe as i32;
    game_init(exe, map, server, outbox);
}
