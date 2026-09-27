//! obj_lobby_prolet: the slowly scrolling pictures of a map behind the vote, the
//! round intro and the results.

use crate::client::canvas::Canvas;
use macroquad::rand;
use crate::core::resources::names::sprite::*;
use crate::core::resources::SpriteId;
use crate::core::world::C_WHITE;
use crate::core::config::step;

/// mapData: the pictures of every map, by level number. `vertical` pictures scroll up instead of left.
struct MapPictures {
    tiles: &'static [SpriteId],
    background: SpriteId,
    vertical: &'static [bool],
}

const MAP_PICTURES: [MapPictures; 21] = [
    MapPictures { tiles: &[SPR_PR_HST, SPR_PR_HST2, SPR_PR_HST3], background: SPR_PR_HSTBG, vertical: &[true, false, false] },
    MapPictures { tiles: &[SPR_PR_RM, SPR_PR_RM2], background: SPR_PR_RMBG, vertical: &[false, false] },
    MapPictures { tiles: &[SPR_PR_DOT, SPR_PR_DOT2, SPR_PR_DOT3], background: BACKGROUND_DOTDOTDOT, vertical: &[false, false, false] },
    MapPictures { tiles: &[SPR_PR_DT, SPR_PR_DT2, SPR_PR_DT3], background: SPR_PR_DTBG, vertical: &[false, true, false] },
    MapPictures { tiles: &[SPR_PR_YCR, SPR_PR_YCR2, SPR_PR_YCR3], background: SPR_PR_YCRBG, vertical: &[false, false, true] },
    MapPictures { tiles: &[SPR_PR_LC, SPR_PR_LC2, SPR_PR_LC3], background: SPR_PR_LCBG, vertical: &[false, false, false] },
    MapPictures { tiles: &[SPR_PR_NP, SPR_PR_NP2, SPR_PR_NP3, SPR_PR_NP4], background: SPR_PR_NPBG, vertical: &[false, true, false, true] },
    MapPictures { tiles: &[SPR_PR_KAF, SPR_PR_KAF2, SPR_PR_KAF3], background: SPR_PR_KAFBG, vertical: &[false, false, false] },
    MapPictures { tiles: &[SPR_PR_ACT9, SPR_PR_ACT92, SPR_PR_ACT93], background: BACKGROUND_ACT9, vertical: &[false, true, false] },
    MapPictures { tiles: &[SPR_PR_NAP, SPR_PR_NAP2, SPR_PR_NAP3], background: SPR_PR_NAPBG, vertical: &[false, false, false] },
    MapPictures { tiles: &[SPR_PR_PF, SPR_PR_PF2, SPR_PR_PF3], background: BACKGROUND_PF, vertical: &[false, false, false] },
    MapPictures { tiles: &[SPR_PR_VV, SPR_PR_VV2, SPR_PR_VV3], background: SPR_PR_VVBG, vertical: &[false, false, false] },
    MapPictures { tiles: &[SPR_PR_GHZ, SPR_PR_GHZ2, SPR_PR_GHZ3], background: SPR_PR_GHZBG, vertical: &[false, false, false] },
    MapPictures { tiles: &[SPR_PR_MF, SPR_PR_MF2, SPR_PR_MF3], background: SPR_PR_MFBG, vertical: &[false, false, false] },
    MapPictures { tiles: &[SPR_PR_HS, SPR_PR_HS2, SPR_PR_HS3], background: SPR_PR_HSBG, vertical: &[false, false, false] },
    MapPictures { tiles: &[SPR_PR_TC, SPR_PR_TC2, SPR_PR_TC3], background: SPR_PR_TCBG, vertical: &[false, false, false] },
    MapPictures { tiles: &[SPR_PR_DARK, SPR_PR_DARK2, SPR_PR_DARK3], background: BACKGROUND_DARKTOWER, vertical: &[false, false, false] },
    MapPictures { tiles: &[SPR_PR_HD, SPR_PR_HD2, SPR_PR_HD3], background: SPR_PR_HDBG, vertical: &[false, false, false] },
    MapPictures { tiles: &[SPR_PR_WD, SPR_PR_WD2, SPR_PR_WD3], background: SPR_PR_WDBG, vertical: &[false, false, true] },
    MapPictures { tiles: &[SPR_PR_MA, SPR_PR_MA2, SPR_PR_MA3], background: SPR_PR_MABG, vertical: &[false, false, false] },
    MapPictures { tiles: &[SPR_PR_HD, SPR_PR_HD2, SPR_PR_HD3], background: BACKGROUND_TEST, vertical: &[false, false, false] },
];
/// MAP_FART has no tile pictures of its own, only the background.
const MAP_WITHOUT_TILES: usize = 20;
/// Kinda Fair scrolls slower.
const SLOW_SCROLL_MAP: usize = 7;
const SCROLL_PER_STEP: f64 = 0.64 + 0.16;
const SLOW_MAP_SCROLL_REDUCTION: f64 = 0.38;
/// Fading in over the first pixels of a picture and out before its end.
const FADE_IN_UNTIL: f64 = 56.0;
const FADE_DISTANCE: f64 = 32.0;
const FADE_OUT_MARGIN: f64 = 48.0;
const VIEW_WIDTH: f64 = 480.0;
const VIEW_HEIGHT: f64 = 270.0;
/// The background moves ten times slower than the picture and is less opaque.
const BACKGROUND_PARALLAX: f64 = 10.0;
const BACKGROUND_ALPHA_BELOW_PICTURE: f64 = 0.5;
const BACKGROUND_FRAME_MILLISECONDS: f64 = 100.0;
/// The map shown before the server names one.
const FIRST_MAP: usize = 6;

pub struct MapPreview {
    pub visible: bool,
    pub blend: u32,
    /// blackwhite: drawn without colours (the results of a round EXE won).
    pub black_white: bool,
    alpha: f64,
    map: usize,
    picture: usize,
    scroll: f64,
    vertical: bool,
}

impl MapPreview {
    pub fn new() -> MapPreview {
        // The room's creation code hides it until the vote.
        MapPreview { visible: false, blend: C_WHITE, black_white: false, alpha: 1.0, map: FIRST_MAP, picture: 0, scroll: 0.0, vertical: false }
    }

    pub fn map(&self) -> usize {
        self.map
    }

    /// setZone: a random picture of `map` from its start.
    pub fn set_zone(&mut self, map: usize) {
        let Some(pictures) = MAP_PICTURES.get(map) else { return };
        self.picture = rand::gen_range(0, pictures.tiles.len());
        self.scroll = 0.0;
        self.map = map;
        self.vertical = pictures.vertical[self.picture];
    }

    /// Draw_0: scrolls, fades and draws.
    pub fn draw(&mut self, canvas: &mut Canvas, current_time_ms: f64) {
        if !self.visible {
            return;
        }
        if self.black_white {
            canvas.set_black_white();
        }
        let pictures = &MAP_PICTURES[self.map];
        let tile = pictures.tiles[self.picture];
        let tile_meta = canvas.sprites.get(tile);
        let length = if self.vertical { tile_meta.height } else { tile_meta.width } as f64;
        self.scroll_one_step(length, pictures);

        let background_meta = canvas.sprites.get(pictures.background);
        let (background_width, background_height) = (background_meta.width as f64, background_meta.height as f64);
        let background_frame = current_time_ms / BACKGROUND_FRAME_MILLISECONDS;
        let background_offset = -self.scroll / BACKGROUND_PARALLAX;
        let mut column = 0.0;
        while column < VIEW_WIDTH / background_width + 2.0 {
            let mut row = 0.0;
            while row < VIEW_HEIGHT / background_height + 2.0 {
                let (x, y) = if self.vertical {
                    (column * background_width, background_offset + row * background_height)
                } else {
                    (background_offset + column * background_width, row * background_height)
                };
                canvas.draw_sprite_ext(pictures.background, background_frame, x, y, 1.0, 1.0, 0.0, self.blend, self.alpha - BACKGROUND_ALPHA_BELOW_PICTURE);
                row += 1.0;
            }
            column += 1.0;
        }

        if self.map != MAP_WITHOUT_TILES {
            let (x, y) = if self.vertical { (0.0, -self.scroll) } else { (-self.scroll, 0.0) };
            canvas.draw_sprite_ext(tile, 0.0, x, y, 1.0, 1.0, 0.0, self.blend, self.alpha);
        }
        if self.black_white {
            canvas.reset_shader();
        }
    }

    fn scroll_one_step(&mut self, length: f64, pictures: &MapPictures) {
        self.scroll += SCROLL_PER_STEP * step();
        if self.map == SLOW_SCROLL_MAP {
            self.scroll -= SLOW_MAP_SCROLL_REDUCTION * step();
        }
        if self.scroll <= FADE_IN_UNTIL {
            self.alpha = self.scroll / FADE_DISTANCE;
        }
        let view_length = if self.vertical { VIEW_HEIGHT } else { VIEW_WIDTH };
        let fade_out_start = length - (view_length + FADE_OUT_MARGIN);
        if self.scroll >= fade_out_start {
            self.alpha = 1.5 - (self.scroll - fade_out_start) / FADE_DISTANCE;
        }
        if self.scroll > length - view_length {
            self.scroll = 0.0;
            self.picture = (self.picture + 1) % pictures.tiles.len();
            self.vertical = pictures.vertical[self.picture];
        }
    }
}
