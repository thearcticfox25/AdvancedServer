//! A room as its Tiled map describes it: layers in drawing order with their
//! backgrounds, decoration sprites and placed instances. The room draws its
//! backgrounds and decorations; screens turn placed instances into their own
//! objects and draw those.

use crate::client::canvas::{Canvas, HeatHaze, VIEW_HEIGHT, VIEW_WIDTH};
use anyhow::{Context as _, Result};
use std::path::Path;
use crate::format::tiled::{self, Property};
use crate::core::resources::names::SPRITE_FILES;
use crate::core::resources::sprites::Sprites;
use crate::core::resources::SpriteId;
use crate::core::objects::ids::ObjectId;
use crate::core::rooms::ids::RoomId;
use crate::core::world::{map_object_class, map_object_placement, TileLookup, C_WHITE};
use crate::core::config::step;

pub struct Room {
    pub width: f64,
    pub height: f64,
    /// From the back (highest depth) to the front, as GameMaker draws them.
    pub layers: Vec<Layer>,
}

pub struct Layer {
    pub name: String,
    pub depth: i32,
    pub visible: bool,
    pub content: LayerContent,
    /// GameMaker's layer filter, if the map put one on this layer: everything drawn
    /// down to here is pushed around by the heat haze once the layer is drawn.
    pub heat_haze: Option<HeatHaze>,
    /// _filter_tintfilter (g_TintCol): what the layer draws is multiplied by this colour.
    pub tint: u32,
    /// _filter_greyscale at g_Intensity 1: what the layer draws is grey. Intensity 0
    /// (off) and 1 are the only values the game sets.
    pub greyscale: bool,
}

pub enum LayerContent {
    Background(Background),
    /// The sprites of an asset layer.
    Decorations(Vec<Decoration>),
    Instances(Vec<PlacedInstance>),
}

pub struct Background {
    pub sprite: Option<SpriteId>,
    pub image_index: f64,
    /// Tint of the sprite, or the fill colour when there is no sprite.
    pub colour: u32,
    pub alpha: f64,
    pub tile_horizontally: bool,
    pub tile_vertically: bool,
    pub stretch: bool,
    pub hspeed: f64,
    pub vspeed: f64,
    /// Tiled parallax factors: 1 moves with the room, 0 stays on screen (global.parallax).
    pub parallax_x: f64,
    pub parallax_y: f64,
    /// Frames per second when the layer overrides the sprite's own speed.
    pub animation_fps: Option<f64>,
    pub x: f64,
    pub y: f64,
}

pub struct Decoration {
    pub sprite: SpriteId,
    pub x: f64,
    pub y: f64,
    pub xscale: f64,
    pub yscale: f64,
    pub angle: f64,
    pub image_index: f64,
    /// Multiplier of the sprite's own animation speed.
    pub animation_speed: f64,
    pub blend: u32,
    pub alpha: f64,
}

/// An instance as the room editor placed it, before its object takes it over.
pub struct PlacedInstance {
    /// Creation order: GameMaker creates room instances in this order.
    pub id: u32,
    pub object: ObjectId,
    pub x: f64,
    pub y: f64,
    pub xscale: f64,
    pub yscale: f64,
    pub angle: f64,
    /// Instance variables and creation code assignments.
    pub variables: Vec<Property>,
}

impl PlacedInstance {
    pub fn number(&self, name: &str) -> Option<f64> {
        tiled::find_property(&self.variables, name).and_then(|value| value.as_f64())
    }

    pub fn string(&self, name: &str) -> Option<&str> {
        tiled::find_property(&self.variables, name).and_then(|value| value.as_str())
    }

    pub fn boolean(&self, name: &str) -> Option<bool> {
        tiled::find_property(&self.variables, name).and_then(|value| value.as_bool())
    }

    /// A colour variable as GameMaker colour and alpha.
    pub fn colour(&self, name: &str) -> Option<(u32, f64)> {
        self.string(name).map(parse_tiled_colour)
    }
}

impl Room {
    pub fn load(maps_folder: &Path, room: RoomId, sprites: &Sprites) -> Result<Room> {
        let map_path = maps_folder.join(format!("{}.tmj", room.file_name()));
        let map = tiled::load_map(&map_path)?;
        let tiles = TileLookup::load(&map_path, &map)?;
        let mut layers = Vec::new();
        collect_layers(&map.layers, &tiles, sprites, &mut layers).with_context(|| format!("room {}", map_path.display()))?;
        // Stable, so layers with the same depth keep their order in the file.
        layers.sort_by_key(|layer| std::cmp::Reverse(layer.depth));
        Ok(Room { width: map.pixel_width() as f64, height: map.pixel_height() as f64, layers })
    }

    /// Every placed instance of `object` with the index of its layer, layer by layer.
    pub fn placed(&self, object: ObjectId) -> impl Iterator<Item = (usize, &PlacedInstance)> {
        self.layers.iter().enumerate().flat_map(move |(index, layer)| {
            let instances: &[PlacedInstance] = match &layer.content {
                LayerContent::Instances(instances) => instances,
                _ => &[],
            };
            instances.iter().filter(move |instance| instance.object == object).map(move |instance| (index, instance))
        })
    }

    /// Every placed instance with the index of its layer, in creation order.
    pub fn placed_in_creation_order(&self) -> Vec<(usize, &PlacedInstance)> {
        let mut all: Vec<(usize, &PlacedInstance)> = self
            .layers
            .iter()
            .enumerate()
            .flat_map(|(index, layer)| match &layer.content {
                LayerContent::Instances(instances) => instances.iter().map(|instance| (index, instance)).collect(),
                _ => Vec::new(),
            })
            .collect();
        all.sort_by_key(|(_, instance)| instance.id);
        all
    }

    pub fn background_mut(&mut self, layer_name: &str) -> Option<&mut Background> {
        match &mut self.layer_mut(layer_name)?.content {
            LayerContent::Background(background) => Some(background),
            _ => None,
        }
    }

    pub fn layer_mut(&mut self, name: &str) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|layer| layer.name == name)
    }

    /// layer_set_visible
    pub fn set_layer_visible(&mut self, name: &str, visible: bool) {
        if let Some(layer) = self.layer_mut(name) {
            layer.visible = visible;
        }
    }

    /// The layer speeds and animations of one step.
    pub fn step(&mut self, sprites: &Sprites) {
        for layer in &mut self.layers {
            match &mut layer.content {
                LayerContent::Background(background) => {
                    background.x += background.hspeed * step();
                    background.y += background.vspeed * step();
                    if let Some(sprite) = background.sprite {
                        let fps = background.animation_fps.unwrap_or(sprites.get(sprite).fps as f64);
                        background.image_index += fps / crate::core::config::ticks_per_second();
                    }
                }
                LayerContent::Decorations(decorations) => {
                    for decoration in decorations {
                        let fps = sprites.get(decoration.sprite).fps as f64;
                        decoration.image_index += fps / crate::core::config::ticks_per_second() * decoration.animation_speed;
                    }
                }
                LayerContent::Instances(_) => {}
            }
        }
    }

    /// Every visible layer from the back, for rooms whose instances draw nothing of their own.
    /// Every visible layer from the back; `draw_instances(canvas, layer index)` draws
    /// the screen's own instances of each visible layer after the layer itself.
    pub fn draw_layers_with(&self, canvas: &mut Canvas, mut draw_instances: impl FnMut(&mut Canvas, usize)) {
        for (index, layer) in self.layers.iter().enumerate() {
            if layer.visible {
                self.draw_layer(canvas, layer);
                draw_instances(canvas, index);
            }
        }
    }

    /// Draws a background or decoration layer; instance layers belong to the screen.
    pub fn draw_layer(&self, canvas: &mut Canvas, layer: &Layer) {
        if !layer.visible {
            return;
        }
        match &layer.content {
            LayerContent::Background(background) => self.draw_background(canvas, background),
            LayerContent::Decorations(decorations) => {
                for decoration in decorations {
                    canvas.draw_sprite_ext(
                        decoration.sprite,
                        decoration.image_index,
                        decoration.x,
                        decoration.y,
                        decoration.xscale,
                        decoration.yscale,
                        decoration.angle,
                        decoration.blend,
                        decoration.alpha,
                    );
                }
            }
            LayerContent::Instances(_) => {}
        }
    }

    /// DrawLayerBackgroundElement of the runner: a tiled background repeats over the view.
    fn draw_background(&self, canvas: &mut Canvas, background: &Background) {
        let (view_x, view_y) = canvas.view();
        let Some(sprite) = background.sprite else {
            canvas.fill_rectangle(view_x, view_y, VIEW_WIDTH, VIEW_HEIGHT, background.colour, background.alpha);
            return;
        };
        let meta = canvas.sprites.get(sprite);
        let (image_width, image_height) = (meta.width as f64, meta.height as f64);
        let (xscale, yscale) = if background.stretch { (self.width / image_width, self.height / image_height) } else { (1.0, 1.0) };
        let (tile_width, tile_height) = (image_width * xscale, image_height * yscale);
        // obj_parallax: layer_x(layer, view_x * speed), where speed = 1 - the Tiled factor.
        let x = background.x + view_x * (1.0 - background.parallax_x);
        let y = background.y + view_y * (1.0 - background.parallax_y);

        let columns = tile_positions(x, tile_width, view_x, VIEW_WIDTH, background.tile_horizontally);
        let rows = tile_positions(y, tile_height, view_y, VIEW_HEIGHT, background.tile_vertically);
        let whole_image = [0.0, 0.0, image_width, image_height];
        for &top in &rows {
            for &left in &columns {
                canvas.draw_sprite_part_ext(sprite, background.image_index, whole_image, left, top, xscale, yscale, background.colour, background.alpha);
            }
        }
    }
}

/// Where copies of a tile of `tile_size` placed at `offset` start so that they cover
/// `visible_size` from `visible_start`; just `offset` when the layer does not repeat.
fn tile_positions(offset: f64, tile_size: f64, visible_start: f64, visible_size: f64, repeats: bool) -> Vec<f64> {
    if !repeats || tile_size <= 0.0 {
        return vec![offset];
    }
    let mut positions = Vec::new();
    let mut position = visible_start - (visible_start - offset).rem_euclid(tile_size);
    while position < visible_start + visible_size {
        positions.push(position);
        position += tile_size;
    }
    positions
}

/// Folders only group layers in the editor; GameMaker ignores their visibility
/// at runtime, so the layers inside keep their own.
fn collect_layers(layers: &[tiled::Layer], tiles: &TileLookup, sprites: &Sprites, out: &mut Vec<Layer>) -> Result<()> {
    for layer in layers {
        let depth = tiled::find_property(layer.properties(), "depth").and_then(|value| value.as_i64()).unwrap_or(0) as i32;
        let content = match layer {
            tiled::Layer::Group(group) => {
                collect_layers(&group.layers, tiles, sprites, out)?;
                continue;
            }
            tiled::Layer::ImageLayer(image) => LayerContent::Background(background(image)),
            tiled::Layer::ObjectGroup(group) => {
                let is_asset_layer = tiled::find_property(&group.properties, "asset_layer").and_then(|value| value.as_bool()) == Some(true);
                if is_asset_layer {
                    LayerContent::Decorations(group.objects.iter().filter_map(|object| decoration(object, tiles, sprites)).collect())
                } else {
                    LayerContent::Instances(group.objects.iter().map(|object| placed_instance(object, tiles, sprites)).collect::<Result<_>>()?)
                }
            }
        };
        let visible = match layer {
            tiled::Layer::ImageLayer(image) => image.visible,
            tiled::Layer::ObjectGroup(group) => group.visible,
            tiled::Layer::Group(_) => unreachable!("groups were flattened above"),
        };
        let (tint, greyscale) = colour_filter(layer.properties());
        out.push(Layer { name: layer.name().to_string(), depth, visible, content, heat_haze: heat_haze(layer.properties()), tint, greyscale });
    }
    Ok(())
}

/// The layer filter `_filter_heathaze` of a map layer, with the numbers the map gives it.
fn heat_haze(properties: &[tiled::Property]) -> Option<HeatHaze> {
    let text = tiled::find_property(properties, "effect").and_then(|value| value.as_str().map(str::to_string));
    if text.as_deref() != Some("_filter_heathaze") {
        return None;
    }
    let number = |name, missing| tiled::find_property(properties, name).and_then(|value| value.as_f64()).unwrap_or(missing) as f32;
    let camera = number("effect.g_CamOffsetScale", 1.0);
    Some(HeatHaze {
        first: (number("effect.g_Distort1Scale", 1.3), number("effect.g_Distort1Speed", 0.01), number("effect.g_Distort1Amount", 1.0), camera),
        second: (number("effect.g_Distort2Scale", 3.3), number("effect.g_Distort2Speed", 0.025), number("effect.g_Distort2Amount", 2.0), camera),
        chroma_spread: number("effect.g_ChromaSpreadAmount", 0.5),
    })
}

/// The layer filters _filter_tintfilter and _filter_greyscale: (tint, grey).
fn colour_filter(properties: &[tiled::Property]) -> (u32, bool) {
    let text = |name| tiled::find_property(properties, name).and_then(|value| value.as_str().map(str::to_string));
    match text("effect").as_deref() {
        Some("_filter_tintfilter") => (text("effect.g_TintCol").map_or(C_WHITE, |colour| parse_tiled_colour(&colour).0), false),
        Some("_filter_greyscale") => (C_WHITE, tiled::find_property(properties, "effect.g_Intensity").and_then(|value| value.as_f64()).unwrap_or(1.0) >= 0.5),
        _ => (C_WHITE, false),
    }
}

fn background(image: &tiled::ImageLayer) -> Background {
    let number = |name| tiled::find_property(&image.properties, name).and_then(|value| value.as_f64());
    let (colour, alpha) = image.tintcolor.as_deref().map_or((C_WHITE, 1.0), parse_tiled_colour);
    Background {
        sprite: sprite_by_image_path(&image.image),
        image_index: 0.0,
        colour,
        alpha: alpha * image.opacity,
        tile_horizontally: image.repeatx,
        tile_vertically: image.repeaty,
        stretch: tiled::find_property(&image.properties, "stretch").and_then(|value| value.as_bool()) == Some(true),
        hspeed: number("hspeed").unwrap_or(0.0),
        vspeed: number("vspeed").unwrap_or(0.0),
        parallax_x: image.parallaxx,
        parallax_y: image.parallaxy,
        animation_fps: number("animation_fps"),
        x: image.offsetx,
        y: image.offsety,
    }
}

fn decoration(object: &tiled::Object, tiles: &TileLookup, sprites: &Sprites) -> Option<Decoration> {
    let tile = object.tile_gid().and_then(|gid| tiles.get(gid));
    let Some(sprite) = tile.and_then(|tile| sprite_by_image_path(&tile.image)) else {
        eprintln!("map object {} is not a known sprite, skipped", object.id);
        return None;
    };
    let meta = sprites.get(sprite);
    let placement = map_object_placement(object, tile, (meta.origin_x, meta.origin_y));
    let number = |name| tiled::find_property(&object.properties, name).and_then(|value| value.as_f64());
    let (blend, alpha) = tiled::find_property(&object.properties, "image_blend").and_then(|value| value.as_str()).map_or((C_WHITE, 1.0), parse_tiled_colour);
    Some(Decoration {
        sprite,
        x: placement.x,
        y: placement.y,
        xscale: placement.scale_x,
        yscale: placement.scale_y,
        angle: placement.rotation,
        image_index: number("frame").unwrap_or(0.0),
        animation_speed: number("animation_speed").unwrap_or(1.0),
        blend,
        alpha,
    })
}

fn placed_instance(object: &tiled::Object, tiles: &TileLookup, sprites: &Sprites) -> Result<PlacedInstance> {
    let tile = object.tile_gid().and_then(|gid| tiles.get(gid));
    let object_id = map_object_class(object, tile)?;
    let origin = object_id.info().sprite.map_or((0.0, 0.0), |sprite| {
        let meta = sprites.get(sprite);
        (meta.origin_x, meta.origin_y)
    });
    let placement = map_object_placement(object, tile, origin);
    Ok(PlacedInstance {
        id: object.id,
        object: object_id,
        x: placement.x,
        y: placement.y,
        xscale: placement.scale_x,
        yscale: placement.scale_y,
        angle: placement.rotation,
        variables: object.properties.clone(),
    })
}

/// Maps and tilesets point at image files; the file name is the sprite's name.
fn sprite_by_image_path(image: &str) -> Option<SpriteId> {
    let name = Path::new(image).file_stem()?.to_str()?;
    SPRITE_FILES.iter().position(|file| *file == name).map(SpriteId)
}

/// Tiled writes "#AARRGGBB" (or "#RRGGBB"); GameMaker wants 0xBBGGRR and an alpha.
fn parse_tiled_colour(text: &str) -> (u32, f64) {
    let digits = text.trim_start_matches('#');
    let Ok(value) = u32::from_str_radix(digits, 16) else { return (C_WHITE, 1.0) };
    let alpha = if digits.len() == 8 { (value >> 24) as f64 / 255.0 } else { 1.0 };
    let [blue, green, red, _] = value.to_le_bytes();
    (crate::client::canvas::make_color_rgb(red, green, blue), alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiled_colours_become_gamemaker_colours() {
        assert_eq!(parse_tiled_colour("#ff282828"), (0x282828, 1.0));
        assert_eq!(parse_tiled_colour("#80c20037"), (0x3700C2, 128.0 / 255.0));
        assert_eq!(parse_tiled_colour("#0000ff"), (0xFF0000, 1.0));
    }

    /// The cut level of the 12.2022 alpha, imported into Resources/Maps beside the maps the
    /// game plays (nothing loads it yet): it has to be a map this code can read, with the
    /// tilesets, pictures and object types it names all there.
    #[test]
    fn the_imported_cut_level_is_a_map_this_game_can_read() {
        let maps = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Resources/Maps");
        let map = crate::format::tiled::load_map(&maps.join("level1.tmj")).unwrap();
        assert_eq!((map.pixel_width(), map.pixel_height()), (4250, 1280), "the room of the alpha");

        let mut tiles = std::collections::HashMap::new();
        for reference in &map.tilesets {
            let path = crate::format::tiled::relative_to(&maps.join("level1.tmj"), &reference.source);
            let tileset = crate::format::tiled::load_tileset(&path).unwrap();
            for tile in &tileset.tiles {
                let image = crate::format::tiled::relative_to(&path, &tile.image);
                assert!(image.exists(), "{} is missing", image.display());
                tiles.insert(reference.firstgid + tile.id, tile.clone());
            }
        }

        let mut objects = 0;
        for layer in &map.layers {
            let crate::format::tiled::Layer::ObjectGroup(group) = layer else { continue };
            for object in &group.objects {
                objects += 1;
                if !object.class.is_empty() {
                    crate::core::world::map_object_class(object, None).unwrap();
                }
                if let Some(gid) = object.tile_gid() {
                    assert!(tiles.contains_key(&gid), "gid {gid} is in no tileset of the map");
                }
            }
        }
        assert_eq!(objects, 229, "71 instances and 158 pieces of level art");
    }

    #[test]
    fn tiled_backgrounds_cover_the_view() {
        assert_eq!(tile_positions(-0.5, 96.0, 0.0, 200.0, true), vec![-0.5, 95.5, 191.5]);
        assert_eq!(tile_positions(0.0, 96.0, 0.0, 200.0, true), vec![0.0, 96.0, 192.0]);
        assert_eq!(tile_positions(10.0, 96.0, 0.0, 200.0, false), vec![10.0]);
        // A view far into the room starts at the copy under its left edge.
        assert_eq!(tile_positions(10.0, 96.0, 1000.0, 200.0, true), vec![970.0, 1066.0, 1162.0]);
    }
}
