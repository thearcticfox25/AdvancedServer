//! Keyboard and mouse the way GameMaker events see them. A press counts for
//! exactly one tick, whether several frames pass between two ticks or several
//! ticks run in one frame.

use macroquad::input::{self as mq, KeyCode, MouseButton};
use std::collections::HashSet;

/// keyboard_string never grows past this many characters in GameMaker.
const KEYBOARD_STRING_LIMIT: usize = 1024;
const BACKSPACE: char = '\u{8}';

#[derive(Default)]
pub struct Input {
    held: HashSet<KeyCode>,
    pressed: HashSet<KeyCode>,
    /// Keys that went down during frames since the last tick.
    pending_presses: HashSet<KeyCode>,
    /// keyboard_string: typed characters, backspace removes the last one. Screens may overwrite it.
    pub keyboard_string: String,
    /// keyboard_lastkey
    pub last_key: Option<KeyCode>,
    pub mouse_x: f64,
    pub mouse_y: f64,
    mouse_left_pressed: bool,
    pending_mouse_left: bool,
    mouse_any_pressed: bool,
    pending_mouse_any: bool,
    mouse_any_held: bool,
    wheel_up: bool,
    pending_wheel_up: bool,
    wheel_down: bool,
    pending_wheel_down: bool,
}

impl Input {
    /// Every frame: remembers what happened until the next tick reads it.
    pub fn collect_frame_events(&mut self) {
        let presses = mq::get_keys_pressed();
        if let Some(&key) = presses.iter().next() {
            self.last_key = Some(key);
        }
        self.pending_presses.extend(presses);

        while let Some(character) = mq::get_char_pressed() {
            self.type_character(character);
        }

        let buttons = [MouseButton::Left, MouseButton::Right, MouseButton::Middle];
        self.pending_mouse_left |= mq::is_mouse_button_pressed(MouseButton::Left);
        self.pending_mouse_any |= buttons.into_iter().any(mq::is_mouse_button_pressed);
        let (_, wheel) = mq::mouse_wheel();
        self.pending_wheel_up |= wheel > 0.0;
        self.pending_wheel_down |= wheel < 0.0;
    }

    /// At the start of a tick: the presses collected so far become this tick's presses.
    pub fn begin_tick(&mut self, mouse_position: (f64, f64)) {
        self.held = mq::get_keys_down();
        self.pressed = std::mem::take(&mut self.pending_presses);
        self.mouse_left_pressed = std::mem::take(&mut self.pending_mouse_left);
        self.mouse_any_pressed = std::mem::take(&mut self.pending_mouse_any);
        self.mouse_any_held = [MouseButton::Left, MouseButton::Right, MouseButton::Middle].into_iter().any(mq::is_mouse_button_down);
        self.wheel_up = std::mem::take(&mut self.pending_wheel_up);
        self.wheel_down = std::mem::take(&mut self.pending_wheel_down);
        (self.mouse_x, self.mouse_y) = mouse_position;
    }

    /// keyboard_check
    pub fn held(&self, key: KeyCode) -> bool {
        self.held.contains(&key)
    }

    /// keyboard_check_pressed
    pub fn pressed(&self, key: KeyCode) -> bool {
        self.pressed.contains(&key)
    }

    /// keyboard_check_pressed(vk_anykey)
    pub fn any_key_pressed(&self) -> bool {
        !self.pressed.is_empty()
    }

    /// keyboard_check(vk_control): either control key.
    pub fn control_held(&self) -> bool {
        self.held(KeyCode::LeftControl) || self.held(KeyCode::RightControl)
    }

    /// mouse_check_button_pressed(mb_left)
    pub fn mouse_left_pressed(&self) -> bool {
        self.mouse_left_pressed
    }

    /// mouse_check_button_pressed(mb_any)
    pub fn mouse_any_pressed(&self) -> bool {
        self.mouse_any_pressed
    }

    /// mouse_check_button(mb_any)
    pub fn mouse_any_held(&self) -> bool {
        self.mouse_any_held
    }

    /// The mouse wheel up event (ev_mouse_wheel_up).
    pub fn wheel_up(&self) -> bool {
        self.wheel_up
    }

    /// The mouse wheel down event (ev_mouse_wheel_down).
    pub fn wheel_down(&self) -> bool {
        self.wheel_down
    }


    fn type_character(&mut self, character: char) {
        if character == BACKSPACE {
            self.keyboard_string.pop();
            return;
        }
        if character.is_control() || self.keyboard_string.chars().count() >= KEYBOARD_STRING_LIMIT {
            return;
        }
        self.keyboard_string.push(character);
    }
}

/// Key names shown in the controls menu and written to Options.toml
/// (scr_controls_keycode_to_key).
const KEY_NAMES: &[(KeyCode, &str)] = &[
    (KeyCode::Backspace, "backspace"),
    (KeyCode::Tab, "tab"),
    (KeyCode::Enter, "enter"),
    (KeyCode::Space, "space"),
    (KeyCode::LeftShift, "lshift"),
    (KeyCode::LeftControl, "lctrl"),
    (KeyCode::LeftAlt, "lalt"),
    (KeyCode::RightShift, "rshift"),
    (KeyCode::RightControl, "rctrl"),
    (KeyCode::RightAlt, "ralt"),
    (KeyCode::Pause, "pause"),
    (KeyCode::CapsLock, "capslock"),
    (KeyCode::Escape, "esc"),
    (KeyCode::PageDown, "pdown"),
    (KeyCode::PageUp, "pup"),
    (KeyCode::End, "end"),
    (KeyCode::Home, "home"),
    (KeyCode::Left, "left"),
    (KeyCode::Up, "up"),
    (KeyCode::Right, "right"),
    (KeyCode::Down, "down"),
    (KeyCode::Insert, "insert"),
    (KeyCode::Delete, "delete"),
    (KeyCode::Kp0, "num 0"),
    (KeyCode::Kp1, "num 1"),
    (KeyCode::Kp2, "num 2"),
    (KeyCode::Kp3, "num 3"),
    (KeyCode::Kp4, "num 4"),
    (KeyCode::Kp5, "num 5"),
    (KeyCode::Kp6, "num 6"),
    (KeyCode::Kp7, "num 7"),
    (KeyCode::Kp8, "num 8"),
    (KeyCode::Kp9, "num 9"),
    (KeyCode::F1, "f1"),
    (KeyCode::F2, "f2"),
    (KeyCode::F3, "f3"),
    (KeyCode::F4, "f4"),
    (KeyCode::F5, "f5"),
    (KeyCode::F6, "f6"),
    (KeyCode::F7, "f7"),
    (KeyCode::F8, "f8"),
    (KeyCode::F9, "f9"),
    (KeyCode::F10, "f10"),
    (KeyCode::F11, "f11"),
    (KeyCode::F12, "f12"),
    (KeyCode::A, "a"),
    (KeyCode::B, "b"),
    (KeyCode::C, "c"),
    (KeyCode::D, "d"),
    (KeyCode::E, "e"),
    (KeyCode::F, "f"),
    (KeyCode::G, "g"),
    (KeyCode::H, "h"),
    (KeyCode::I, "i"),
    (KeyCode::J, "j"),
    (KeyCode::K, "k"),
    (KeyCode::L, "l"),
    (KeyCode::M, "m"),
    (KeyCode::N, "n"),
    (KeyCode::O, "o"),
    (KeyCode::P, "p"),
    (KeyCode::Q, "q"),
    (KeyCode::R, "r"),
    (KeyCode::S, "s"),
    (KeyCode::T, "t"),
    (KeyCode::U, "u"),
    (KeyCode::V, "v"),
    (KeyCode::W, "w"),
    (KeyCode::X, "x"),
    (KeyCode::Y, "y"),
    (KeyCode::Z, "z"),
    (KeyCode::Key0, "0"),
    (KeyCode::Key1, "1"),
    (KeyCode::Key2, "2"),
    (KeyCode::Key3, "3"),
    (KeyCode::Key4, "4"),
    (KeyCode::Key5, "5"),
    (KeyCode::Key6, "6"),
    (KeyCode::Key7, "7"),
    (KeyCode::Key8, "8"),
    (KeyCode::Key9, "9"),
    (KeyCode::Comma, ","),
    (KeyCode::Period, "."),
    (KeyCode::Slash, "/"),
    (KeyCode::Semicolon, ";"),
    (KeyCode::Apostrophe, "'"),
    (KeyCode::Minus, "-"),
    (KeyCode::Equal, "="),
    (KeyCode::LeftBracket, "["),
    (KeyCode::RightBracket, "]"),
    (KeyCode::Backslash, "\\"),
    (KeyCode::GraveAccent, "`"),
];

pub fn key_name(key: KeyCode) -> &'static str {
    KEY_NAMES.iter().find(|(candidate, _)| *candidate == key).map_or("???", |(_, name)| name)
}

pub fn key_by_name(name: &str) -> Option<KeyCode> {
    KEY_NAMES.iter().find(|(_, candidate)| *candidate == name).map(|(key, _)| *key)
}
