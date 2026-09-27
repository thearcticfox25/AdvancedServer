//! Deterministic simulation shared by the server (authority) and the client
//! (prediction of the local player). No graphics, audio or networking here.

pub mod resources;
pub mod black_ring;
pub mod collision;
pub mod config;
pub mod contact;
pub mod egg_tracker;
pub mod events;
pub mod exeller_clone;
pub mod game;
pub mod heal;
pub mod level;
pub mod objects;
pub mod palettes;
pub mod player;
pub mod rings;
pub mod rooms;
pub mod shield_shards;
pub mod snapshot;
pub mod stomp_wave;
pub mod tails_projectile;
pub mod world;

/// The window every client draws a round in (obj_camera's view_wport and view_hport).
/// The server knows it too: it decides what a player is near enough to be told about
/// (states/round.rs) and the client draws in it (client/canvas.rs).
pub const VIEW_WIDTH: f64 = 480.0;
pub const VIEW_HEIGHT: f64 = 270.0;

#[cfg(test)]
mod tests;
