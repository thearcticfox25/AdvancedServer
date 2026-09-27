// The game's chat protocol carries colour as single marker characters inside the
// message text. One table below is the only place that pairing lives, so adding a
// colour means adding one row and nothing else.

// A complete palette: every marker the client understands has a name here, even
// the ones no server message happens to use yet.
pub const COLOR_RED: &str    = "\\";
pub const COLOR_CYAN: &str   = "@";
pub const COLOR_PURPUR: &str = "&";
pub const COLOR_BLUE: &str   = "/";
pub const COLOR_GRAY: &str   = "|";
pub const COLOR_YELLOW: &str = "`";
pub const COLOR_ORANGE: &str = "№";
pub const COLOR_RESET: &str  = "~";

/// Every colour marker and the colour the game draws the letters after it in
/// (client/text.rs). The server console shows the same colour. COLOR_RESET is not
/// here: it ends a colour rather than starting one. Each marker is one character,
/// so `starts_with` below compares it whole.
const MARKER_COLORS: &[(&str, (u8, u8, u8))] = &[
    (COLOR_RED,    (0xC2, 0x00, 0x37)),
    (COLOR_CYAN,   (0x0F, 0xFF, 0x39)),
    (COLOR_PURPUR, (0xB8, 0x24, 0xFF)),
    (COLOR_BLUE,   (0x5D, 0x67, 0xFF)),
    (COLOR_GRAY,   (0x64, 0x64, 0x64)),
    (COLOR_YELLOW, (0xFF, 0xDB, 0x00)),
    (COLOR_ORANGE, (0xEA, 0x60, 0x14)),
];

const RESET_ESCAPE: &str = "\x1B[0m";

pub fn is_reset(ch: char) -> bool {
    COLOR_RESET.starts_with(ch)
}

/// The colour `marker` switches to as (red, green, blue), or None for any other character.
pub fn marker_color(marker: char) -> Option<(u8, u8, u8)> {
    MARKER_COLORS.iter()
        .find(|&&(color, _)| color.starts_with(marker))
        .map(|&(_, rgb)| rgb)
}

/// Whether `ch` is a colour marker or the reset: shown as nothing but a change of colour.
pub fn is_marker(ch: char) -> bool {
    is_reset(ch) || marker_color(ch).is_some()
}

/// Removes every colour marker, leaving the plain text. Used wherever two names
/// have to be compared as the players actually read them.
pub fn strip(text: &str) -> String {
    text.chars()
        .filter(|&ch| !is_marker(ch))
        .collect()
}

/// Turns the markers into the game's colours on the server console (24-bit ANSI), and closes
/// any colour still open at the end of the string.
pub fn colorize(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 32);
    let mut colored = false;

    for ch in text.chars() {
        if is_reset(ch) {
            out.push_str(RESET_ESCAPE);
            colored = false;
        } else if let Some((red, green, blue)) = marker_color(ch) {
            out.push_str(&format!("\x1B[38;2;{red};{green};{blue}m"));
            colored = true;
        } else {
            out.push(ch);
        }
    }

    if colored {
        out.push_str(RESET_ESCAPE);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_become_the_games_colours_on_the_console() {
        assert_eq!(strip("\\red~ №orange"), "red orange");
        assert_eq!(colorize("\\red~ x"), "\x1B[38;2;194;0;55mred\x1B[0m x");
        assert_eq!(colorize("№open"), "\x1B[38;2;234;96;20mopen\x1B[0m");
        assert!(is_marker('~') && is_marker('№') && !is_marker('a'));
    }
}
