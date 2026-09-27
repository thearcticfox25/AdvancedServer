//! The Singleplayer map page, looking as the map vote does (obj_lobby): the maps' vote
//! cards, the chosen map's pictures drifting behind them, the vote's bars. All the maps do
//! not fit on the screen, so the cards scroll as the map list of the legacy server window
//! did (as_legacy_ui MapList.c): the wheel moves the list and it glides after it. A card
//! chosen with the keys is scrolled into sight.
//!
//! Where each card stands is its button's place in the menu map, as for every button;
//! this only scrolls, cuts the cards to the space between the bars and draws the rest.

use crate::client::canvas::{make_color_rgb, Canvas, C_DKGRAY, VIEW_WIDTH};
use crate::client::input::Input;
use crate::client::screens::map_preview::MapPreview;
use crate::core::resources::names::sprite;
use crate::core::world::C_WHITE;

/// spr_vote_bars: its top bar ends above row 25, its bottom bar starts at row 223; the
/// cards are seen between the two.
const WINDOW_TOP: f64 = 28.0;
const WINDOW_BOTTOM: f64 = 223.0;
/// The top bar is painted over in its own colour to hide the words "vote for the map",
/// and the chosen map's name is written there instead.
const TITLE_BAR_HEIGHT: f64 = 25.0;
const TITLE_BAR_COLOUR: u32 = make_color_rgb(0x03, 0x00, 0x09);
const TITLE_TOP: f64 = 8.0;
/// The list stops with its first and last row this far inside the window.
const WINDOW_MARGIN: f64 = 6.0;
/// MapList.c: 45 px a notch of the wheel, and the list covers 0.30 of the way a step.
const WHEEL_STEP: f64 = 45.0;
const SCROLL_SMOOTHING: f64 = 0.30;

pub struct MapGrid {
    /// How far the cards are moved up from their places in the map (negative) or down.
    scroll: f64,
    target_scroll: f64,
    /// The card last scrolled into sight for the keys, by widget, so the wheel is free
    /// to move away from it until another card is chosen.
    followed: Option<usize>,
    preview: MapPreview,
}

/// What the page is made of this step, as the menu knows it.
pub struct Cards {
    /// The top of the highest card and the bottom of the lowest one, unscrolled.
    pub span: (f64, f64),
    /// The chosen card: its widget and its top, unscrolled.
    pub chosen: Option<(usize, f64)>,
}

impl MapGrid {
    pub fn new() -> MapGrid {
        MapGrid { scroll: 0.0, target_scroll: 0.0, followed: None, preview: MapPreview::new() }
    }

    /// The height of a card: a frame of spr_mapvote.
    pub fn card_height(canvas: &Canvas) -> f64 {
        canvas.sprites.get(sprite::SPR_MAPVOTE).height as f64
    }

    /// One step: the wheel, the chosen card kept in sight, the glide.
    pub fn step(&mut self, canvas: &Canvas, input: &Input, cards: &Cards) {
        if input.wheel_up() {
            self.target_scroll += WHEEL_STEP;
        } else if input.wheel_down() {
            self.target_scroll -= WHEEL_STEP;
        }
        if let Some((widget, top)) = cards.chosen.filter(|(widget, _)| self.followed != Some(*widget)) {
            self.followed = Some(widget);
            self.scroll_into_sight(top, top + Self::card_height(canvas));
        }
        let (first_top, last_bottom) = cards.span;
        let start = WINDOW_TOP + WINDOW_MARGIN - first_top;
        let end = (WINDOW_BOTTOM - WINDOW_MARGIN - last_bottom).min(start);
        self.target_scroll = self.target_scroll.clamp(end, start);
        self.scroll += (self.target_scroll - self.scroll) * crate::core::config::eased_share(SCROLL_SMOOTHING);
    }

    /// Moves the list just enough for a card to be seen whole.
    fn scroll_into_sight(&mut self, top: f64, bottom: f64) {
        let above = WINDOW_TOP + WINDOW_MARGIN - (top + self.target_scroll);
        let below = (bottom + self.target_scroll) - (WINDOW_BOTTOM - WINDOW_MARGIN);
        if above > 0.0 {
            self.target_scroll += above;
        } else if below > 0.0 {
            self.target_scroll -= below;
        }
    }

    /// The vote's picture of the chosen map, drawn where the menu's background would be.
    pub fn draw_preview(&mut self, canvas: &mut Canvas, map: usize, current_time_ms: f64) {
        self.preview.visible = true;
        self.preview.blend = C_DKGRAY;
        if self.preview.map() != map {
            self.preview.set_zone(map);
        }
        self.preview.draw(canvas, current_time_ms);
    }

    /// The vote's bars, with the chosen map's name at the top, drawn where the menu's
    /// own bars would be. With no map chosen (Back) the bars stay and the name is empty.
    pub fn draw_bars(&self, canvas: &mut Canvas, name: Option<&str>) {
        canvas.draw_sprite(sprite::SPR_VOTE_BARS, 0.0, 0.0, 0.0);
        canvas.fill_rectangle(0.0, 0.0, VIEW_WIDTH, TITLE_BAR_HEIGHT, TITLE_BAR_COLOUR, 1.0);
        if let Some(name) = name {
            let left = ((VIEW_WIDTH - canvas.menu_text_width(name)) / 2.0).floor();
            canvas.draw_menu_text(left, TITLE_TOP, name, C_WHITE, 1.0);
        }
    }

    /// A map's card at its place, scrolled and cut to the window; dark unless chosen,
    /// as the vote draws the cards not under its cursor.
    pub fn draw_card(&self, canvas: &mut Canvas, map: usize, left: f64, top: f64, chosen: bool) {
        let Some((part_top, part_height, screen_top)) = self.visible_part(canvas, top) else { return };
        let width = canvas.sprites.get(sprite::SPR_MAPVOTE).width as f64;
        let blend = if chosen { C_WHITE } else { C_DKGRAY };
        canvas.draw_sprite_part_ext(sprite::SPR_MAPVOTE, map as f64, [0.0, part_top, width, part_height], left, screen_top, 1.0, 1.0, blend, 1.0);
    }

    /// Whether (x, y) is on the seen part of the card placed at (left, top).
    pub fn card_contains(&self, canvas: &Canvas, left: f64, top: f64, x: f64, y: f64) -> bool {
        let Some((_, part_height, screen_top)) = self.visible_part(canvas, top) else { return false };
        let width = canvas.sprites.get(sprite::SPR_MAPVOTE).width as f64;
        (left..left + width).contains(&x) && (screen_top..screen_top + part_height).contains(&y)
    }

    /// The rows of a card that are inside the window: where they start in the card,
    /// how many there are, and where they are on the screen.
    fn visible_part(&self, canvas: &Canvas, top: f64) -> Option<(f64, f64, f64)> {
        let screen_top = top + self.scroll;
        let screen_bottom = screen_top + Self::card_height(canvas);
        let (shown_top, shown_bottom) = (screen_top.max(WINDOW_TOP), screen_bottom.min(WINDOW_BOTTOM));
        (shown_bottom > shown_top).then_some((shown_top - screen_top, shown_bottom - shown_top, shown_top))
    }
}
