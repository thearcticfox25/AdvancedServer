//! room_message (obj_message): why the connection failed or ended.

use super::is_cancel_button_clicked;
use crate::client::net;
use crate::client::room::Room;
use crate::client::Context;
use anyhow::{Context as _, Result};
use crate::core::resources::names::sprite;
use crate::core::objects::ids::ObjectId;
use crate::core::rooms::ids::RoomId;
use crate::core::world::C_WHITE;

pub struct Message {
    room: Room,
    message_layer: usize,
    message_position: (f64, f64),
}

impl Message {
    pub fn open(context: &mut Context) -> Result<Message> {
        let room = Room::load(&context.maps_folder, RoomId::Message, &context.canvas.sprites)?;
        let (message_layer, placed) = room.placed(ObjectId::Message).next().context("room_message has no obj_message")?;
        let message_position = (placed.x, placed.y);
        // Create_0: the connection is gone for good.
        context.net = net::NetClient::new();
        context.audio.stop_music();
        Ok(Message { room, message_layer, message_position })
    }

    pub fn tick(&mut self, context: &mut Context) -> Option<RoomId> {
        self.room.step(&context.canvas.sprites);
        let next_room = (context.input.pressed(context.options.keys.jump.0) || is_cancel_button_clicked(context)).then(|| net::reset(context));
        let frame = context.error_code as f64;
        let (x, y) = self.message_position;
        let message_layer = self.message_layer;
        self.room.draw_layers_with(&mut context.canvas, |canvas, layer| {
            if layer == message_layer {
                canvas.draw_sprite_ext(sprite::SPR_MENU_ERROR, frame, x, y, 1.0, 1.0, 0.0, C_WHITE, 1.0);
            }
        });
        next_room
    }
}
