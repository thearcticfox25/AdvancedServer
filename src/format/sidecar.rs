//! Optional metadata files that sit next to a sprite (sounds have one list, see sounds.rs):
//! `tails_idle.png` + `tails_idle.toml`.
//!
//! A sidecar only exists when something differs from the defaults below, so
//! most media files stand alone and can be dragged around by themselves.
//! A file per resource (instead of one big manifest) means a texture pack can
//! override the origin of one sprite without copying anyone else's entries.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// How collisions are checked against a sprite, as in the GameMaker sprite editor.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum CollisionKind {
    /// Pixel mask combined from all frames.
    Precise,
    #[default]
    Rectangle,
    Ellipse,
    Diamond,
    /// Separate pixel mask for every frame.
    PrecisePerFrame,
    RotatedRectangle,
}

/// Where the bounding box comes from, as in the GameMaker sprite editor.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum BboxMode {
    /// Computed from non-transparent pixels, so it follows texture pack edits.
    #[default]
    Automatic,
    FullImage,
    /// Uses `bbox` from the sidecar.
    Manual,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct SpriteSidecar {
    #[serde(skip_serializing_if = "is_zero_pair")]
    pub origin: [i32; 2],
    #[serde(skip_serializing_if = "is_default")]
    pub collision: CollisionKind,
    #[serde(skip_serializing_if = "is_default")]
    pub bbox_mode: BboxMode,
    /// left, top, right, bottom (inclusive). Only read when bbox_mode = "manual".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bbox: Option<[i32; 4]>,
    /// Alpha at or below this value counts as empty space for automatic bbox and precise masks.
    #[serde(skip_serializing_if = "is_default")]
    pub alpha_tolerance: u8,
    /// Overrides the playback speed stored in the APNG. Needed for animated
    /// sprites with speed 0, whose frame is chosen by code through image_index.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fps: Option<u16>,
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

fn is_zero_pair(value: &[i32; 2]) -> bool {
    *value == [0, 0]
}

/// Sidecar path for a media file: same folder, same name, `.toml` extension.
pub fn path_for(media_path: &Path) -> std::path::PathBuf {
    media_path.with_extension("toml")
}

/// Reads the sidecar next to a media file, or the defaults when there is none.
pub fn load<T: for<'de> Deserialize<'de> + Default>(media_path: &Path) -> Result<T> {
    let path = path_for(media_path);
    if !crate::core::resources::exists(&path) {
        return Ok(T::default());
    }
    let text = crate::core::resources::read_to_string(&path)?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Writes the sidecar, or removes a stale one when every value is default.
pub fn save<T: Serialize + Default + PartialEq>(media_path: &Path, sidecar: &T) -> Result<()> {
    let path = path_for(media_path);
    if *sidecar == T::default() {
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        return Ok(());
    }
    std::fs::write(&path, toml::to_string(sidecar)?)?;
    Ok(())
}
