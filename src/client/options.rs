//! PersistentData/Game/Options.toml: window size, nickname, last IP, HUD counters
//! and key bindings (obj_config). Saved after every change, like the original.

use crate::client::input::{key_by_name, key_name};
use macroquad::input::KeyCode;
use macroquad::rand;
use serde::{Deserialize, Serialize};
use std::path::Path;

const OPTIONS_FILE: &str = "PersistentData/Game/Options.toml";
/// string_copy(nickname, 0, 15) in obj_config.
const NICKNAME_MAX_CHARACTERS: usize = 15;
pub const FULLSCREEN_MODE: u8 = 3;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Options {
    /// 0, 1, 2: a window 1, 2 or 3 times 480x270. 3: fullscreen.
    pub screen_mode: u8,
    pub nickname: String,
    pub ip: String,
    pub show_fps: bool,
    pub show_ping: bool,
    /// The lobby's preference cards: 0 for any, 1-based exe ... exeller or 255
    /// for "not me", and 1-based tails ... sally for the survivor.
    pub preferred_exe: u8,
    pub preferred_survivor: u8,
    pub keys: KeyBindings,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct KeyBindings {
    pub left: BoundKey,
    pub right: BoundKey,
    pub up: BoundKey,
    pub down: BoundKey,
    /// global.KeyA
    pub jump: BoundKey,
    /// global.KeyB
    pub special1: BoundKey,
    /// global.KeyC
    pub special2: BoundKey,
    pub emotion1: BoundKey,
    pub emotion2: BoundKey,
    pub emotion3: BoundKey,
    /// global.KeyIdle
    pub emotion4: BoundKey,
    pub hide_gui: BoundKey,
    pub player_list: BoundKey,
}

/// A key written by its name, as the controls menu shows it ("left", "z", "lctrl").
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(try_from = "String", into = "String")]
pub struct BoundKey(pub KeyCode);

impl TryFrom<String> for BoundKey {
    type Error = String;
    fn try_from(name: String) -> Result<BoundKey, String> {
        key_by_name(&name).map(BoundKey).ok_or_else(|| format!("unknown key name {name:?}"))
    }
}

impl From<BoundKey> for String {
    fn from(key: BoundKey) -> String {
        key_name(key.0).to_string()
    }
}

impl Default for Options {
    fn default() -> Options {
        Options {
            screen_mode: 0,
            nickname: format!("\\player /{}", rand::gen_range(0, 10000)),
            ip: "localhost".to_string(),
            show_fps: false,
            show_ping: true,
            preferred_exe: crate::packet::PREFERENCE_ANY,
            preferred_survivor: crate::packet::PREFERENCE_ANY,
            keys: KeyBindings::default(),
        }
    }
}

impl Default for KeyBindings {
    fn default() -> KeyBindings {
        KeyBindings {
            left: BoundKey(KeyCode::Left),
            right: BoundKey(KeyCode::Right),
            up: BoundKey(KeyCode::Up),
            down: BoundKey(KeyCode::Down),
            jump: BoundKey(KeyCode::Z),
            special1: BoundKey(KeyCode::X),
            special2: BoundKey(KeyCode::C),
            emotion1: BoundKey(KeyCode::A),
            emotion2: BoundKey(KeyCode::S),
            emotion3: BoundKey(KeyCode::D),
            emotion4: BoundKey(KeyCode::W),
            hide_gui: BoundKey(KeyCode::LeftControl),
            player_list: BoundKey(KeyCode::Tab),
        }
    }
}

impl KeyBindings {
    /// The binding a controls menu panel shows, by the panel's tid.
    pub fn by_panel_name(&mut self, panel: &str) -> Option<&mut BoundKey> {
        Some(match panel {
            "left" => &mut self.left,
            "right" => &mut self.right,
            "up" => &mut self.up,
            "down" => &mut self.down,
            "jump" => &mut self.jump,
            "special1" => &mut self.special1,
            "special2" => &mut self.special2,
            "emotion1" => &mut self.emotion1,
            "emotion2" => &mut self.emotion2,
            "emotion3" => &mut self.emotion3,
            "emotion4" => &mut self.emotion4,
            "hidegui" => &mut self.hide_gui,
            "playerlist" => &mut self.player_list,
            _ => return None,
        })
    }
}

impl Options {
    /// The saved options, or the defaults (written to disk) when there are none.
    /// A broken file is reported and left alone until the next change saves over it.
    pub fn load() -> Options {
        let text = match std::fs::read_to_string(OPTIONS_FILE) {
            Ok(text) => text,
            Err(_) => {
                let options = Options::default();
                options.save();
                return options;
            }
        };
        match toml::from_str::<Options>(&text) {
            Ok(mut options) => {
                options.nickname = validate_nickname(&options.nickname.chars().take(NICKNAME_MAX_CHARACTERS).collect::<String>());
                // Writes options added since the file was saved, and drops ones no longer kept.
                options.save();
                options
            }
            Err(error) => {
                eprintln!("{OPTIONS_FILE} is broken, using the defaults: {error}");
                Options::default()
            }
        }
    }

    pub fn save(&self) {
        let written = Path::new(OPTIONS_FILE)
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|_| std::fs::write(OPTIONS_FILE, toml::to_string(self).expect("options always serialize")));
        if let Err(error) = written {
            eprintln!("failed to save {OPTIONS_FILE}: {error}");
        }
    }
}

/// scr_nickname_validate: a nickname made only of colour codes and spaces would be
/// invisible, so it is replaced by a random one.
pub fn validate_nickname(nickname: &str) -> String {
    if nickname.chars().all(|character| character == ' ' || crate::colors::is_marker(character)) {
        return format!("/player~ \\{}", rand::gen_range(0, 10000));
    }
    nickname.to_string()
}

pub struct WindowSize {
    pub width: i32,
    pub height: i32,
    pub fullscreen: bool,
}

/// obj_config.applyRes
pub fn window_size(screen_mode: u8) -> WindowSize {
    let scale = screen_mode.min(FULLSCREEN_MODE - 1) as i32 + 1;
    WindowSize {
        width: crate::client::canvas::VIEW_WIDTH as i32 * scale,
        height: crate::client::canvas::VIEW_HEIGHT as i32 * scale,
        fullscreen: screen_mode >= FULLSCREEN_MODE,
    }
}

/// Resizes the running window to the screen mode.
pub fn apply_screen_mode(screen_mode: u8) {
    let size = window_size(screen_mode);
    macroquad::window::set_fullscreen(size.fullscreen);
    if !size.fullscreen {
        macroquad::window::request_new_screen_size(size.width as f32, size.height as f32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_survive_a_save_and_load() {
        let mut options = Options::default();
        options.keys.hide_gui = BoundKey(KeyCode::Kp5);
        let text = toml::to_string(&options).unwrap();
        assert!(text.contains("hide_gui = \"num 5\""));
        let loaded: Options = toml::from_str(&text).unwrap();
        assert_eq!(loaded.keys.hide_gui, BoundKey(KeyCode::Kp5));
    }

    #[test]
    fn invisible_nicknames_are_replaced() {
        assert_eq!(validate_nickname("\\fox"), "\\fox");
        assert!(validate_nickname("@ ~").starts_with("/player~ \\"));
    }
}
