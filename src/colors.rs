pub const COLOR_RED: &str    = "\\";
pub const COLOR_CYAN: &str   = "@";
pub const COLOR_PURPUR: &str = "&";
pub const COLOR_BLUE: &str   = "/";
pub const COLOR_GRAY: &str   = "|";
pub const COLOR_YELLOW: &str = "`";
pub const COLOR_ORANGE: &str = "№";
pub const COLOR_RESET: &str  = "~";

pub fn strip(s: &str) -> String {
    s.chars()
        .filter(|&c| !matches!(c, '\\' | '@' | '&' | '/' | '|' | '`' | '№' | '~'))
        .collect()
}

pub fn colorize(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 32);
    let mut colored = false;
    for ch in s.chars() {
        match ch {
            '\\' => { out.push_str("\x1B[31m");         colored = true; }
            '@'  => { out.push_str("\x1B[32m");         colored = true; }
            '&'  => { out.push_str("\x1B[35m");         colored = true; }
            '/'  => { out.push_str("\x1B[94m");         colored = true; }
            '|'  => { out.push_str("\x1B[90m");         colored = true; }
            '`'  => { out.push_str("\x1B[33m");         colored = true; }
            '№'        => { out.push_str("\x1B[38;5;208m"); colored = true; }
            '~'  => { out.push_str("\x1B[0m");          colored = false; }
            c    => out.push(c),
        }
    }
    if colored {
        out.push_str("\x1B[0m");
    }
    out
}
