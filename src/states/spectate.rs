//! `.spectate` -- watching a round from the waiting room.
//!
//! A spectator never joins the round they are watching. They stay off the roster,
//! out of the exe draw, out of the win conditions and out of the anti-cheat, nobody
//! in the round is told they are there, and everything they send about it is dropped
//! rather than being read as a move in it (`plays_no_part`; their chat is let
//! through, because they never left the waiting room's). `peer.in_game` stays false
//! the whole time, which is what every other stage already asks to decide who is
//! playing.
//!
//! What they are sent is the round itself, the same way a player is sent it: who is
//! in it, who is exe and on which map, the gameplay numbers, and then the snapshot
//! of every tick (states::round). Their snapshot has no player of their own in it
//! (`Snapshot::own` is None) and nothing is left out of it, while a player is only
//! told about what is near them (states.gameplay.limit_player_view). Their client
//! sees that it has no player in the round and watches it: the camera follows
//! survivors, demons and killers alike, skipping whoever is away in their pause
//! menu, or flies over the map on its own, and the chat is at hand instead of being
//! a page of the pause menu (client/screens/level).
//!
//! Between rounds they follow the round's screens as the players do: the results,
//! then the lobby. The first Lobby entry that lets the waiting room in ends the
//! spectate and they walk in as an ordinary player (lobby_init); a tournament that
//! keeps the waiting room out puts them into the next round instead (game_init calls
//! `enter_round` again).

use crate::colors::*;
use crate::config::cfg;
use crate::packet::{Packet, PacketType};
use crate::server::{GameState, OutboxMsg, Server};

/// Handles the `.spectate` command. Always answers the player in chat, so the
/// caller has nothing left to decide.
pub fn start(player_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if let Some(refusal) = refuse_reason(player_id, server) {
        crate::server::send_chat(outbox, player_id, &format!("{}{}", COLOR_RED, refusal));
        return;
    }

    if let Some(peer) = server.find_peer_mut(player_id) {
        peer.is_spectating = true;
    }
    // The waiting room loses them from its list the moment the round has them, so let
    // the list say where they went. MapVote has not drawn an exe yet, which is -1 here.
    let exe_id = match server.state {
        GameState::MapVote => -1,
        _ => server.game.exe,
    };
    crate::states::waiting_room::announce_waiter_spectating(player_id, exe_id, server, outbox);

    let nick = server.find_peer(player_id).map(|peer| peer.nickname.clone()).unwrap_or_default();
    log::info!("{} (id {}) is now spectating the round", colorize(&nick), player_id);

    // A round that is already running can be walked into at once; one that is still
    // being voted or picked for is joined by game_init when it starts.
    if server.game.round.is_some() {
        send_round(player_id, server, outbox);
        crate::server::send_chat(outbox, player_id, &format!("{}watching the round", COLOR_CYAN));
    } else {
        crate::server::send_chat(outbox, player_id, &format!("{}watching the round as soon as it starts", COLOR_CYAN));
    }
}

/// Why this player cannot start spectating right now, in the words the player
/// gets told, or None if they can.
fn refuse_reason(player_id: u16, server: &Server) -> Option<&'static str> {
    if !cfg().states.gameplay.allow_spectators {
        return Some("spectating is disabled on this server");
    }
    if !matches!(server.state, GameState::MapVote | GameState::Game) {
        return Some("there is no round to spectate yet");
    }

    let peer = match server.find_peer(player_id) {
        Some(peer) => peer,
        None => return Some("you are not connected to this round"),
    };
    if peer.in_game {
        return Some("you are playing this round already");
    }
    if peer.is_spectating {
        return Some("you are already spectating");
    }

    None
}

/// Called by game_init once the round exists: every spectator carried over from the
/// round before, or waiting for this one to start, is walked into it.
pub fn enter_round(server: &Server, outbox: &mut Vec<OutboxMsg>) {
    let watching: Vec<u16> = server.peers.iter().filter(|peer| peer.is_spectating).map(|peer| peer.id).collect();
    for player_id in watching {
        send_round(player_id, server, outbox);
    }
}

/// Whether this peer is watching the round rather than playing it, which is what
/// makes everything it sends about the round something to drop: its own client sends
/// none of it, and a changed one is answered here. What it may still do is talk (the
/// caller lets its chat through), because it never left the waiting room's chat.
pub fn plays_no_part(player_id: u16, server: &Server) -> bool {
    server.find_peer(player_id).is_some_and(|peer| peer.is_spectating)
}

/// The running round, for one spectator: who is in it, who is exe and where, the
/// numbers it is simulated with, and then the word that starts the watching. All of
/// it is what the round really is -- a spectator is told the truth about a round
/// they are not in, not walked through a join they never made.
fn send_round(player_id: u16, server: &Server, outbox: &mut Vec<OutboxMsg>) {
    let send = |outbox: &mut Vec<OutboxMsg>, pkt: Packet| {
        outbox.push(OutboxMsg::SendTo(player_id, pkt.data().to_vec(), true));
    };

    // Who is playing, by the same list the lobby hands anyone who asks for it: the
    // client draws the round's players by the names and icons in it.
    crate::states::lobby::send_lobby_player_list(player_id, server, outbox);

    let mut exe = Packet::new(PacketType::SERVER_LOBBY_EXE);
    let _ = exe.write_u16(server.game.exe as u16);
    let _ = exe.write_u16(server.game.map as u16);
    send(outbox, exe);

    // The characters, unless the round is played with them hidden: a round watched
    // is the round its players are seeing, not a better-informed version of it.
    if !cfg().states.gameplay.hide_player_characters {
        for (id, character) in round_characters(server) {
            let mut change = Packet::new(PacketType::SERVER_LOBBY_CHARACTER_CHANGE);
            let _ = change.write_u16(id);
            let _ = change.write_u8(character);
            send(outbox, change);
        }
    }

    // The gameplay numbers the round is simulated with, as its players were sent them.
    if let Some(setup) = server.game.round.as_ref().map(crate::states::round::config_packet) {
        outbox.push(OutboxMsg::SendTo(player_id, setup.to_vec(), true));
    }

    // "You are watching this round": the client leaves the waiting screen for the map.
    // It is its own packet rather than the round's start, so it cannot be mistaken for
    // "you are playing" and cannot arrive before the round it belongs to.
    send(outbox, Packet::new(PacketType::SERVER_SPECTATE_START));
}

/// Each player in the round and the character byte the client expects for them:
/// the exe character as-is for the exe, and the 1-based survivor character for
/// everyone else, exactly as char_select announces them.
fn round_characters(server: &Server) -> Vec<(u16, u8)> {
    server
        .peers
        .iter()
        .filter(|peer| peer.in_game)
        .map(|peer| {
            let character = if peer.id as i32 == server.game.exe { peer.exe_char as i8 as u8 } else { peer.surv_char as i8 as u8 + 1 };
            (peer.id, character)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::PeerData;

    /// What a spectator is sent is the round, addressed to them alone and ending with
    /// the word that starts the watching, and what they are not is a player of it.
    #[test]
    fn watching_sends_the_round_to_the_watcher_alone_and_joins_nothing() {
        crate::config::set_test_config();

        let mut server = Server::new(0);
        server.state = GameState::Game;
        server.game.exe = 1;
        let mut playing = PeerData::new(1, String::from("10.0.0.1"));
        playing.in_game = true;
        playing.nickname = String::from("player");
        server.peers.push(playing);
        let mut waiting = PeerData::new(2, String::from("10.0.0.2"));
        waiting.nickname = String::from("watcher");
        server.peers.push(waiting);

        let mut outbox = Vec::new();
        start(2, &mut server, &mut outbox);
        let watcher = server.find_peer(2).expect("the watcher is still connected");
        assert!(watcher.is_spectating && !watcher.in_game, "watching the round, not playing it");
        assert!(plays_no_part(2, &server), "nothing they send is a move in the round");
        assert!(watcher.follows_round(), "but they follow its screens");
        assert_eq!(server.ingame_count(), 1, "the roster is the one player it was");

        outbox.clear();
        send_round(2, &server, &mut outbox);
        let addressed: Vec<u16> = outbox
            .iter()
            .map(|message| match message {
                OutboxMsg::SendTo(id, ..) => *id,
                _ => panic!("the round is sent to the one watcher, not broadcast"),
            })
            .collect();
        assert!(addressed.iter().all(|&id| id == 2), "nobody in the round is told: {addressed:?}");

        let kinds: Vec<PacketType> = outbox
            .iter()
            .filter_map(|message| match message {
                OutboxMsg::SendTo(_, data, _) => PacketType::from_u8(*data.get(1)?),
                _ => None,
            })
            .collect();
        assert!(kinds.contains(&PacketType::SERVER_LOBBY_PLAYER), "who is in the round: {kinds:?}");
        assert!(kinds.contains(&PacketType::SERVER_LOBBY_EXE), "who is exe and where: {kinds:?}");
        assert_eq!(kinds.last(), Some(&PacketType::SERVER_SPECTATE_START), "the watching starts last: {kinds:?}");
    }
}
