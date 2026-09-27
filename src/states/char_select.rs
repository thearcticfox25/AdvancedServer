use crate::config::cfg;
use crate::packet::{Packet, PacketType};
use crate::server::{ExeChar, OutboxMsg, Server, SurvChar};
use crate::states::game::game_init;

use rand::Rng;

/// The characters of the round, handed out as soon as the vote has chosen the map:
/// the EXE are drawn, everyone gets what their preference cards asked for (or a
/// random character), and the round starts at once. The original ran a separate
/// stage here where players picked; in this game the picking happens in the lobby.
pub fn charselect_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    log::debug!("Handing out the characters...");
    for peer in server.peers.iter_mut() {
        peer.surv_char = SurvChar::None;
        peer.exe_char = ExeChar::None;
    }

    let exes = pick_exes(server);
    for &exe_id in &exes {
        if let Some(peer) = server.find_peer(exe_id) {
            log::info!("{} (id {}, c {}) is exe!", crate::colors::colorize(&peer.nickname), exe_id, peer.exe_chance);
        }
    }
    server.lobby.exes = exes.clone();
    let exe_id = server.lobby.main_exe();
    let map = server.lobby.map;

    let mut pkt_exe = Packet::new(PacketType::SERVER_LOBBY_EXE);
    let _ = pkt_exe.write_u16(exe_id);
    let _ = pkt_exe.write_u16(map as u16);
    // The other EXE after the fields the original client reads.
    let _ = pkt_exe.write_u8((exes.len() - 1) as u8);
    for &other_exe in &exes[1..] {
        let _ = pkt_exe.write_u16(other_exe);
    }
    outbox.push(OutboxMsg::Broadcast(pkt_exe.data().to_vec(), true));

    for peer in server.peers.iter_mut() {
        if exes.contains(&peer.id) {
            peer.exe_chance = 1;
        } else if peer.exe_chance < 100 {
            peer.exe_chance += 1;
        }
    }

    assign_preferred_characters(&exes, server);
    send_characters(&exes, server, outbox);

    // ponytail: the round rules below know one EXE; the others become EXE there with the authoritative round (core)
    game_init(exe_id as i32, map, server, outbox);
}

/// Tells every player their own character and everyone else's.
fn send_characters(exes: &[u16], server: &Server, outbox: &mut Vec<OutboxMsg>) {
    for peer in server.peers.iter().filter(|peer| peer.in_game) {
        let is_exe = exes.contains(&peer.id);
        let character = if is_exe { peer.exe_char as i8 as u8 } else { peer.surv_char as i8 as u8 + 1 };
        if is_exe {
            let mut resp = Packet::new(PacketType::SERVER_LOBBY_EXECHARACTER_RESPONSE);
            let _ = resp.write_u8(character);
            outbox.push(OutboxMsg::SendTo(peer.id, resp.data().to_vec(), true));
        } else {
            let mut resp = Packet::new(PacketType::SERVER_LOBBY_CHARACTER_RESPONSE);
            let _ = resp.write_u8(character);
            let _ = resp.write_u8(1);
            outbox.push(OutboxMsg::SendTo(peer.id, resp.data().to_vec(), true));
        }

        let mut char_change_pkt = Packet::new(PacketType::SERVER_LOBBY_CHARACTER_CHANGE);
        let _ = char_change_pkt.write_u16(peer.id);
        let _ = char_change_pkt.write_u8(character);
        outbox.push(OutboxMsg::Broadcast(char_change_pkt.data().to_vec(), true));
    }
}

/// Every player gets what their preference
/// cards asked for when getting ready. A card without a wish, or with a character
/// that went to a player earlier in the list while characters are not shared,
/// gets a random character instead, as this mode always did.
fn assign_preferred_characters(exes: &[u16], server: &mut Server) {
    let duplicates_allowed = cfg().states.lobby_misc.character_selection.charselect_mod_unlocked;
    // With one EXE nobody else can hold its character.
    let exes_share = duplicates_allowed || exes.len() == 1;
    let mut free_survivors = [true; SURVIVOR_COUNT];
    let mut free_exes = [true; EXE_COUNT];
    let mut rng = rand::thread_rng();

    for peer in server.peers.iter_mut().filter(|peer| peer.in_game) {
        if exes.contains(&peer.id) {
            let wanted = peer.preferred_exe;
            let granted = wanted != ExeChar::None && (exes_share || free_exes[wanted as usize]);
            let exe = if granted { wanted as usize } else { random_character(&free_exes, exes_share, &mut rng) };
            peer.exe_char = ExeChar::from_i8(exe as i8);
            free_exes[exe] = false;
            continue;
        }
        let wanted = peer.preferred_survivor;
        let granted = wanted != SurvChar::None && (duplicates_allowed || free_survivors[wanted as usize]);
        let survivor = if granted { wanted as usize } else { random_character(&free_survivors, duplicates_allowed, &mut rng) };
        peer.surv_char = SurvChar::from_i8(survivor as i8);
        free_survivors[survivor] = false;
    }
}

/// A random character number; only a free one unless characters are shared. The
/// config check keeps enough characters free; the first one if not.
fn random_character(free: &[bool], shared: bool, rng: &mut impl Rng) -> usize {
    let choices: Vec<usize> = (0..free.len()).filter(|&character| shared || free[character]).collect();
    if choices.is_empty() {
        return 0;
    }
    choices[rng.gen_range(0..choices.len())]
}

const SURVIVOR_COUNT: usize = 6;
const EXE_COUNT: usize = 4;

/// The round's EXE: exe_count of them, but at least one survivor stays. Each is the
/// original weighted draw among the players not drawn yet; players whose card
/// says "not me" are drawn only when nobody else is left.
fn pick_exes(server: &Server) -> Vec<u16> {
    let players = server.peers.iter().filter(|peer| peer.in_game).count();
    let count = (cfg().states.lobby_misc.character_selection.exe_count as usize).min(players.saturating_sub(1)).max(1);
    let mut exes = Vec::with_capacity(count);
    while exes.len() < count {
        exes.push(pick_exe(server, &exes));
    }
    exes
}

fn pick_exe(server: &Server, already_exe: &[u16]) -> u16 {
    let not_drawn: Vec<&crate::server::PeerData> = server.peers.iter()
        .filter(|peer| peer.in_game && !already_exe.contains(&peer.id))
        .collect();
    let willing: Vec<&crate::server::PeerData> = not_drawn.iter().copied().filter(|peer| !peer.refuses_exe).collect();
    let mut ingame = if willing.is_empty() { not_drawn } else { willing };

    if ingame.is_empty() {
        return server.peers.first().map(|peer| peer.id).unwrap_or(1);
    }

    for peer in &ingame {
        if peer.exe_chance >= 100 {
            return peer.id;
        }
    }

    let eligible: Vec<&crate::server::PeerData> = ingame.iter()
        .copied()
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
