//! obj_lobby_votekick and obj_menu_waitkick: picking a player with left and right, for
//! a vote kick or, for operators, a kick, ban or operator rights. The lobby and the
//! waiting room keep their own list of who is there, so the list is passed in.

use crate::client::canvas::Canvas;
use crate::client::Context;
use crate::packet::{Packet, PacketType};
use crate::core::resources::names::sprite;

const ARROW_ROW_LEFT: f64 = 40.0;
const ARROW_ROW_STEP: f64 = 80.0;
const ARROW_Y: f64 = 58.0;
const ARROW_FRAME_MILLISECONDS: f64 = 300.0;

pub struct PlayerPicker {
    /// The packet that carries the picked player.
    answer: PacketType,
    /// ind: position among the other players.
    choice: usize,
}

/// What the picker did this step.
pub enum PickerOutcome {
    StillPicking,
    Closed,
}

impl PlayerPicker {
    /// `request` is the server's SERVER_LOBBY_CHOOSE* packet.
    pub fn new(request: PacketType) -> PlayerPicker {
        let answer = match request {
            PacketType::SERVER_LOBBY_CHOOSEKICK => PacketType::CLIENT_LOBBY_CHOOSEKICK,
            PacketType::SERVER_LOBBY_CHOOSEBAN => PacketType::CLIENT_LOBBY_CHOOSEBAN,
            PacketType::SERVER_LOBBY_CHOOSEOP => PacketType::CLIENT_LOBBY_CHOOSEOP,
            _ => PacketType::CLIENT_LOBBY_CHOOSEVOTEKICK,
        };
        PlayerPicker { answer, choice: 0 }
    }

    /// Draw_0: Z sends the pick, X cancels; the arrow marks the choice. `others` is
    /// everyone who can be picked, in the order their icons stand on the screen.
    pub fn draw(&mut self, context: &mut Context, others: &[u16], canvas_time_ms: f64) -> PickerOutcome {
        let picked = others.get(self.choice).copied();
        let keys = &context.options.keys;
        if let Some(player) = picked {
            if context.input.pressed(keys.jump.0) {
                let mut packet = Packet::new(self.answer);
                let _ = packet.write_u16(player);
                context.net.send_reliable(&packet);
                return PickerOutcome::Closed;
            }
            if context.input.pressed(keys.special1.0) {
                return PickerOutcome::Closed;
            }
        }
        if others.is_empty() {
            return PickerOutcome::Closed;
        }
        let mut choice = self.choice as i32;
        if context.input.pressed(keys.left.0) {
            choice -= 1;
        }
        if context.input.pressed(keys.right.0) {
            choice += 1;
        }
        let count = others.len() as i32;
        if choice < 0 {
            choice = count - 1;
        } else if choice >= count {
            choice = 0;
        }
        self.choice = choice as usize;
        draw_arrow(&mut context.canvas, self.choice, canvas_time_ms);
        PickerOutcome::StillPicking
    }
}

fn draw_arrow(canvas: &mut Canvas, choice: usize, current_time_ms: f64) {
    let x = ARROW_ROW_LEFT + choice as f64 * ARROW_ROW_STEP;
    canvas.draw_sprite(sprite::SPR_LOBBY_ICON_ARROW, current_time_ms / ARROW_FRAME_MILLISECONDS, x, ARROW_Y);
}
