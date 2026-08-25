use crate::terminal::cmd::parse_cmd;
use crate::config::cfg;
use crate::maps::MAP_COUNT;
use crate::packet::{Packet, PacketType};
use crate::server::{GameState, OutboxMsg, Server};
use crate::states::char_select::charselect_init;

use rand::Rng;

pub fn mapvote_check_state(server: &mut Server) {
    let in_game = server.ingame_count();
    if in_game == 0 { return; }

    let voted = server.peers.iter().filter(|p| p.in_game && p.voted).count();
    if voted >= in_game && server.lobby.countdown_sec > 3 {
        server.lobby.countdown = 0.0;
        server.lobby.countdown_sec = 4;
    }
}

pub fn mapvote_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    log::debug!("Entering MapVote state...");
    let cfg = cfg();

    let mut rng = rand::thread_rng();

    let mut available: Vec<usize> = Vec::new();
    for i in 0..MAP_COUNT {
        let enabled = cfg.states.map_selection.map_list.get(i).copied().unwrap_or(false);
        if !enabled { continue; }
        if cfg.states.map_selection.exclude_last_map && server.last_map == i as i8 { continue; }
        available.push(i);
    }

    if available.is_empty() {
        for i in 0..MAP_COUNT {
            let enabled = cfg.states.map_selection.map_list.get(i).copied().unwrap_or(false);
            if enabled { available.push(i); }
        }
    }
    if available.is_empty() {
        log::error!("No maps available for automatic selection! Falling back to map 0.");
        available.push(0);
    }

    if !cfg.states.map_selection.enabled {
        let chosen = available[rng.gen_range(0..available.len())] as i8;
        server.last_map = chosen;
        server.lobby.map = chosen;
        log::info!("Map automatically chosen: {}", crate::maps::MAP_LIST[chosen as usize].name);
        charselect_init(server, outbox);
        return;
    }

    server.state = GameState::MapVote;

    let count = available.len().min(3);
    let mut chosen: [u8; 3] = [0, 0, 0];
    if available.len() <= 3 {
        for slot in 0..count {
            chosen[slot] = available[slot] as u8;
        }
    } else {
        // Weighted rejection sampling against map_pickrates: a map is accepted
        // with probability pickrate/255, so maps recently picked (low pickrate
        // after finish_map_vote's decay) are less likely to reappear.
        let mut used = vec![false; available.len()];
        for slot in 0..count {
            let mut chosen_idx = None;
            for _ in 0..500 {
                let idx = rng.gen_range(0..available.len());
                if used[idx] { continue; }
                let map = available[idx];
                let pickrate = server.map_pickrates.get(map).copied().unwrap_or(255);
                let roll = rng.gen_range(0..255);
                if roll < pickrate {
                    chosen_idx = Some(idx);
                    break;
                }
                log::debug!("{} vs {} lost", roll, pickrate);
            }
            let idx = chosen_idx.unwrap_or_else(|| {
                (0..available.len()).find(|i| !used[*i]).unwrap_or(0)
            });
            used[idx] = true;
            chosen[slot] = available[idx] as u8;
        }
    }
    for slot in count..3 {
        chosen[slot] = chosen[0];
    }

    server.lobby.maps = chosen;
    server.lobby.votes = [0u8; 3];
    server.lobby.voting_map = -1;

    log::info!("Server is now in Map Vote");
    log::info!(
        "Maps: [{}] [{}] [{}]",
        crate::maps::MAP_LIST[chosen[0] as usize].name,
        crate::maps::MAP_LIST[chosen[1] as usize].name,
        crate::maps::MAP_LIST[chosen[2] as usize].name,
    );


    for p in server.peers.iter_mut() {
        p.voted = false;
    }

    let timer = cfg.states.map_selection.timer;

    let mut pkt = Packet::new(PacketType::SERVER_VOTE_MAPS);
    let _ = pkt.write_u8(chosen[0]);
    let _ = pkt.write_u8(chosen[1]);
    let _ = pkt.write_u8(chosen[2]);
    outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));

    let mut pkt2 = Packet::new(PacketType::SERVER_VOTE_TIME_SYNC);
    let _ = pkt2.write_u8(timer);
    outbox.push(OutboxMsg::Broadcast(pkt2.data().to_vec(), true));


    server.lobby.countdown = 60.0;
    server.lobby.countdown_sec = timer;
}

pub fn mapvote_state_join(v_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    use crate::states::waiting_room as wr;
    wr::send_waiting_player_list(v_id, -1, server, outbox);
    wr::announce_waiter_joined(v_id, server, outbox);
    wr::send_waiting_room_greeting(v_id, None, server, outbox);
}

pub fn mapvote_state_left(v_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let mut pkt = Packet::new(PacketType::SERVER_PLAYER_LEFT);
    let _ = pkt.write_u16(v_id);
    outbox.push(OutboxMsg::BroadcastEx(pkt.data().to_vec(), true, v_id));

    let remaining = server.peers.iter().filter(|p| p.in_game && p.id != v_id).count();
    let min_to_continue = cfg().states.lobby_misc.min_players_required.max(1) as usize;
    if remaining < min_to_continue {
        crate::states::lobby::lobby_init(server, outbox);
        crate::states::lobby::lobby_broadcast_init(server, outbox);
        return;
    }

    mapvote_check_state(server);
}

pub fn mapvote_state_handle(
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
        PacketType::CLIENT_VOTE_REQUEST => {

            let in_game = server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false);
            if !in_game { return; }
            let already_voted = server.find_peer(v_id).map(|p| p.voted).unwrap_or(true);
            if already_voted { return; }

            packet.pos = 2;
            let map_idx = match packet.read_u8() { Some(v) => v, None => return };
            if map_idx >= 3 { return; }

            server.lobby.votes[map_idx as usize] = server.lobby.votes[map_idx as usize].saturating_add(1);
            server.lobby.voting_map = map_idx as i8;

            if let Some(pd) = server.find_peer_mut(v_id) {
                pd.voted = true;
            }

            let voted_map = server.lobby.maps[map_idx as usize];
            let nick = server.find_peer(v_id).map(|p| p.nickname.clone()).unwrap_or_default();
            log::info!(
                "{} (id {}) voted for [{}]!",
                crate::colors::colorize(&nick), v_id, crate::maps::MAP_LIST[voted_map as usize].name,
            );


            let mut pkt = Packet::new(PacketType::SERVER_VOTE_SET);
            let _ = pkt.write_u8(server.lobby.votes[0]);
            let _ = pkt.write_u8(server.lobby.votes[1]);
            let _ = pkt.write_u8(server.lobby.votes[2]);
            outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));


            mapvote_check_state(server);
        }

        PacketType::CLIENT_CHAT_MESSAGE => {
            if !server.chat_rate_allow(v_id) { return; } // anti-flood
            packet.pos = 2;
            let _sender = packet.read_u16();
            let msg = match packet.read_str() { Some(s) => s, None => return };

            let in_game = server.find_peer(v_id).map(|p| p.in_game).unwrap_or(false);
            if !in_game {
                crate::states::waiting_room::handle_waiter_chat(v_id, &msg, server, outbox);
                return;
            }

            let nick = server.find_peer(v_id).map(|p| p.nickname.clone()).unwrap_or_default();
            log::info!("{} (id {}): {}", crate::colors::colorize(&nick), v_id, msg);

            let trimmed = msg.trim();
            if let Some(cmd) = parse_cmd(trimmed) {
                match cmd.name.as_str() {
                    "vp" | "yes" | "y" => {
                        let op = server.find_peer(v_id).map(|p| p.op).unwrap_or(0);
                        if op > 0 {
                            finish_map_vote(server, outbox);
                        }
                        return;
                    }
                    _ => {}
                }
            }
            let mut pkt = Packet::new(PacketType::CLIENT_CHAT_MESSAGE);
            let _ = pkt.write_u16(v_id);
            let _ = pkt.write_str(&msg);
            outbox.push(OutboxMsg::BroadcastEx(pkt.data().to_vec(), true, v_id));
        }

        PacketType::CLIENT_PING => {
            let mut pkt = Packet::new(PacketType::SERVER_PONG);
            outbox.push(OutboxMsg::SendTo(v_id, pkt.data().to_vec(), false));
        }

        _ => {}
    }
}

pub fn mapvote_state_tick(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if server.lobby.countdown <= 0.0 {
        server.lobby.countdown += 60.0;

        if server.lobby.countdown_sec == 0 {
            finish_map_vote(server, outbox);
            return;
        }
        server.lobby.countdown_sec -= 1;
        if server.lobby.countdown_sec == 0 {
            finish_map_vote(server, outbox);
            return;
        }


        let mut pkt = Packet::new(PacketType::SERVER_VOTE_TIME_SYNC);
        let _ = pkt.write_u8(server.lobby.countdown_sec);
        outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));
    }

    server.lobby.countdown -= 1.0;
}

fn finish_map_vote(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {

    let votes = server.lobby.votes;
    let maps  = server.lobby.maps;

    let mut best_votes = 0u8;
    let mut candidates: Vec<u8> = Vec::new();
    for i in 0..3 {
        if votes[i] > best_votes {
            best_votes = votes[i];
            candidates.clear();
            candidates.push(maps[i]);
        } else if votes[i] == best_votes {
            candidates.push(maps[i]);
        }
    }

    let chosen_map = if candidates.is_empty() {
        maps[0] as i8
    } else {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        candidates[rng.gen_range(0..candidates.len())] as i8
    };

    server.lobby.map = chosen_map;
    server.last_map = chosen_map;

    // Pickrate decay: the winner drops hard, the other two candidates on the
    // ballot drop a little, everything else not picked this round drifts back
    // up — so a map that just won is unlikely to reappear in the next vote's
    // candidate slate.
    let won = chosen_map as usize;
    if let Some(p) = server.map_pickrates.get_mut(won) {
        *p = (*p - 255).max(0);
    }
    for &m in maps.iter() {
        if let Some(p) = server.map_pickrates.get_mut(m as usize) {
            *p = (*p - 25).max(0);
        }
    }
    for (i, p) in server.map_pickrates.iter_mut().enumerate() {
        if i == won { continue; }
        *p = (*p + 25).min(255);
    }

    log::debug!("Pickrates:");
    for (i, p) in server.map_pickrates.iter().enumerate().take(crate::maps::MAP_COUNT) {
        log::debug!("  {}: {}", i, p);
    }
    log::info!("Map is [{}]", crate::maps::MAP_LIST[chosen_map as usize].name);

    crate::states::waiting_room::broadcast_map_announcement(chosen_map, outbox);
    charselect_init(server, outbox);
}
