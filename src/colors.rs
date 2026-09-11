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
#[allow(dead_code)] // part of the palette; no server message uses orange yet
pub const COLOR_ORANGE: &str = "№";
pub const COLOR_RESET: &str  = "~";

/// Every colour marker and the ANSI escape it turns into on the server console.
/// COLOR_RESET is handled separately: it ends a colour rather than starting one.
const COLOR_ESCAPES: &[(char, &str)] = &[
    ('\\',       "\x1B[31m"),
    ('@',        "\x1B[32m"),
    ('&',        "\x1B[35m"),
    ('/',        "\x1B[94m"),
    ('|',        "\x1B[90m"),
    ('`',        "\x1B[33m"),
    ('№',        "\x1B[38;5;208m"),
];

const RESET_MARKER: char = '~';
const RESET_ESCAPE: &str = "\x1B[0m";

fn escape_for(marker: char) -> Option<&'static str> {
    COLOR_ESCAPES.iter()
        .find(|&&(candidate, _)| candidate == marker)
        .map(|&(_, escape)| escape)
}

/// Removes every colour marker, leaving the plain text. Used wherever two names
/// have to be compared as the players actually read them.
pub fn strip(text: &str) -> String {
    text.chars()
        .filter(|&ch| ch != RESET_MARKER && escape_for(ch).is_none())
        .collect()
}

/// Turns the markers into real ANSI colours for the server console, and closes
/// any colour still open at the end of the string.
pub fn colorize(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 32);
    let mut colored = false;

    for ch in text.chars() {
        if ch == RESET_MARKER {
            out.push_str(RESET_ESCAPE);
            colored = false;
        } else if let Some(escape) = escape_for(ch) {
            out.push_str(escape);
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
