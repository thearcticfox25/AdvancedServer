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

    let voted = server.peers.iter().filter(|peer| peer.in_game && peer.voted).count();
    if voted >= in_game && server.lobby.countdown_sec > 3 {
        server.lobby.countdown = 0.0;
        server.lobby.countdown_sec = 4;
    }
}

pub fn mapvote_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    log::debug!("Entering MapVote state...");
    let cfg = cfg();
    let available = available_maps(server);

    // With map selection off there is nothing to vote on: pick one and move on.
    if !cfg.states.map_selection.enabled {
        let chosen = available[rand::thread_rng().gen_range(0..available.len())] as i8;
        server.last_map = chosen;
        server.lobby.map = chosen;
        log::info!("Map automatically chosen: {}", crate::maps::MAP_LIST[chosen as usize].name);
        charselect_init(server, outbox);
        return;
    }

    server.state = GameState::MapVote;

    let candidates = pick_vote_candidates(&available, server);
    server.lobby.maps = candidates;
    server.lobby.votes = [0u8; 3];
    server.lobby.voting_map = -1;

    log::info!("Server is now in Map Vote");
    log::info!(
        "Maps: [{}] [{}] [{}]",
        crate::maps::MAP_LIST[candidates[0] as usize].name,
        crate::maps::MAP_LIST[candidates[1] as usize].name,
        crate::maps::MAP_LIST[candidates[2] as usize].name,
    );

    for peer in server.peers.iter_mut() {
        peer.voted = false;
    }

    let timer = cfg.states.map_selection.timer;

    let mut maps_pkt = Packet::new(PacketType::SERVER_VOTE_MAPS);
    let _ = maps_pkt.write_u8(candidates[0]);
    let _ = maps_pkt.write_u8(candidates[1]);
    let _ = maps_pkt.write_u8(candidates[2]);
    outbox.push(OutboxMsg::Broadcast(maps_pkt.data().to_vec(), true));

    let mut time_pkt = Packet::new(PacketType::SERVER_VOTE_TIME_SYNC);
    let _ = time_pkt.write_u8(timer);
    outbox.push(OutboxMsg::Broadcast(time_pkt.data().to_vec(), true));

    server.lobby.countdown = 60.0;
    server.lobby.countdown_sec = timer;
}

/// Every map the config allows right now. Never returns an empty list: the
/// exclude_last_map rule is dropped before the list is allowed to run dry, and
/// map 0 is the last resort if the owner enabled no maps at all.
fn available_maps(server: &Server) -> Vec<usize> {
    let cfg = cfg();
    let enabled = |map_id: usize| cfg.states.map_selection.map_list.get(map_id).copied().unwrap_or(false);

    let available: Vec<usize> = (0..MAP_COUNT)
        .filter(|&map_id| enabled(map_id))
        .filter(|&map_id| !(cfg.states.map_selection.exclude_last_map && server.last_map == map_id as i8))
        .collect();
    if !available.is_empty() {
        return available;
    }

    let available: Vec<usize> = (0..MAP_COUNT).filter(|&map_id| enabled(map_id)).collect();
    if !available.is_empty() {
        return available;
    }

    log::error!("No maps available for automatic selection! Falling back to map 0.");
    vec![0]
}

/// Picks the three maps players will vote between.
///
/// With more than three to choose from this is weighted rejection sampling
/// against map_pickrates: a map is accepted with probability pickrate/255, so a
/// map picked recently (low pickrate after finish_map_vote's decay) is less
/// likely to come back around. Fewer than three candidates are padded by
/// repeating the first, which is what the client's three vote slots expect.
fn pick_vote_candidates(available: &[usize], server: &Server) -> [u8; 3] {
    let mut rng = rand::thread_rng();
    let mut chosen: [u8; 3] = [0, 0, 0];
    let count = available.len().min(3);

    if available.len() <= 3 {
        for slot in 0..count {
            chosen[slot] = available[slot] as u8;
        }
    } else {
        const MAX_ROLLS: u32 = 500;
        let mut used = vec![false; available.len()];

        for slot in 0..count {
            let mut accepted = None;
            for _ in 0..MAX_ROLLS {
                let idx = rng.gen_range(0..available.len());
                if used[idx] { continue; }
                let pickrate = server.map_pickrates.get(available[idx]).copied().unwrap_or(255);
                let roll = rng.gen_range(0..255);
                if roll < pickrate {
                    accepted = Some(idx);
                    break;
                }
                log::debug!("{} vs {} lost", roll, pickrate);
            }
            // Out of rolls: take the first map not already on the ballot.
            let idx = accepted.unwrap_or_else(|| {
                (0..available.len()).find(|slot| !used[*slot]).unwrap_or(0)
            });
            used[idx] = true;
            chosen[slot] = available[idx] as u8;
        }
    }

    for slot in count..3 {
        chosen[slot] = chosen[0];
    }
    chosen
}

pub fn mapvote_state_join(player_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    use crate::states::waiting_room as wr;
    wr::send_waiting_player_list(player_id, -1, server, outbox);
    wr::announce_waiter_joined(player_id, server, outbox);
    wr::send_waiting_room_greeting(player_id, None, server, outbox);
}

pub fn mapvote_state_left(player_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let mut pkt = Packet::new(PacketType::SERVER_PLAYER_LEFT);
    let _ = pkt.write_u16(player_id);
    outbox.push(OutboxMsg::BroadcastEx(pkt.data().to_vec(), true, player_id));

    let remaining = server.peers.iter().filter(|peer| peer.in_game && peer.id != player_id).count();
    let min_to_continue = cfg().states.lobby_misc.min_players_required.max(1) as usize;
    if remaining < min_to_continue {
        crate::states::lobby::lobby_init(server, outbox);
        crate::states::lobby::lobby_broadcast_init(server, outbox);
        return;
    }

    mapvote_check_state(server);
}

pub fn mapvote_state_handle(
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
        PacketType::CLIENT_VOTE_REQUEST => {
            let in_game = server.find_peer(player_id).map(|peer| peer.in_game).unwrap_or(false);
            if !in_game { return; }
            let already_voted = server.find_peer(player_id).map(|peer| peer.voted).unwrap_or(true);
            if already_voted { return; }

            packet.pos = 2;
            let map_idx = match packet.read_u8() { Some(value) => value, None => return };
            if map_idx >= 3 { return; }

            server.lobby.votes[map_idx as usize] = server.lobby.votes[map_idx as usize].saturating_add(1);
            server.lobby.voting_map = map_idx as i8;

            if let Some(peer) = server.find_peer_mut(player_id) {
                peer.voted = true;
            }

            let voted_map = server.lobby.maps[map_idx as usize];
            let nick = server.find_peer(player_id).map(|peer| peer.nickname.clone()).unwrap_or_default();
            log::info!(
                "{} (id {}) voted for [{}]!",
                crate::colors::colorize(&nick), player_id, crate::maps::MAP_LIST[voted_map as usize].name,
            );

            let mut pkt = Packet::new(PacketType::SERVER_VOTE_SET);
            let _ = pkt.write_u8(server.lobby.votes[0]);
            let _ = pkt.write_u8(server.lobby.votes[1]);
            let _ = pkt.write_u8(server.lobby.votes[2]);
            outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));

            mapvote_check_state(server);
        }

        PacketType::CLIENT_CHAT_MESSAGE => {
            if !server.chat_rate_allow(player_id) { return; } // anti-flood
            packet.pos = 2;
            let _sender = packet.read_u16();
            let msg = match packet.read_str() { Some(text) => text, None => return };

            let in_game = server.find_peer(player_id).map(|peer| peer.in_game).unwrap_or(false);
            if !in_game {
                crate::states::waiting_room::handle_waiter_chat(player_id, &msg, server, outbox);
                return;
            }

            let nick = server.find_peer(player_id).map(|peer| peer.nickname.clone()).unwrap_or_default();
            log::info!("{} (id {}): {}", crate::colors::colorize(&nick), player_id, msg);

            let trimmed = msg.trim();
            if let Some(cmd) = parse_cmd(trimmed) {
                match cmd.name.as_str() {
                    "vp" | "yes" | "y" => {
                        let op = server.find_peer(player_id).map(|peer| peer.op).unwrap_or(0);
                        if op > 0 {
                            finish_map_vote(server, outbox);
                        }
                        return;
                    }
                    _ => {}
                }
            }
            let mut pkt = Packet::new(PacketType::CLIENT_CHAT_MESSAGE);
            let _ = pkt.write_u16(player_id);
            let _ = pkt.write_str(&msg);
            outbox.push(OutboxMsg::BroadcastEx(pkt.data().to_vec(), true, player_id));
        }

        PacketType::CLIENT_PING => {
            let pkt = Packet::new(PacketType::SERVER_PONG);
            outbox.push(OutboxMsg::SendTo(player_id, pkt.data().to_vec(), false));
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
    for slot in 0..3 {
        if votes[slot] > best_votes {
            best_votes = votes[slot];
            candidates.clear();
            candidates.push(maps[slot]);
        } else if votes[slot] == best_votes {
            candidates.push(maps[slot]);
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
    // up - so a map that just won is unlikely to reappear in the next vote's
    // candidate slate.
    let winner = chosen_map as usize;
    if let Some(pickrate) = server.map_pickrates.get_mut(winner) {
        *pickrate = (*pickrate - 255).max(0);
    }
    for &offered_map in maps.iter() {
        if let Some(pickrate) = server.map_pickrates.get_mut(offered_map as usize) {
            *pickrate = (*pickrate - 25).max(0);
        }
    }
    for (map_id, pickrate) in server.map_pickrates.iter_mut().enumerate() {
        if map_id == winner { continue; }
        *pickrate = (*pickrate + 25).min(255);
    }

    log::debug!("Pickrates:");
    for (map_id, pickrate) in server.map_pickrates.iter().enumerate().take(crate::maps::MAP_COUNT) {
        log::debug!("  {}: {}", map_id, pickrate);
    }
    log::info!("Map is [{}]", crate::maps::MAP_LIST[chosen_map as usize].name);

    crate::states::waiting_room::broadcast_map_announcement(chosen_map, outbox);
    charselect_init(server, outbox);
}
