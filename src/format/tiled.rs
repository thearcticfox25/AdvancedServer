//! The subset of the Tiled JSON format (.tmj maps, .tsj tilesets) the game uses.
//!
//! Tiled (https://www.mapeditor.org) is the level editor: object layers play
//! the role of GameMaker instance layers, image layers are background layers,
//! and every object type is a tile of an image-collection tileset, so placing
//! an instance is dragging a picture onto the map, like in GameMaker.
//!
//! GameMaker specifics that Tiled has no field for (layer depth, background
//! scroll speed, instance variables...) are stored as custom properties.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub const FLIPPED_HORIZONTALLY: u32 = 0x8000_0000;
pub const FLIPPED_VERTICALLY: u32 = 0x4000_0000;
const FLIP_FLAGS: u32 = 0xF000_0000;

/// Tiled format version written by the converter. Image layer repeat needs 1.8+,
/// the "type" key for classes is what 1.10+ reads and writes.
pub const FORMAT_VERSION: &str = "1.10";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Property {
    pub name: String,
    /// "string", "int", "float", "bool" or "color".
    #[serde(rename = "type")]
    pub kind: String,
    pub value: Value,
}

impl Property {
    pub fn string(name: &str, value: &str) -> Property {
        Property { name: name.into(), kind: "string".into(), value: Value::from(value) }
    }

    pub fn int(name: &str, value: i64) -> Property {
        Property { name: name.into(), kind: "int".into(), value: Value::from(value) }
    }

    pub fn float(name: &str, value: f64) -> Property {
        Property { name: name.into(), kind: "float".into(), value: Value::from(value) }
    }

    pub fn bool(name: &str, value: bool) -> Property {
        Property { name: name.into(), kind: "bool".into(), value: Value::from(value) }
    }
}

pub fn find_property<'a>(properties: &'a [Property], name: &str) -> Option<&'a Value> {
    properties.iter().find(|property| property.name == name).map(|property| &property.value)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Map {
    #[serde(rename = "type")]
    pub kind: String,
    pub version: String,
    pub orientation: String,
    pub renderorder: String,
    pub infinite: bool,
    /// Size in tiles. Tiles are only used as the editor snapping grid.
    pub width: u32,
    pub height: u32,
    pub tilewidth: u32,
    pub tileheight: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backgroundcolor: Option<String>,
    pub nextlayerid: u32,
    pub nextobjectid: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<Property>,
    pub tilesets: Vec<TilesetRef>,
    pub layers: Vec<Layer>,
}

impl Map {
    pub fn pixel_width(&self) -> u32 {
        self.width * self.tilewidth
    }

    pub fn pixel_height(&self) -> u32 {
        self.height * self.tileheight
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TilesetRef {
    pub firstgid: u32,
    /// Path of the .tsj file, relative to the map file.
    pub source: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Layer {
    ImageLayer(ImageLayer),
    ObjectGroup(ObjectGroup),
    /// A folder of layers, like a GameMaker folder layer. Hiding it hides everything inside.
    Group(GroupLayer),
}

impl Layer {
    pub fn name(&self) -> &str {
        match self {
            Layer::ImageLayer(layer) => &layer.name,
            Layer::ObjectGroup(layer) => &layer.name,
            Layer::Group(layer) => &layer.name,
        }
    }

    pub fn properties(&self) -> &[Property] {
        match self {
            Layer::ImageLayer(layer) => &layer.properties,
            Layer::ObjectGroup(layer) => &layer.properties,
            Layer::Group(layer) => &layer.properties,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GroupLayer {
    pub id: u32,
    pub name: String,
    #[serde(default = "one")]
    pub opacity: f64,
    #[serde(default = "yes")]
    pub visible: bool,
    pub x: i32,
    pub y: i32,
    pub layers: Vec<Layer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<Property>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImageLayer {
    pub id: u32,
    pub name: String,
    /// Image path relative to the map file. Empty for a solid colour layer.
    #[serde(default)]
    pub image: String,
    #[serde(default)]
    pub imagewidth: u32,
    #[serde(default)]
    pub imageheight: u32,
    #[serde(default)]
    pub repeatx: bool,
    #[serde(default)]
    pub repeaty: bool,
    #[serde(default)]
    pub offsetx: f64,
    #[serde(default)]
    pub offsety: f64,
    /// How much the layer moves with the camera: 1 like the map, 0 fixed on screen.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub parallaxx: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub parallaxy: f64,
    #[serde(default = "one")]
    pub opacity: f64,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tintcolor: Option<String>,
    pub x: i32,
    pub y: i32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<Property>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ObjectGroup {
    pub id: u32,
    pub name: String,
    pub draworder: String,
    #[serde(default = "one")]
    pub opacity: f64,
    #[serde(default = "yes")]
    pub visible: bool,
    pub x: i32,
    pub y: i32,
    pub objects: Vec<Object>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<Property>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Object {
    pub id: u32,
    #[serde(default)]
    pub name: String,
    /// Object class. For tile objects Tiled leaves it empty and the class comes from the tile.
    #[serde(rename = "type", default)]
    pub class: String,
    /// Tile reference with flip flags in the high bits. None for plain rectangles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gid: Option<u32>,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub height: f64,
    /// Degrees, clockwise, around the object's alignment point (top-left here).
    #[serde(default)]
    pub rotation: f64,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<Property>,
}

impl Object {
    pub fn tile_gid(&self) -> Option<u32> {
        self.gid.map(|gid| gid & !FLIP_FLAGS)
    }

    pub fn flipped_horizontally(&self) -> bool {
        self.gid.is_some_and(|gid| gid & FLIPPED_HORIZONTALLY != 0)
    }

    pub fn flipped_vertically(&self) -> bool {
        self.gid.is_some_and(|gid| gid & FLIPPED_VERTICALLY != 0)
    }
}

/// Image-collection tileset: every tile is a separate image file.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Tileset {
    #[serde(rename = "type")]
    pub kind: String,
    pub version: String,
    pub name: String,
    pub tilewidth: u32,
    pub tileheight: u32,
    pub tilecount: u32,
    pub columns: u32,
    pub margin: u32,
    pub spacing: u32,
    /// "topleft" makes a tile object's x/y its top-left corner, like an unrotated sprite at origin 0,0.
    pub objectalignment: String,
    pub tiles: Vec<Tile>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Tile {
    pub id: u32,
    /// Image path relative to the tileset file.
    pub image: String,
    pub imagewidth: u32,
    pub imageheight: u32,
    #[serde(rename = "type", default, skip_serializing_if = "String::is_empty")]
    pub class: String,
}

fn one() -> f64 {
    1.0
}

fn yes() -> bool {
    true
}

fn is_one(value: &f64) -> bool {
    *value == 1.0
}

pub fn load_map(path: &Path) -> Result<Map> {
    let text = crate::core::resources::read_to_string(path)?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

pub fn load_tileset(path: &Path) -> Result<Tileset> {
    let text = crate::core::resources::read_to_string(path)?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

pub fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    std::fs::write(path, serde_json::to_string_pretty(value)?)
        .with_context(|| format!("writing {}", path.display()))
}

/// Resolves a path written relative to a map or tileset file.
pub fn relative_to(file: &Path, relative: &str) -> PathBuf {
    file.parent().unwrap_or(Path::new(".")).join(relative)
}
