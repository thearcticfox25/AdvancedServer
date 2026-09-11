pub mod char_select;
pub mod game;
pub mod lobby;
pub mod map_vote;
pub mod results;
pub mod spectate;
pub mod waiting_room;

use crate::packet::Packet;
use crate::server::{GameState, OutboxMsg, Server};

pub const NO_COUNTDOWN: u8 = 6;

// The server is a state machine, and each of its states answers the same four
// questions: a player joined, a player left, a player sent a packet, a tick
// passed. Routing them lives here, in one place, so adding a state means
// touching this file instead of hunting the same `match` down in the worker
// loop and in the connection code.

/// A player finished connecting while the server was in `state`.
pub fn state_join(player_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    match server.state {
        GameState::Lobby      => lobby::lobby_state_join(player_id, server, outbox),
        GameState::MapVote    => map_vote::mapvote_state_join(player_id, server, outbox),
        GameState::CharSelect => char_select::charselect_state_join(player_id, server, outbox),
        GameState::Game       => game::game_state_join(player_id, server, outbox),
        GameState::Results    => {}
    }
}

/// A player disconnected while the server was in `state`.
pub fn state_left(player_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    match server.state {
        GameState::Lobby      => lobby::lobby_state_left(player_id, server, outbox),
        GameState::MapVote    => map_vote::mapvote_state_left(player_id, server, outbox),
        GameState::CharSelect => char_select::charselect_state_left(player_id, server, outbox),
        GameState::Game       => game::game_state_left(player_id, server, outbox),
        GameState::Results    => results::results_state_left(player_id, server, outbox),
    }
}

/// A packet arrived from an identified player.
pub fn state_handle(
    player_id: u16,
    packet: &mut Packet,
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
) {
    match server.state {
        GameState::Lobby      => lobby::lobby_state_handle(player_id, packet, server, outbox),
        GameState::MapVote    => map_vote::mapvote_state_handle(player_id, packet, server, outbox),
        GameState::CharSelect => char_select::charselect_state_handle(player_id, packet, server, outbox),
        GameState::Game       => game::game_state_handle(player_id, packet, server, outbox),
        GameState::Results    => results::results_state_handle(player_id, packet, server, outbox),
    }
}

/// One 60 Hz tick of whichever state the server is in.
pub fn state_tick(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    match server.state {
        GameState::Lobby      => lobby::lobby_state_tick(server, outbox),
        GameState::MapVote    => map_vote::mapvote_state_tick(server, outbox),
        GameState::CharSelect => char_select::charselect_state_tick(server, outbox),
        GameState::Game       => game::game_state_tick(server, outbox),
        GameState::Results    => results::results_state_tick(server, outbox),
    }
}
