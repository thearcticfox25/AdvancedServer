//! The 480x270 picture a room draws into, with GameMaker's sprite drawing
//! functions. At the end of a frame the picture is stretched over the whole
//! window without smoothing (the project's "full scale" and "no interpolation"
//! options).

use crate::client::font::{Font, Glyph};
use crate::client::palette::Colours;
use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Equation, PipelineParams, UniformDesc, UniformType};
use macroquad::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use crate::format::apng;
use crate::core::resources::sprites::Sprites;
use crate::core::resources::SpriteId;

/// GameMaker colour constants (0xBBGGRR) that the client draws with.
pub const C_RED: u32 = 0x0000FF;
pub const C_GREEN: u32 = 0x008000;
pub const C_DKGRAY: u32 = 0x404040;

pub use crate::core::{VIEW_HEIGHT, VIEW_WIDTH};

/// make_color_rgb: GameMaker keeps colours as 0xBBGGRR.
pub const fn make_color_rgb(red: u8, green: u8, blue: u8) -> u32 {
    (blue as u32) << 16 | (green as u32) << 8 | red as u32
}

/// shd_colour takes 256 floats per list: 64 colours.
const MAX_PALETTE_COLOURS: usize = 64;

const PALETTE_VERTEX_SHADER: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
varying lowp vec2 uv;
varying lowp vec4 color;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    color = color0 / 255.0;
    uv = texcoord;
}"#;

/// shd_colour: a pixel equal (within 0.01 per channel) to a pattern colour is
/// drawn as that colour's replacement.
const PALETTE_FRAGMENT_SHADER: &str = r#"#version 100
precision highp float;
varying lowp vec2 uv;
varying lowp vec4 color;
uniform sampler2D Texture;
uniform vec4 Pattern[64];
uniform vec4 Replacements[64];
uniform int ColourCount;
void main() {
    vec4 texel = texture2D(Texture, uv);
    for (int i = 0; i < 64; i++) {
        if (i >= ColourCount) {
            break;
        }
        if (all(lessThan(abs(texel - Pattern[i]), vec4(0.01)))) {
            texel = Replacements[i];
            break;
        }
    }
    gl_FragColor = color * texel;
}"#;

/// shd_blackwhite: every pixel as its brightness.
const BLACK_WHITE_FRAGMENT_SHADER: &str = r#"#version 100
precision highp float;
varying lowp vec2 uv;
varying lowp vec4 color;
uniform sampler2D Texture;
void main() {
    vec4 source = color * texture2D(Texture, uv);
    float brightness = dot(vec3(0.2989, 0.5870, 0.1140), source.rgb);
    gl_FragColor = vec4(vec3(brightness), source.a);
}"#;

/// shd_inverse: inside a circle around a point the picture is inverted and swirled.
const INVERSE_FRAGMENT_SHADER: &str = r#"#version 100
precision highp float;
varying lowp vec2 uv;
varying lowp vec4 color;
uniform sampler2D Texture;
uniform vec2 Point;
uniform float Radius;
uniform float Scale;
void main() {
    vec2 view_size = vec2(480.0, 270.0);
    // The picture is stored upside down.
    vec2 pixel = vec2(uv.x, 1.0 - uv.y) * view_size;
    float len = length(pixel - Point) * Scale;
    float radius = Radius * Scale;
    if (len < radius) {
        float size = max(len - radius / 2.0, 0.0) / radius * 10.0 * Scale;
        vec2 offset = vec2(cos(len * 0.2), -sin(len * 0.2)) * size / (view_size * Scale);
        vec4 source = color * texture2D(Texture, uv + offset);
        gl_FragColor = vec4(1.0 - source.rgb, source.a);
    } else {
        gl_FragColor = color * texture2D(Texture, uv);
    }
}"#;

/// GameMaker's _filter_heathaze, as the maps set it up: two scrolling samples of a
/// noise picture push the pixels around, and the colours are spread a little apart.
/// The numbers come from the layer the map put the filter on.
const HEAT_HAZE_FRAGMENT_SHADER: &str = r#"#version 100
precision highp float;
varying lowp vec2 uv;
varying lowp vec4 color;
uniform sampler2D Texture;
uniform sampler2D Noise;
uniform vec2 View;
uniform float Time;
uniform vec4 Distort1;
uniform vec4 Distort2;
uniform float ChromaSpread;
void main() {
    vec2 view_size = vec2(480.0, 270.0);
    // Distort<n> is (scale, speed, amount, camera offset scale).
    vec2 world = uv * view_size + View * Distort1.w;
    vec2 first = texture2D(Noise, world / (view_size * Distort1.x) + vec2(0.0, -Time * Distort1.y)).rg - 0.5;
    vec2 second = texture2D(Noise, world / (view_size * Distort2.x) + vec2(0.0, -Time * Distort2.y)).rg - 0.5;
    vec2 offset = (first * Distort1.z + second * Distort2.z) / view_size;
    vec4 source = texture2D(Texture, uv + offset);
    float spread = ChromaSpread / 100.0;
    source.r = texture2D(Texture, uv + offset * (1.0 + spread)).r;
    source.b = texture2D(Texture, uv + offset * (1.0 - spread)).b;
    gl_FragColor = color * source;
}"#;

/// What a map asks the heat haze for (the effect properties of its layers).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeatHaze {
    /// scale, speed, amount and camera offset scale of the first noise sample.
    pub first: (f32, f32, f32, f32),
    pub second: (f32, f32, f32, f32),
    pub chroma_spread: f32,
}

pub struct Canvas {
    pub sprites: Arc<Sprites>,
    palette_swap: Material,
    black_white: Material,
    inverse: Material,
    heat_haze: Material,
    /// The picture the heat haze reads while it draws into the one being built.
    scratch: RenderTarget,
    /// The inverted circle over the whole picture (view position and radius), if any.
    pub inverse_circle: Option<(f64, f64, f64)>,
    /// Uploaded frames, by sprite. A sprite is uploaded the first time it is drawn.
    // ponytail: textures stay until the game closes; drop them on room change once levels fill the GPU
    frames: HashMap<SpriteId, Vec<Texture2D>>,
    target: RenderTarget,
    camera: Camera2D,
    /// view_camera[0]: the room position shown at the picture's top-left corner.
    view: (f64, f64),
    /// _filter_tintfilter of the layer being drawn: every sprite's colour times this.
    layer_tint: u32,
    /// The menu buttons' lettering, fnt_big and the chat lettering, and their
    /// pictures once uploaded.
    fonts: [Font; 3],
    font_textures: [Option<Texture2D>; 3],
}

/// The bitmap fonts a canvas draws with.
#[derive(Clone, Copy)]
pub enum TextFont {
    Menu = 0,
    /// fnt_big
    Big = 1,
    /// The letters the original drew chat, names and the version with (spr_letter1).
    Chat = 2,
}

impl Canvas {
    pub fn new(sprites: Arc<Sprites>, fonts: [Font; 3]) -> Canvas {
        let target = render_target(VIEW_WIDTH as u32, VIEW_HEIGHT as u32);
        target.texture.set_filter(FilterMode::Nearest);
        let camera = view_camera(&target, 0.0, 0.0, VIEW_WIDTH, VIEW_HEIGHT);
        Canvas {
            sprites,
            palette_swap: palette_swap_material(),
            black_white: black_white_material(),
            inverse: inverse_material(),
            heat_haze: heat_haze_material(),
            scratch: {
                let scratch = render_target(VIEW_WIDTH as u32, VIEW_HEIGHT as u32);
                scratch.texture.set_filter(FilterMode::Nearest);
                scratch
            },
            inverse_circle: None,
            frames: HashMap::new(),
            target,
            camera,
            view: (0.0, 0.0),
            layer_tint: crate::core::world::C_WHITE,
            fonts,
            font_textures: [None, None, None],
        }
    }

    /// Draws one line of `text` in the menu font with its top-left corner at (x, y).
    pub fn draw_menu_text(&mut self, x: f64, y: f64, text: &str, colour: u32, alpha: f64) {
        self.draw_font_text(TextFont::Menu, x, y, text, colour, alpha, 1.0);
    }

    /// Draws one line of `text` with its top-left corner at (x, y), `scale` times as big.
    pub fn draw_font_text(&mut self, font: TextFont, x: f64, y: f64, text: &str, colour: u32, alpha: f64, scale: f64) {
        let (font, texture) = (&self.fonts[font as usize], &mut self.font_textures[font as usize]);
        let texture = texture.get_or_insert_with(|| {
            let texture = Texture2D::from_rgba8(font.image.width as u16, font.image.height as u16, &font.image.frames[0]);
            texture.set_filter(FilterMode::Nearest);
            texture
        });
        let (texture, size) = (texture.clone(), (font.image.width as f64, font.image.height as f64));
        let placed: Vec<(f64, Glyph)> = font.layout(text).map(|(start, glyph)| (start, *glyph)).collect();
        for (start, glyph) in placed {
            draw_glyph(&texture, size, &glyph, x + start * scale, y, scale, colour, alpha);
        }
    }

    /// One glyph of `font` with its top-left corner at (x, y), for text laid out by
    /// the caller (the chat lettering colours every letter on its own).
    pub fn draw_font_glyph(&mut self, font: TextFont, glyph: &Glyph, x: f64, y: f64, colour: u32, alpha: f64) {
        let (font, texture) = (&self.fonts[font as usize], &mut self.font_textures[font as usize]);
        let texture = texture.get_or_insert_with(|| {
            let texture = Texture2D::from_rgba8(font.image.width as u16, font.image.height as u16, &font.image.frames[0]);
            texture.set_filter(FilterMode::Nearest);
            texture
        });
        let size = (font.image.width as f64, font.image.height as f64);
        draw_glyph(&texture.clone(), size, glyph, x, y, 1.0, colour, alpha);
    }

    /// The font itself, for code that lays text out letter by letter.
    pub fn font(&self, font: TextFont) -> &Font {
        &self.fonts[font as usize]
    }

    pub fn menu_text_width(&self, text: &str) -> f64 {
        self.fonts[TextFont::Menu as usize].width(text)
    }

    /// The width and line height of `text` in `font`.
    pub fn font_text_size(&self, font: TextFont, text: &str) -> (f64, f64) {
        let font = &self.fonts[font as usize];
        (font.width(text), font.line_height())
    }

    /// scr_palette_swap: sprites drawn until `reset_shader` get their colours replaced.
    pub fn set_palette_swap(&self, from: &Colours, to: &Colours) {
        let as_uniform = |colours: &Colours| {
            let mut padded = [Vec4::ZERO; MAX_PALETTE_COLOURS];
            for (slot, colour) in padded.iter_mut().zip(colours) {
                let [red, green, blue] = colour.map(|channel| crate::client::palette::fraction(channel) as f32);
                *slot = Vec4::new(red, green, blue, 1.0);
            }
            padded
        };
        let count = from.len().min(to.len()).min(MAX_PALETTE_COLOURS) as i32;
        gl_use_material(&self.palette_swap);
        self.palette_swap.set_uniform_array("Pattern", &as_uniform(from));
        self.palette_swap.set_uniform_array("Replacements", &as_uniform(to));
        self.palette_swap.set_uniform("ColourCount", count);
    }

    /// A layer's _filter_tintfilter while it is drawn; C_WHITE when it is done.
    pub fn set_layer_tint(&mut self, tint: u32) {
        self.layer_tint = tint;
    }

    /// shader_set(shd_blackwhite), and _filter_greyscale at full intensity
    pub fn set_black_white(&self) {
        gl_use_material(&self.black_white);
    }

    /// shader_reset
    pub fn reset_shader(&self) {
        gl_use_default_material();
    }

    /// Starts a new picture; everything drawn until the next `begin` goes into it.
    /// Drawing starts in the picture's own coordinates, as in a room without a view.
    pub fn begin(&mut self) {
        self.set_view(0.0, 0.0);
        clear_background(BLACK);
    }

    /// camera_set_view_pos: what follows is drawn in room coordinates, with this room
    /// position at the picture's top-left corner. Draw GUI is the view at (0, 0).
    pub fn set_view(&mut self, x: f64, y: f64) {
        self.set_view_sized(x, y, VIEW_WIDTH, VIEW_HEIGHT);
    }

    /// camera_set_view_pos and camera_set_view_size: a view larger than the picture
    /// shows more of the room, squeezed into it (Dark Tower's jumpscare).
    pub fn set_view_sized(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.view = (x, y);
        self.camera = view_camera(&self.target, x, y, width, height);
        set_camera(&self.camera);
    }

    /// camera_get_view_x, camera_get_view_y
    pub fn view(&self) -> (f64, f64) {
        self.view
    }

    /// GameMaker's layer filter: everything drawn so far is pushed around by the heat
    /// haze, as a layer with `_filter_heathaze` does to what lies under it. Called while
    /// the level is being drawn, so the HUD drawn later stays still.
    pub fn heat_haze(&mut self, haze: HeatHaze, seconds: f64) {
        let view = self.view;
        // The picture so far, copied aside: a shader may not read what it writes.
        let scratch_camera = view_camera(&self.scratch, 0.0, 0.0, VIEW_WIDTH, VIEW_HEIGHT);
        set_camera(&scratch_camera);
        clear_background(BLANK);
        draw_texture_ex(&self.target.texture, 0.0, 0.0, WHITE, DrawTextureParams { dest_size: Some(vec2(VIEW_WIDTH as f32, VIEW_HEIGHT as f32)), ..Default::default() });
        set_camera(&self.camera);
        let noise = self.frame_texture(crate::core::resources::names::sprite::SPR_HEATHAZE_NOISE, 0.0);
        if let Some(noise) = noise {
            self.heat_haze.set_texture("Noise", noise);
        }
        self.heat_haze.set_uniform("View", (view.0 as f32, view.1 as f32));
        self.heat_haze.set_uniform("Time", seconds as f32);
        self.heat_haze.set_uniform("Distort1", haze.first);
        self.heat_haze.set_uniform("Distort2", haze.second);
        self.heat_haze.set_uniform("ChromaSpread", haze.chroma_spread);
        gl_use_material(&self.heat_haze);
        draw_texture_ex(
            &self.scratch.texture,
            view.0 as f32,
            view.1 as f32,
            WHITE,
            DrawTextureParams { dest_size: Some(vec2(VIEW_WIDTH as f32, VIEW_HEIGHT as f32)), ..Default::default() },
        );
        gl_use_default_material();
    }

    /// Shows the latest picture in the window.
    pub fn present(&self) {
        set_default_camera();
        clear_background(BLACK);
        let whole_window = DrawTextureParams {
            dest_size: Some(vec2(screen_width(), screen_height())),
            // Render targets are stored upside down.
            flip_y: true,
            ..Default::default()
        };
        if let Some((x, y, radius)) = self.inverse_circle.filter(|&(_, _, radius)| radius > 0.0) {
            self.inverse.set_uniform("Point", (x as f32, y as f32));
            self.inverse.set_uniform("Radius", radius as f32);
            self.inverse.set_uniform("Scale", screen_width() / VIEW_WIDTH as f32);
            gl_use_material(&self.inverse);
            draw_texture_ex(&self.target.texture, 0.0, 0.0, WHITE, whole_window);
            gl_use_default_material();
        } else {
            draw_texture_ex(&self.target.texture, 0.0, 0.0, WHITE, whole_window);
        }
    }

    /// A window pixel as a position in the picture (mouse_x, mouse_y).
    pub fn window_to_view(&self, x: f32, y: f32) -> (f64, f64) {
        (x as f64 * VIEW_WIDTH / screen_width() as f64, y as f64 * VIEW_HEIGHT / screen_height() as f64)
    }

    pub fn draw_sprite(&mut self, sprite: SpriteId, frame: f64, x: f64, y: f64) {
        self.draw_sprite_ext(sprite, frame, x, y, 1.0, 1.0, 0.0, crate::core::world::C_WHITE, 1.0);
    }

    /// draw_sprite_ext: the sprite origin lands on (x, y); scale and rotation
    /// (degrees, counter-clockwise) happen around it. `blend` tints, `alpha` fades.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_sprite_ext(&mut self, sprite: SpriteId, frame: f64, x: f64, y: f64, xscale: f64, yscale: f64, angle: f64, blend: u32, alpha: f64) {
        let (x, y) = snap(x, y);
        let meta = self.sprites.get(sprite);
        let (width, height, origin_x, origin_y) = (meta.width as f64, meta.height as f64, meta.origin_x, meta.origin_y);
        let Some(texture) = self.frame_texture(sprite, frame) else { return };

        let left = -origin_x * xscale;
        let right = (width - origin_x) * xscale;
        let top = -origin_y * yscale;
        let bottom = (height - origin_y) * yscale;
        let corners = [(left, top), (right, top), (right, bottom), (left, bottom)].map(|(dx, dy)| {
            let (turned_x, turned_y) = rotate(dx, dy, angle);
            (x + turned_x, y + turned_y)
        });
        draw_textured_quad(texture, corners, [0.0, 0.0, 1.0, 1.0], gm_color(multiply_colours(blend, self.layer_tint), alpha));
    }

    /// draw_sprite_part_ext: a rectangle cut out of the sprite image, its top-left
    /// corner at (x, y). The sprite origin is not used. Parts outside the image are cut off.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_sprite_part_ext(&mut self, sprite: SpriteId, frame: f64, part: [f64; 4], x: f64, y: f64, xscale: f64, yscale: f64, blend: u32, alpha: f64) {
        let (x, y) = snap(x, y);
        let meta = self.sprites.get(sprite);
        let (width, height) = (meta.width as f64, meta.height as f64);
        let Some(texture) = self.frame_texture(sprite, frame) else { return };

        let [part_left, part_top, part_width, part_height] = part;
        let (clipped_left, clipped_top) = (part_left.max(0.0), part_top.max(0.0));
        let clipped_right = (part_left + part_width).min(width);
        let clipped_bottom = (part_top + part_height).min(height);
        if clipped_right <= clipped_left || clipped_bottom <= clipped_top {
            return;
        }
        let screen_left = x + (clipped_left - part_left) * xscale;
        let screen_top = y + (clipped_top - part_top) * yscale;
        let screen_right = screen_left + (clipped_right - clipped_left) * xscale;
        let screen_bottom = screen_top + (clipped_bottom - clipped_top) * yscale;
        let corners = [(screen_left, screen_top), (screen_right, screen_top), (screen_right, screen_bottom), (screen_left, screen_bottom)];
        let uv = [clipped_left / width, clipped_top / height, clipped_right / width, clipped_bottom / height].map(|value| value as f32);
        draw_textured_quad(texture, corners, uv, gm_color(multiply_colours(blend, self.layer_tint), alpha));
    }

    /// draw_rectangle filled with one colour (backgrounds without a sprite).
    pub fn fill_rectangle(&mut self, left: f64, top: f64, width: f64, height: f64, colour: u32, alpha: f64) {
        draw_rectangle(left as f32, top as f32, width as f32, height as f32, gm_color(multiply_colours(colour, self.layer_tint), alpha));
    }

    /// `frame` counts on past the last frame and wraps around, like image_index.
    fn frame_texture(&mut self, sprite: SpriteId, frame: f64) -> Option<Texture2D> {
        let frames = self.frames.entry(sprite).or_insert_with(|| upload_frames(&self.sprites, sprite));
        if frames.is_empty() {
            return None;
        }
        let index = (frame.floor() as i64).rem_euclid(frames.len() as i64) as usize;
        Some(frames[index].clone())
    }
}

/// Sprites land on whole pixels. Drawn half a pixel off (a centred word, a resource placed
/// at x.5 in a room), each texel sits on a pixel edge and rounds its own way, so letters
/// drift apart; the menu font's glyphs are floored the same way.
/// Puts one glyph on the canvas: its cell of the font picture, `scale` times as big,
/// snapped to whole pixels so letters never land between them.
fn draw_glyph(texture: &Texture2D, size: (f64, f64), glyph: &Glyph, x: f64, y: f64, scale: f64, colour: u32, alpha: f64) {
    let (left, top) = ((x + glyph.offset as f64 * scale).floor(), y.floor());
    let (gw, gh) = (glyph.w as f64, glyph.h as f64);
    let (w, h) = (gw * scale, gh * scale);
    let corners = [(left, top), (left + w, top), (left + w, top + h), (left, top + h)];
    let uv = [glyph.x as f64 / size.0, glyph.y as f64 / size.1, (glyph.x as f64 + gw) / size.0, (glyph.y as f64 + gh) / size.1].map(|value| value as f32);
    draw_textured_quad(texture.clone(), corners, uv, gm_color(colour, alpha));
}

fn snap(x: f64, y: f64) -> (f64, f64) {
    (x.floor(), y.floor())
}

fn view_camera(target: &RenderTarget, x: f64, y: f64, width: f64, height: f64) -> Camera2D {
    let mut camera = Camera2D::from_display_rect(Rect::new(x as f32, y as f32, width as f32, height as f32));
    camera.render_target = Some(target.clone());
    camera
}

fn black_white_material() -> Material {
    let shader = ShaderSource::Glsl { vertex: PALETTE_VERTEX_SHADER, fragment: BLACK_WHITE_FRAGMENT_SHADER };
    let pipeline_params = PipelineParams {
        color_blend: Some(BlendState::new(Equation::Add, BlendFactor::Value(BlendValue::SourceAlpha), BlendFactor::OneMinusValue(BlendValue::SourceAlpha))),
        ..Default::default()
    };
    load_material(shader, MaterialParams { pipeline_params, ..Default::default() }).expect("the black and white shader compiles")
}

fn heat_haze_material() -> Material {
    let shader = ShaderSource::Glsl { vertex: PALETTE_VERTEX_SHADER, fragment: HEAT_HAZE_FRAGMENT_SHADER };
    let uniforms = vec![
        UniformDesc::new("View", UniformType::Float2),
        UniformDesc::new("Time", UniformType::Float1),
        UniformDesc::new("Distort1", UniformType::Float4),
        UniformDesc::new("Distort2", UniformType::Float4),
        UniformDesc::new("ChromaSpread", UniformType::Float1),
    ];
    let params = MaterialParams { uniforms, textures: vec!["Noise".to_string()], ..Default::default() };
    load_material(shader, params).expect("the heat haze shader compiles")
}

fn inverse_material() -> Material {
    let shader = ShaderSource::Glsl { vertex: PALETTE_VERTEX_SHADER, fragment: INVERSE_FRAGMENT_SHADER };
    let uniforms = vec![UniformDesc::new("Point", UniformType::Float2), UniformDesc::new("Radius", UniformType::Float1), UniformDesc::new("Scale", UniformType::Float1)];
    load_material(shader, MaterialParams { uniforms, ..Default::default() }).expect("the inverse shader compiles")
}

fn palette_swap_material() -> Material {
    let shader = ShaderSource::Glsl { vertex: PALETTE_VERTEX_SHADER, fragment: PALETTE_FRAGMENT_SHADER };
    let uniforms = vec![
        UniformDesc::new("Pattern", UniformType::Float4).array(MAX_PALETTE_COLOURS),
        UniformDesc::new("Replacements", UniformType::Float4).array(MAX_PALETTE_COLOURS),
        UniformDesc::new("ColourCount", UniformType::Int1),
    ];
    let pipeline_params = PipelineParams {
        color_blend: Some(BlendState::new(Equation::Add, BlendFactor::Value(BlendValue::SourceAlpha), BlendFactor::OneMinusValue(BlendValue::SourceAlpha))),
        ..Default::default()
    };
    load_material(shader, MaterialParams { pipeline_params, uniforms, ..Default::default() }).expect("the palette shader compiles")
}

fn upload_frames(sprites: &Sprites, sprite: SpriteId) -> Vec<Texture2D> {
    // Same policy as the sprite metadata: a sprite the code uses must exist.
    let path = sprites.path(sprite).unwrap_or_else(|| panic!("sprite {sprite:?} has no file in Textures"));
    let image = apng::read(path).unwrap_or_else(|error| panic!("sprite {}: {error:#}", path.display()));
    image
        .frames
        .iter()
        .map(|pixels| {
            let texture = Texture2D::from_rgba8(image.width as u16, image.height as u16, pixels);
            texture.set_filter(FilterMode::Nearest);
            texture
        })
        .collect()
}

/// GameMaker turns counter-clockwise as seen on screen, where y grows downwards.
fn rotate(dx: f64, dy: f64, angle_degrees: f64) -> (f64, f64) {
    let (sin, cos) = angle_degrees.to_radians().sin_cos();
    (dx * cos + dy * sin, -dx * sin + dy * cos)
}

/// Two colours multiplied channel by channel, as a tint does to what it covers.
fn multiply_colours(a: u32, b: u32) -> u32 {
    let (a, b) = (a.to_le_bytes(), b.to_le_bytes());
    let channel = |i: usize| (u32::from(a[i]) * u32::from(b[i]) + 127) / 255;
    channel(0) | channel(1) << 8 | channel(2) << 16
}

fn gm_color(colour: u32, alpha: f64) -> Color {
    let [red, green, blue, _] = colour.to_le_bytes();
    Color::from_rgba(red, green, blue, (alpha.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// `corners` go clockwise from the top-left of the image; `uv` is left, top, right, bottom.
fn draw_textured_quad(texture: Texture2D, corners: [(f64, f64); 4], uv: [f32; 4], color: Color) {
    let [u_left, v_top, u_right, v_bottom] = uv;
    let uv_corners = [(u_left, v_top), (u_right, v_top), (u_right, v_bottom), (u_left, v_bottom)];
    let vertices = corners.iter().zip(uv_corners).map(|(&(x, y), (u, v))| Vertex::new(x as f32, y as f32, 0.0, u, v, color)).collect();
    draw_mesh(&Mesh { vertices, indices: vec![0, 1, 2, 0, 2, 3], texture: Some(texture) });
}
