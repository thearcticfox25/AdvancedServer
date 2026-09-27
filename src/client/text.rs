//! scr_text_spr: text drawn with the chat lettering (Fonts/letter.png, the original's
//! spr_letter1). Some characters are not drawn but switch the colour of the letters
//! after them, so the text is laid out letter by letter here instead of by the font.

use crate::client::canvas::{make_color_rgb, Canvas, TextFont};
use crate::client::font::{Font, Glyph};
use crate::core::resources::names::sprite;
use crate::core::world::C_WHITE;

const LINE_HEIGHT: f64 = 8.0;
/// Draws `text` with its top-left corner at (x, y) and returns the width of its
/// widest line. A `color` other than white overrides the colour codes.
pub fn draw_text(canvas: &mut Canvas, x: f64, y: f64, text: &str, color: u32, alpha: f64) -> f64 {
    let (letters, width) = layout(canvas.font(TextFont::Chat), x, y, text, color);
    for letter in letters {
        canvas.draw_font_glyph(TextFont::Chat, &letter.glyph, letter.x, letter.y, letter.color, alpha);
    }
    width
}

/// The width `draw_text` would return (the original measured by drawing off screen).
pub fn text_width(canvas: &Canvas, text: &str) -> f64 {
    layout(canvas.font(TextFont::Chat), 0.0, 0.0, text, C_WHITE).1
}

/// scr_counter_draw: one spr_counter digit frame.
pub fn draw_counter(canvas: &mut Canvas, number: i32, x: f64, y: f64) {
    canvas.draw_sprite(sprite::SPR_COUNTER, number as f64, x, y);
}

/// scr_counter_draw_m: the digits of `number`, padded with leading zeros to `width` digits.
pub fn draw_counter_padded(canvas: &mut Canvas, number: i32, x: f64, y: f64, width: usize) {
    let digits = format!("{number:0width$}");
    for (index, digit) in digits.chars().enumerate() {
        let frame = digit.to_digit(10).unwrap_or(0) as f64;
        canvas.draw_sprite(sprite::SPR_COUNTER, frame, x + COUNTER_DIGIT_WIDTH * index as f64, y);
    }
}

const COUNTER_DIGIT_WIDTH: f64 = 11.0;

struct Letter {
    glyph: Glyph,
    x: f64,
    y: f64,
    color: u32,
}

fn layout(font: &Font, x: f64, y: f64, text: &str, color: u32) -> (Vec<Letter>, f64) {
    let mut letters = Vec::new();
    let (mut pen_x, mut pen_y) = (x, y);
    let mut code_color = color;
    let mut widest_line = 0.0;

    for character in text.chars() {
        if character == '\n' {
            pen_x = x;
            pen_y += LINE_HEIGHT;
            continue;
        }
        // Spaces and colour codes do not count towards the width.
        if character == ' ' {
            pen_x += font.glyph(' ').map_or(0.0, |space| space.shift as f64);
            continue;
        }
        if let Some(switched) = color_code(character, color) {
            code_color = switched;
            continue;
        }

        // A letter the lettering has no sign of is written as its question mark (font.rs).
        let Some(glyph) = font.drawn_glyph(character).copied() else { continue };
        let letter_color = if color != C_WHITE { color } else { code_color };
        letters.push(Letter { glyph, x: pen_x, y: pen_y, color: letter_color });
        pen_x += glyph.shift as f64;
        widest_line = f64::max(widest_line, pen_x - x);
    }
    (letters, widest_line)
}

/// `~` goes back to the colour the caller asked for; the others are in crate::colors.
fn color_code(character: char, caller_color: u32) -> Option<u32> {
    if crate::colors::is_reset(character) {
        return Some(caller_color);
    }
    crate::colors::marker_color(character).map(|(red, green, blue)| make_color_rgb(red, green, blue))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn lettering() -> Font {
        Font::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("Resources/Fonts/letter.png")).unwrap()
    }

    /// The widths the sprite version had: 6 px a letter, 8 for the wide ones, 5 for a space.
    #[test]
    fn the_lettering_keeps_the_widths_the_frames_were_drawn_with() {
        let font = lettering();
        assert_eq!(font.glyph('a').unwrap().shift, 6);
        assert_eq!(font.glyph('m').unwrap().shift, 8);
        assert_eq!(font.glyph('ж').unwrap().shift, 8);
        assert_eq!(font.glyph(' ').unwrap().shift, 5);
        // Upper case is the same lettering, and the version's alpha is in there now.
        assert_eq!(font.glyph('A').map(|glyph| glyph.x), font.glyph('a').map(|glyph| glyph.x));
        assert!(font.glyph('α').is_some());
    }

    /// A letter the sheet draws no sign of is written as the question mark, and a
    /// letter it does draw is written as itself whichever case it is typed in.
    #[test]
    fn a_letter_the_lettering_lacks_is_written_as_a_question_mark() {
        let font = lettering();
        assert!(font.glyph('\u{4e2d}').is_none(), "the sheet has no chinese");
        let (letters, _) = layout(&font, 0.0, 0.0, "\u{4e2d}", C_WHITE);
        assert_eq!(letters[0].glyph.x, font.glyph('?').unwrap().x, "written as a question mark");

        let (upper, _) = layout(&font, 0.0, 0.0, "ЩУКА", C_WHITE);
        let (lower, _) = layout(&font, 0.0, 0.0, "щука", C_WHITE);
        let boxes: Vec<i32> = upper.iter().map(|letter| letter.glyph.x).collect();
        assert_eq!(boxes, lower.iter().map(|letter| letter.glyph.x).collect::<Vec<i32>>(), "the same letters either way");
        assert!(!boxes.contains(&font.glyph('?').unwrap().x), "and none of them a question mark: {boxes:?}");
    }

    #[test]
    fn width_skips_spaces_and_colour_codes_and_counts_wide_letters() {
        let font = lettering();
        let width = |text: &str| layout(&font, 0.0, 0.0, text, C_WHITE).1;
        assert_eq!(width("ab"), 12.0);
        assert_eq!(width("@a b~"), 17.0);
        assert_eq!(width("mw"), 16.0);
        assert_eq!(width("abc\nd"), 18.0);
        // A trailing space moves the pen but not the measured width.
        assert_eq!(width("a "), 6.0);
    }

    #[test]
    fn caller_colour_overrides_codes() {
        let font = lettering();
        let red = make_color_rgb(255, 0, 0);
        let (letters, _) = layout(&font, 0.0, 0.0, "@a", red);
        assert_eq!(letters[0].color, red);
        let (letters, _) = layout(&font, 0.0, 0.0, "@a~b", C_WHITE);
        assert_eq!(letters[0].color, make_color_rgb(0x0F, 0xFF, 0x39));
        assert_eq!(letters[1].color, C_WHITE);
    }
}
