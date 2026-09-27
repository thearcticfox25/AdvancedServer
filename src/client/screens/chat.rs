//! The chat of the lobby and the waiting room: the lines on screen and the line
//! being typed (the chat parts of obj_lobby and obj_menu_waiting,
//! lobby_add_message, scr_chat_filter).

use crate::client::canvas::Canvas;
use crate::client::input::Input;
use crate::client::text::draw_text;
use crate::client::Context;
use macroquad::input::KeyCode;
use crate::packet::{Packet, PacketType};
use crate::core::resources::names::{sound, sprite};
use crate::core::world::C_WHITE;

/// A ninth line pushes the oldest one out.
const MAX_LINES: usize = 8;
const MUTE_COMMANDS: [&str; 2] = [".mute", ".m"];
/// Words scr_chat_filter replaces with stars, in sent and shown messages alike.
const FILTERED_WORDS: [&str; 15] = [
    "nigger", "nigga", "niggas", "nigg", "faggot", "tranny", "экзюшка", "эгусик", "эггусик", "эмичка", "кримка", "салька", "наксик",
    "хохол", "москаль",
];

#[derive(Default)]
pub struct Chat {
    lines: Vec<String>,
    /// chatMsg
    pub typed: String,
    /// chatMode
    pub is_open: bool,
}

impl Chat {
    /// lobby_add_message
    pub fn add_message(&mut self, context: &mut Context, sender: &str, text: &str) {
        if context.chat_muted {
            return;
        }
        context.audio.stop(sound::SND_MESSAGE);
        // Heard in the pause menu's chat too, where the level's sounds are muted.
        context.audio.play_interface(sound::SND_MESSAGE);
        self.lines.push(format!("{sender}: {}~", filter(text)));
        if self.lines.len() > MAX_LINES {
            self.lines.remove(0);
        }
    }

    /// While the chat is open, what is typed becomes the message, lowercase and cut to `max_length`.
    pub fn type_message(&mut self, input: &mut Input, max_length: usize) {
        if !self.is_open {
            return;
        }
        if input.control_held() && input.pressed(KeyCode::V) {
            input.keyboard_string = macroquad::miniquad::window::clipboard_get().unwrap_or_default();
        }
        self.typed = input.keyboard_string.to_lowercase().chars().take(max_length).collect();
        if input.keyboard_string.chars().count() > max_length {
            input.keyboard_string = self.typed.clone();
        }
    }

    /// Enter: opens the chat, or sends the typed message and closes it.
    pub fn toggle(&mut self, context: &mut Context) {
        self.is_open = !self.is_open;
        if self.is_open {
            context.input.keyboard_string = self.typed.clone();
            return;
        }
        if self.typed.is_empty() {
            return;
        }
        let message = self.typed.to_lowercase();
        if MUTE_COMMANDS.contains(&message.as_str()) {
            self.toggle_mute(context);
        } else {
            send_message(context, &message);
            let nickname = context.options.nickname.clone();
            self.add_message(context, &nickname, &message);
        }
        self.typed.clear();
        context.input.keyboard_string.clear();
    }

    /// T: opens the chat.
    pub fn open(&mut self, input: &mut Input) {
        if !self.is_open {
            self.is_open = true;
            input.keyboard_string = self.typed.clone();
        }
    }

    /// The lines with their bottom line at `lines_top`, and the box with the typed message.
    pub fn draw(&self, canvas: &mut Canvas, lines_top: f64, box_top: f64) {
        let lines: String = self.lines.iter().map(|line| format!("{line}\n")).collect();
        draw_text(canvas, 3.0, lines_top, &lines, C_WHITE, 1.0);
        let box_frame = if self.is_open || !self.typed.is_empty() { 1.0 } else { 0.0 };
        canvas.draw_sprite(sprite::SPR_LOBBY_CHATBOX, box_frame, 0.0, box_top);
        draw_text(canvas, 2.0, box_top + 3.0, &self.typed, C_WHITE, 1.0);
    }

    fn toggle_mute(&mut self, context: &mut Context) {
        if context.chat_muted {
            context.chat_muted = false;
            self.add_message(context, "(mute)", "@chat is now unmuted.~");
        } else {
            self.add_message(context, "(mute)", "\\chat is now muted.~");
            context.chat_muted = true;
        }
    }
}

/// CLIENT_CHAT_MESSAGE, which the server shows to everyone else.
fn send_message(context: &mut Context, message: &str) {
    let mut packet = Packet::passthrough(PacketType::CLIENT_CHAT_MESSAGE);
    let _ = packet.write_u16(context.net.id.unwrap_or(0));
    let _ = packet.write_str(&filter(message));
    context.net.send_reliable(&packet);
}

/// scr_chat_filter: filtered words become stars; line breaks and tabs are removed.
pub fn filter(text: &str) -> String {
    let mut filtered = text.to_string();
    for word in FILTERED_WORDS {
        filtered = filtered.replace(word, &"*".repeat(word.chars().count()));
    }
    filtered.replace(['\r', '\t', '\n'], "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_masks_words_and_strips_breaks() {
        assert_eq!(filter("hi\nхохол"), "hi*****");
    }
}
