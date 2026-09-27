//! File formats shared by the game and the GameMaker converter.
//!
//! Both sides must agree byte-for-byte on how a sprite, a sidecar or a map
//! looks on disk, so this file is kept the same in both projects. The halves
//! only the game uses (reading maps and placements back in) are kept here too,
//! so the two copies stay one file.
#![allow(dead_code)]

pub mod apng;
pub mod placement;
pub mod sidecar;
pub mod sounds;
pub mod tiled;
