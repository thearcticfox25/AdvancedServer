//! Converting between how GameMaker and Tiled describe a placed sprite.
//!
//! GameMaker: position of the sprite origin, signed scale (negative = mirrored),
//! rotation counter-clockwise around the origin.
//! Tiled: position of the top-left corner of the (rotated) box, positive size
//! plus flip flags, rotation clockwise around that corner.
//!
//! Both directions live here so the converter and the game can never disagree.

/// A sprite placed the GameMaker way.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GmPlacement {
    pub x: f64,
    pub y: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    /// Degrees, counter-clockwise on screen.
    pub rotation: f64,
}

/// The same sprite placed the Tiled way (tile object with top-left alignment).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TiledPlacement {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Degrees, clockwise on screen.
    pub rotation: f64,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
}

/// Image size and origin of the sprite being placed.
#[derive(Debug, Clone, Copy)]
pub struct SpriteFrame {
    pub width: f64,
    pub height: f64,
    pub origin_x: f64,
    pub origin_y: f64,
}

pub fn gm_to_tiled(gm: GmPlacement, sprite: SpriteFrame) -> TiledPlacement {
    let (left, top) = box_corner_offset(gm.scale_x, gm.scale_y, sprite);
    let (offset_x, offset_y) = rotate_counter_clockwise(left, top, gm.rotation);
    TiledPlacement {
        x: gm.x + offset_x,
        y: gm.y + offset_y,
        width: sprite.width * gm.scale_x.abs(),
        height: sprite.height * gm.scale_y.abs(),
        rotation: normalize_degrees(-gm.rotation),
        flip_horizontal: gm.scale_x < 0.0,
        flip_vertical: gm.scale_y < 0.0,
    }
}

pub fn tiled_to_gm(tiled: TiledPlacement, sprite: SpriteFrame) -> GmPlacement {
    let sign_x = if tiled.flip_horizontal { -1.0 } else { 1.0 };
    let sign_y = if tiled.flip_vertical { -1.0 } else { 1.0 };
    let scale_x = snap(sign_x * tiled.width / sprite.width.max(1.0));
    let scale_y = snap(sign_y * tiled.height / sprite.height.max(1.0));
    let rotation = snap(normalize_degrees(-tiled.rotation));

    let (left, top) = box_corner_offset(scale_x, scale_y, sprite);
    let (offset_x, offset_y) = rotate_counter_clockwise(left, top, rotation);
    GmPlacement { x: snap(tiled.x - offset_x), y: snap(tiled.y - offset_y), scale_x, scale_y, rotation }
}

/// Where the top-left corner of the scaled, unrotated image is, relative to the origin.
fn box_corner_offset(scale_x: f64, scale_y: f64, sprite: SpriteFrame) -> (f64, f64) {
    let left = (-sprite.origin_x * scale_x).min((sprite.width - sprite.origin_x) * scale_x);
    let top = (-sprite.origin_y * scale_y).min((sprite.height - sprite.origin_y) * scale_y);
    (left, top)
}

/// Screen coordinates have y pointing down, so counter-clockwise on screen
/// turns (1, 0) towards (0, -1).
fn rotate_counter_clockwise(x: f64, y: f64, degrees: f64) -> (f64, f64) {
    if degrees == 0.0 {
        return (x, y);
    }
    let (sin, cos) = degrees.to_radians().sin_cos();
    (x * cos + y * sin, -x * sin + y * cos)
}

fn normalize_degrees(degrees: f64) -> f64 {
    let wrapped = degrees.rem_euclid(360.0);
    if wrapped == 360.0 {
        0.0
    } else {
        wrapped
    }
}

/// Undoes tiny floating point drift from sin/cos and division, so an object
/// at y = 676 does not come back as y = 675.9999999 and land one pixel off
/// after floor(). GameMaker room values are multiples of 1/1024 in practice.
fn snap(value: f64) -> f64 {
    const STEP: f64 = 1024.0;
    let snapped = (value * STEP).round() / STEP;
    if (value - snapped).abs() < 1e-6 {
        snapped
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPRITE: SpriteFrame = SpriteFrame { width: 32.0, height: 48.0, origin_x: 22.0, origin_y: 14.0 };

    fn round_trip(gm: GmPlacement) {
        let back = tiled_to_gm(gm_to_tiled(gm, SPRITE), SPRITE);
        assert_eq!(back, gm, "round trip changed the placement");
    }

    #[test]
    fn plain_placement_survives_round_trip() {
        round_trip(GmPlacement { x: 676.0, y: -4.0, scale_x: 2.90625, scale_y: 1.34375, rotation: 0.0 });
    }

    #[test]
    fn mirrored_and_rotated_placement_survives_round_trip() {
        round_trip(GmPlacement { x: 100.0, y: 200.0, scale_x: -1.0, scale_y: 1.0, rotation: 90.0 });
        round_trip(GmPlacement { x: 3920.0, y: 513.0, scale_x: 1.5, scale_y: -2.0, rotation: 45.0 });
    }

    #[test]
    fn origin_zero_unrotated_is_the_same_point() {
        let sprite = SpriteFrame { width: 16.0, height: 16.0, origin_x: 0.0, origin_y: 0.0 };
        let tiled = gm_to_tiled(GmPlacement { x: 8.0, y: 12.0, scale_x: 1.0, scale_y: 1.0, rotation: 0.0 }, sprite);
        assert_eq!((tiled.x, tiled.y), (8.0, 12.0));
    }
}
