//! One sprite = one PNG file. Several frames = animated PNG (APNG).
//!
//! Frame count, size and playback speed travel inside the file itself, so a
//! texture pack author only needs an image editor that can export APNG.

use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

/// Decoded sprite: every frame is a full canvas of RGBA8 pixels.
pub struct SpriteImage {
    pub width: u32,
    pub height: u32,
    /// Frames per second from the APNG frame delay. 0 when unknown or not animated.
    pub fps: u16,
    pub frames: Vec<Vec<u8>>,
}

const MAX_PALETTE_COLORS: usize = 256;

/// Writes frames as an indexed PNG when all frames together use at most 256
/// RGBA colors, otherwise as RGBA. Both are lossless: the palette swap shader
/// compares exact colors, so any lossy step would break character skins.
pub fn write(path: &Path, image: &SpriteImage) -> Result<()> {
    let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), image.width, image.height);
    encoder.set_depth(png::BitDepth::Eight);

    let palette = build_palette(&image.frames);
    match &palette {
        Some(palette) => {
            encoder.set_color(png::ColorType::Indexed);
            encoder.set_palette(palette.rgb_bytes());
            encoder.set_trns(palette.alpha_bytes());
        }
        None => encoder.set_color(png::ColorType::Rgba),
    }

    if image.frames.len() > 1 {
        encoder.set_animated(image.frames.len() as u32, 0)?;
        // A delay of 1/fps seconds is exact because GameMaker speeds are whole numbers.
        // fps = 0 is written as delay 0 ("as fast as possible"); the sidecar carries the real meaning.
        encoder.set_frame_delay(if image.fps == 0 { 0 } else { 1 }, image.fps.max(1))?;
        encoder.set_dispose_op(png::DisposeOp::Background)?;
        encoder.set_blend_op(png::BlendOp::Source)?;
    }

    let mut writer = encoder.write_header()?;
    for frame in &image.frames {
        match &palette {
            Some(palette) => writer.write_image_data(&palette.index_pixels(frame))?,
            None => writer.write_image_data(frame)?,
        }
    }
    writer.finish()?;
    Ok(())
}

/// Reads any PNG or APNG (indexed, gray, RGB, 16 bit...) into full RGBA8 frames.
/// Sub-frames with offsets and all dispose/blend modes are composited, because
/// images exported by other tools often crop frames.
pub fn read(path: &Path) -> Result<SpriteImage> {
    let bytes = crate::core::resources::read(path)?;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info()?;

    let width = reader.info().width;
    let height = reader.info().height;
    let animation = reader.info().animation_control;
    let mut raw = vec![0u8; reader.output_buffer_size().context("image too large")?];

    let Some(animation) = animation else {
        let output = reader.next_frame(&mut raw)?;
        let frame = to_rgba8(&raw, output.color_type, output.width, output.height)?;
        return Ok(SpriteImage { width, height, fps: 0, frames: vec![frame] });
    };

    // The default image (IDAT) is only a frame when an fcTL chunk came before it.
    let default_image_is_frame = reader.info().frame_control.is_some();
    if !default_image_is_frame {
        reader.next_frame(&mut raw)?;
    }

    let mut canvas = vec![0u8; (width * height * 4) as usize];
    let mut frames = Vec::with_capacity(animation.num_frames as usize);
    let mut fps = 0;
    for frame_number in 0..animation.num_frames {
        let output = reader.next_frame(&mut raw)?;
        // The control chunk of a frame is parsed while reading that frame, so it
        // must be taken after next_frame, not before.
        let control = reader.info().frame_control.context("APNG frame without fcTL")?;
        if frame_number == 0 {
            fps = fps_from_delay(&control);
        }
        let pixels = to_rgba8(&raw, output.color_type, output.width, output.height)?;

        let before = canvas.clone();
        blit(&mut canvas, width, &pixels, &control);
        frames.push(canvas.clone());

        match control.dispose_op {
            png::DisposeOp::None => {}
            png::DisposeOp::Background => clear_rect(&mut canvas, width, &control),
            png::DisposeOp::Previous => canvas = before,
        }
    }
    Ok(SpriteImage { width, height, fps, frames })
}

fn fps_from_delay(control: &png::FrameControl) -> u16 {
    if control.delay_num == 0 {
        return 0;
    }
    // APNG says a denominator of 0 means 1/100 of a second.
    let denominator = if control.delay_den == 0 { 100 } else { control.delay_den };
    denominator / control.delay_num
}

fn to_rgba8(raw: &[u8], color_type: png::ColorType, width: u32, height: u32) -> Result<Vec<u8>> {
    let pixel_count = (width * height) as usize;
    let mut rgba = Vec::with_capacity(pixel_count * 4);
    match color_type {
        png::ColorType::Rgba => rgba.extend_from_slice(&raw[..pixel_count * 4]),
        png::ColorType::Rgb => {
            for pixel in raw[..pixel_count * 3].chunks_exact(3) {
                rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for pixel in raw[..pixel_count * 2].chunks_exact(2) {
                rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
            }
        }
        png::ColorType::Grayscale => {
            for &gray in &raw[..pixel_count] {
                rgba.extend_from_slice(&[gray, gray, gray, 255]);
            }
        }
        png::ColorType::Indexed => bail!("indexed output left after EXPAND transformation"),
    }
    Ok(rgba)
}

fn blit(canvas: &mut [u8], canvas_width: u32, pixels: &[u8], control: &png::FrameControl) {
    for row in 0..control.height {
        for column in 0..control.width {
            let source = ((row * control.width + column) * 4) as usize;
            let target = (((control.y_offset + row) * canvas_width + control.x_offset + column) * 4) as usize;
            let source_pixel = &pixels[source..source + 4];
            match control.blend_op {
                png::BlendOp::Source => canvas[target..target + 4].copy_from_slice(source_pixel),
                png::BlendOp::Over => blend_over(&mut canvas[target..target + 4], source_pixel),
            }
        }
    }
}

/// Standard "source over destination" alpha compositing on straight alpha.
fn blend_over(destination: &mut [u8], source: &[u8]) {
    let source_alpha = source[3] as u32;
    if source_alpha == 255 {
        destination.copy_from_slice(source);
        return;
    }
    if source_alpha == 0 {
        return;
    }
    let destination_alpha = destination[3] as u32;
    let out_alpha = source_alpha + destination_alpha * (255 - source_alpha) / 255;
    for channel in 0..3 {
        let mixed = source[channel] as u32 * source_alpha
            + destination[channel] as u32 * destination_alpha * (255 - source_alpha) / 255;
        destination[channel] = (mixed / out_alpha.max(1)) as u8;
    }
    destination[3] = out_alpha as u8;
}

fn clear_rect(canvas: &mut [u8], canvas_width: u32, control: &png::FrameControl) {
    for row in 0..control.height {
        let start = (((control.y_offset + row) * canvas_width + control.x_offset) * 4) as usize;
        canvas[start..start + (control.width * 4) as usize].fill(0);
    }
}

struct Palette {
    colors: Vec<[u8; 4]>,
    index_of: HashMap<[u8; 4], u8>,
}

impl Palette {
    fn rgb_bytes(&self) -> Vec<u8> {
        self.colors.iter().flat_map(|color| [color[0], color[1], color[2]]).collect()
    }

    fn alpha_bytes(&self) -> Vec<u8> {
        self.colors.iter().map(|color| color[3]).collect()
    }

    fn index_pixels(&self, frame: &[u8]) -> Vec<u8> {
        frame
            .chunks_exact(4)
            .map(|pixel| self.index_of[&[pixel[0], pixel[1], pixel[2], pixel[3]]])
            .collect()
    }
}

/// Colors are numbered in order of first appearance so that converting the
/// same sprite twice gives the same file.
fn build_palette(frames: &[Vec<u8>]) -> Option<Palette> {
    let mut palette = Palette { colors: Vec::new(), index_of: HashMap::new() };
    for frame in frames {
        for pixel in frame.chunks_exact(4) {
            let color = [pixel[0], pixel[1], pixel[2], pixel[3]];
            if palette.index_of.contains_key(&color) {
                continue;
            }
            if palette.colors.len() == MAX_PALETTE_COLORS {
                return None;
            }
            palette.index_of.insert(color, palette.colors.len() as u8);
            palette.colors.push(color);
        }
    }
    Some(palette)
}

