//! `.spectate` -- watching a round from the waiting room.
//!
//! The game client has no spectator mode and no packet that means "you are
//! watching". The only screen on which it draws a live round is the one it
//! reaches by joining that round, and the only way it ever puts its camera on
//! somebody else is its own death: a survivor who dies and is not demonised
//! watches the survivors who are still up. So that is the route taken here.
//! The waiting-room player's client is walked through the join it missed --
//! Lobby, who is exe and on which map, everyone's characters, game start -- and
//! is then killed off the moment it reaches the map. From there the client's own
//! spectator camera does the watching, exactly as it would for anyone who died
//! in the round for real. The walk starts from whichever stage the round is
//! already at: MapVote and CharSelect only need part of it, because the round's
//! own broadcasts carry the client the rest of the way with everyone else.
//!
//! Once started it lasts as long as the player wants it to. There is no packet
//! that puts a client back on the waiting screen, so a spectator is never sent
//! back there: they follow the round out to Results, through Lobby and the next
//! MapVote, and into the round after that, for as long as the waiting room is
//! kept out of Lobby (see results::tournament_continues). The first Lobby entry
//! that does let the waiting room in ends the spectate and they walk in as an
//! ordinary player.
//!
//! None of that reaches the round. The peer keeps `in_game = false` the whole
//! time, which is what every other stage already uses to decide who is playing:
//! they stay off the roster, out of the exe draw, out of the win conditions and
//! out of the anti-cheat. Every packet here is addressed to the one peer that
//! asked for it, and everything that peer sends back is dropped by
//! `handle_packet` below rather than being mistaken for a move in the round.
//!
//! What it cannot do, all of it for the same reason -- the client only learns
//! about things while it is listening, and this one was not:
//!
//! - Map objects spawned before the request (rings, hazards) are invisible to
//!   the spectator. Only ones spawned from here on show up.
//! - There is no chat on the map screen, so the spectator is out of the
//!   waiting-room chat until the round ends and everyone returns to Lobby.
//! - The walk takes about five seconds, most of it the client's own
//!   lobby-to-map fade, which the server cannot shorten.

use crate::colors::*;
use crate::config::cfg;
use crate::packet::{Packet, PacketType};
use crate::server::{GameState, OutboxMsg, Server, SurvChar};

/// How far along a peer is in the walk described above.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Spectate {
    /// Ordinary peer: either playing the round or waiting one out.
    No,
    /// Has been sent the join sequence and is loading the map.
    Joining,
    /// Dead on its own screen, watching the round.
    Watching,
}

/// The body the spectator's client gives them for the moment between reaching
/// the map and being killed. Nobody else is ever told about it. Tails is the
/// plain pick: the client's damage path has no shield or revival special case
/// for it, so the fatal hit below is guaranteed to land.
const SPECTATOR_CHARACTER: SurvChar = SurvChar::Tails;

/// Enough to kill any survivor outright: the client starts them at 100 hp and
/// caps a single hit at one byte.
const FATAL_DAMAGE: u8 = 255;

/// Handles the `.spectate` command. Always answers the player in chat, so the
/// caller has nothing left to decide.
pub fn start(player_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if let Some(refusal) = refuse_reason(player_id, server) {
        crate::server::send_chat(outbox, player_id, &format!("{}{}", COLOR_RED, refusal));
        return;
    }

    if let Some(peer) = server.find_peer_mut(player_id) {
        peer.spectate = Spectate::Joining;
    }
    send_join_sequence(player_id, server, outbox);
    // The waiting room loses them from its chat the moment their client leaves
    // for the map, so let the list they are on say where they went. MapVote has
    // not drawn an exe yet, which is what -1 means here.
    let exe_id = match server.state {
        GameState::MapVote    => -1,
        GameState::CharSelect => server.lobby.exe as i32,
        _                     => server.game.exe,
    };
    crate::states::waiting_room::announce_waiter_spectating(
        player_id, exe_id, server, outbox,
    );

    let nick = server.find_peer(player_id).map(|peer| peer.nickname.clone()).unwrap_or_default();
    log::info!("{} (id {}) is now spectating the round", colorize(&nick), player_id);
    crate::server::send_chat(outbox, player_id, &format!("{}entering the round as a spectator", COLOR_CYAN));
}

/// Why this player cannot start spectating right now, in the words the player
/// gets told, or None if they can.
fn refuse_reason(player_id: u16, server: &Server) -> Option<&'static str> {
    if !cfg().states.gameplay.allow_spectators {
        return Some("spectating is disabled on this server");
    }
    if !matches!(server.state, GameState::MapVote | GameState::CharSelect | GameState::Game) {
        return Some("there is no round to spectate yet");
    }

    let peer = match server.find_peer(player_id) {
        Some(peer) => peer,
        None       => return Some("you are not connected to this round"),
    };
    if peer.in_game {
        return Some("you are playing this round already");
    }
    if peer.spectate != Spectate::No {
        return Some("you are already spectating");
    }

    None
}

/// One packet from a peer that is spectating. Returns false for everyone else,
/// so the caller carries on with the round as usual.
///
/// A spectator is standing in the map but is not in the round, so nothing they
/// send is a move in it and all of it is dropped here. The one thing worth
/// reading is *that* they sent a position at all: the client only reports one
/// once it has spawned in the map, which is the first moment the fatal hit can
/// land.
pub fn handle_packet(
    player_id: u16,
    packet: &Packet,
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
) -> bool {
    match server.find_peer(player_id).map(|peer| peer.spectate) {
        None | Some(Spectate::No) => return false,
        Some(Spectate::Watching)  => return true,
        Some(Spectate::Joining)   => {}
    }

    if packet.packet_type() == Some(PacketType::CLIENT_PLAYER_DATA) {
        kill(player_id, outbox);
        if let Some(peer) = server.find_peer_mut(player_id) {
            peer.spectate = Spectate::Watching;
        }
    }
    true
}

/// Puts the spectator's client where a survivor ends up once they have been
/// killed and nobody came to revive them in time. That is two steps in the real
/// game and it is two steps here, because the client tracks them separately: the
/// fatal hit is what drops it into its spectator camera, and the death timer
/// running out without a demonisation is what marks the body as finally dead.
///
/// Both are needed. Stopping after the hit leaves the client showing a body that
/// is merely down -- the survivor icon other players are meant to run over to,
/// only with no countdown on it, since no countdown is ever coming for someone
/// who is not in the round.
fn kill(player_id: u16, outbox: &mut Vec<OutboxMsg>) {
    let mut hit = Packet::new(PacketType::SERVER_FORCE_DAMAGE);
    let _ = hit.write_u8(FATAL_DAMAGE);
    let _ = hit.write_i8(0);
    let _ = hit.write_i8(0);
    outbox.push(OutboxMsg::SendTo(player_id, hit.data().to_vec(), true));

    let mut timer_over = Packet::new(PacketType::SERVER_GAME_DEATHTIMER_END);
    let _ = timer_over.write_u8(0); // not demonised: dead for good
    outbox.push(OutboxMsg::SendTo(player_id, timer_over.data().to_vec(), true));
}

/// Called from charselect_init, for a spectator carried over from the round
/// before: gives them a body for the round about to start and puts them back at
/// the start of the walk, so the fatal hit lands again once they reach the map.
///
/// Everything else the round broadcasts -- who is exe, on which map, everyone's
/// characters, the start itself -- already reaches them, so this is all that is
/// left to do.
pub fn rejoin(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let carried_over: Vec<u16> = server.peers.iter()
        .filter(|peer| peer.spectate != Spectate::No)
        .map(|peer| peer.id)
        .collect();

    for player_id in carried_over {
        if let Some(peer) = server.find_peer_mut(player_id) {
            peer.spectate = Spectate::Joining;
        }
        send_own_character(player_id, outbox);
    }
}

/// The spectator's own character. They need a body for the client to build its
/// camera on, and something for the fatal hit to take away.
fn send_own_character(player_id: u16, outbox: &mut Vec<OutboxMsg>) {
    let mut own_char = Packet::new(PacketType::SERVER_LOBBY_CHARACTER_RESPONSE);
    let _ = own_char.write_u8(SPECTATOR_CHARACTER as i8 as u8 + 1);
    let _ = own_char.write_u8(1);
    outbox.push(OutboxMsg::SendTo(player_id, own_char.data().to_vec(), true));
}

/// The join the spectator's client missed, replayed for that client alone, in
/// the order the client would have lived through it.
fn send_join_sequence(player_id: u16, server: &Server, outbox: &mut Vec<OutboxMsg>) {
    let send = |outbox: &mut Vec<OutboxMsg>, pkt: Packet| {
        outbox.push(OutboxMsg::SendTo(player_id, pkt.data().to_vec(), true));
    };

    // "You are in the Lobby" -- the same packet connection::accept sends a
    // player who arrives while the server is in Lobby. The client enters the
    // lobby room and stops reading packets until the next frame, which is what
    // makes the rest of this sequence arrive with that room already built.
    let mut in_lobby = Packet::new(PacketType::SERVER_IDENTITY_RESPONSE);
    let _ = in_lobby.write_u8(1);
    let _ = in_lobby.write_u16(player_id);
    send(outbox, in_lobby);

    // Who is in the round. Same roster the Lobby hands anyone who asks for it.
    crate::states::lobby::send_lobby_player_list(player_id, server, outbox);

    // MapVote is as far as the walk goes while the vote is still running: the
    // client only acts on the map list from the Lobby it has just entered, and
    // charselect_init's own broadcast carries it on from there with everyone
    // else.
    if server.state == GameState::MapVote {
        let mut maps = Packet::new(PacketType::SERVER_VOTE_MAPS);
        let _ = maps.write_u8(server.lobby.maps[0]);
        let _ = maps.write_u8(server.lobby.maps[1]);
        let _ = maps.write_u8(server.lobby.maps[2]);
        send(outbox, maps);
        return;
    }

    // Who is exe and where -- this is what moves the client on to character
    // select, and it needs the roster above to already be there. In CharSelect
    // that is the whole walk: the round's own SERVER_LOBBY_GAME_START takes it
    // the rest of the way with everyone else.
    let (exe_id, map) = if server.state == GameState::CharSelect {
        (server.lobby.exe, server.lobby.map)
    } else {
        (server.game.exe as u16, server.game.map)
    };
    let mut exe = Packet::new(PacketType::SERVER_LOBBY_EXE);
    let _ = exe.write_u16(exe_id);
    let _ = exe.write_u16(map as u16);
    send(outbox, exe);

    send_own_character(player_id, outbox);

    if server.state == GameState::CharSelect {
        return;
    }

    // Everyone else's characters. The client draws each player with the
    // character it was told about here, so a round played with characters
    // hidden is watched with them hidden too -- the spectator sees the round
    // its players are seeing, not a better-informed version of it.
    if !cfg().states.gameplay.hide_player_characters {
        for (id, character) in round_characters(server) {
            let mut change = Packet::new(PacketType::SERVER_LOBBY_CHARACTER_CHANGE);
            let _ = change.write_u16(id);
            let _ = change.write_u8(character);
            send(outbox, change);
        }
    }

    // Off to the map. The client fades out of the lobby, loads it, and puts a
    // puppet there for every player on the roster above.
    send(outbox, Packet::new(PacketType::SERVER_LOBBY_GAME_START));
}

/// Each player in the round and the character byte the client expects for them:
/// the exe character as-is for the exe, and the 1-based survivor character for
/// everyone else, exactly as char_select announces them.
fn round_characters(server: &Server) -> Vec<(u16, u8)> {
    server.peers.iter()
        .filter(|peer| peer.in_game)
        .map(|peer| {
            let character = if peer.id as i32 == server.game.exe {
                peer.exe_char as i8 as u8
            } else {
                peer.surv_char as i8 as u8 + 1
            };
            (peer.id, character)
        })
        .collect()
}
