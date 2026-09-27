//! room_connecting (obj_connecting): shown while ENet connects to the server.

use super::{count_down_alarm, go_to_error, is_cancel_button_clicked};
use crate::client::net::{self, Notice};
use crate::client::room::Room;
use crate::client::Context;
use anyhow::Result;
use crate::core::rooms::ids::RoomId;
use crate::core::config::ticks;

/// alarm[0]: steps before the connection attempt starts, so the room shows first.
const CONNECT_DELAY_SECONDS: f64 = 10.0 / 60.0;
/// global.errorCode when the address cannot be connected to.
const CANNOT_CONNECT_ERROR: u32 = 0;

pub struct Connecting {
    room: Room,
    connect_alarm: i32,
}

impl Connecting {
    pub fn open(context: &mut Context) -> Result<Connecting> {
        Ok(Connecting { room: Room::load(&context.maps_folder, RoomId::Connecting, &context.canvas.sprites)?, connect_alarm: ticks(CONNECT_DELAY_SECONDS) })
    }

    pub fn tick(&mut self, context: &mut Context) -> Option<RoomId> {
        let next_room = self.step(context);
        self.room.draw_layers_with(&mut context.canvas, |_, _| {});
        next_room
    }

    fn step(&mut self, context: &mut Context) -> Option<RoomId> {
        self.room.step(&context.canvas.sprites);
        if count_down_alarm(&mut self.connect_alarm) && !context.net.connect() {
            return Some(go_to_error(context, CANNOT_CONNECT_ERROR));
        }
        if context.input.pressed(context.options.keys.jump.0) || is_cancel_button_clicked(context) {
            return Some(net::reset(context));
        }
        for notice in net::update(context) {
            match notice {
                Notice::GoTo(room) => return Some(room),
                Notice::ShowError(code) => return Some(go_to_error(context, code)),
                _ => {}
            }
        }
        None
    }
}
