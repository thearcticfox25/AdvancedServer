//! Sprite data the simulation needs: size, origin, bounding box, collision masks.
//! Pixels are decoded on first use, because all frames together take 1.8 GB.

use super::names::SPRITE_FILES;
use super::{find_files, SpriteId};
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use crate::format::apng;
use crate::format::sidecar::{self, BboxMode, CollisionKind, SpriteSidecar};

pub struct SpriteMeta {
    pub width: u32,
    pub height: u32,
    pub origin_x: f64,
    pub origin_y: f64,
    pub frame_count: u32,
    pub fps: u16,
    /// left, top, right, bottom in sprite pixels, inclusive.
    pub bbox: [i32; 4],
    /// None = rectangle collision. One mask for "precise", one per frame for "precise per frame".
    pub masks: Option<Vec<Vec<bool>>>,
}

pub struct Sprites {
    paths: Vec<Option<PathBuf>>,
    loaded: Vec<OnceLock<SpriteMeta>>,
}

impl Sprites {
    /// Finds every sprite file under `textures`, by file name. Nothing is decoded yet.
    pub fn index(textures: &Path) -> Result<Sprites> {
        // Still pictures are png, animated ones apng; both are read the same way.
        let mut by_name = find_files(textures, &["png", "apng"])?;
        Ok(Sprites {
            paths: SPRITE_FILES.iter().map(|name| by_name.remove(*name)).collect(),
            loaded: SPRITE_FILES.iter().map(|_| OnceLock::new()).collect(),
        })
    }

    /// The sprite's file, for code that needs its pixels (the client's textures).
    pub fn path(&self, sprite: SpriteId) -> Option<&Path> {
        self.paths[sprite.0].as_deref()
    }

    pub fn get(&self, sprite: SpriteId) -> &SpriteMeta {
        self.loaded[sprite.0].get_or_init(|| {
            let name = SPRITE_FILES[sprite.0];
            // ponytail: a missing or broken file is fatal, a placeholder sprite if packs need tolerance
            let path = self.paths[sprite.0].as_ref().unwrap_or_else(|| panic!("sprite file {name}.png (or .apng) not found in Textures"));
            load(path).unwrap_or_else(|error| panic!("sprite {name}: {error:#}"))
        })
    }
}

fn load(path: &Path) -> Result<SpriteMeta> {
    let image = apng::read(path)?;
    let settings: SpriteSidecar = sidecar::load(path)?;
    let solid = |pixel: &[u8]| pixel[3] > settings.alpha_tolerance;

    let bbox = match settings.bbox_mode {
        BboxMode::Manual => settings.bbox.unwrap_or([0, 0, image.width as i32 - 1, image.height as i32 - 1]),
        BboxMode::FullImage => [0, 0, image.width as i32 - 1, image.height as i32 - 1],
        BboxMode::Automatic => automatic_bbox(&image, &solid),
    };

    let frame_mask = |frame: &Vec<u8>| frame.chunks_exact(4).map(solid).collect::<Vec<bool>>();
    let masks = match settings.collision {
        CollisionKind::Precise => {
            let mut combined = vec![false; (image.width * image.height) as usize];
            for frame in &image.frames {
                for (cell, set) in combined.iter_mut().zip(frame_mask(frame)) {
                    *cell |= set;
                }
            }
            Some(vec![combined])
        }
        CollisionKind::PrecisePerFrame => Some(image.frames.iter().map(frame_mask).collect()),
        // ponytail: ellipse/diamond/rotated rectangle checked as rectangles (1 sprite uses ellipse), exact shapes if a map needs them
        _ => None,
    };

    Ok(SpriteMeta {
        width: image.width,
        height: image.height,
        origin_x: settings.origin[0] as f64,
        origin_y: settings.origin[1] as f64,
        frame_count: image.frames.len() as u32,
        fps: settings.fps.unwrap_or(image.fps),
        bbox,
        masks,
    })
}

/// Union of solid pixels over all frames. A fully transparent sprite gets a
/// 1x1 box at 0,0 so that bbox arithmetic stays valid.
fn automatic_bbox(image: &apng::SpriteImage, solid: &dyn Fn(&[u8]) -> bool) -> [i32; 4] {
    let mut bbox: Option<[i32; 4]> = None;
    for frame in &image.frames {
        for (index, pixel) in frame.chunks_exact(4).enumerate() {
            if !solid(pixel) {
                continue;
            }
            let x = (index as u32 % image.width) as i32;
            let y = (index as u32 / image.width) as i32;
            bbox = Some(match bbox {
                None => [x, y, x, y],
                Some([l, t, r, b]) => [l.min(x), t.min(y), r.max(x), b.max(y)],
            });
        }
    }
    bbox.unwrap_or([0, 0, 0, 0])
}
