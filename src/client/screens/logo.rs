//! room_logo (obj_logo): three logos, each changed with a white flash, then the
//! main menu. Any key or mouse button skips straight to the menu.

use super::fades::{BlackFadeOut, WhiteFlash};
use super::{count_down_alarm, ALARM_OFF};
use crate::client::room::Room;
use crate::client::Context;
use anyhow::{Context as _, Result};
use crate::core::resources::names::{sound, sprite};
use crate::core::objects::ids::ObjectId;
use crate::core::rooms::ids::RoomId;
use crate::core::world::C_WHITE;
use crate::core::config::ticks;

/// alarm[0] values: how long each logo stays. The "powered by GameMaker" picture the
/// original opened with is gone (this port is not made with it); its author's credit
/// stayed, and so did the EXE Empire card, with the times those two had.
const LOGO_SECONDS: [f64; 2] = [2.4, 2.5];

pub struct Logo {
    room: Room,
    /// Where obj_logo stands: its layer and position.
    logo_layer: usize,
    logo_position: (f64, f64),
    /// ind: the logo on screen. Past the last logo means "go to the menu".
    logo: i32,
    logo_count: i32,
    /// Draw_77 starts the music and the timer on the first frame.
    started: bool,
    alarm: i32,
    fade: Option<BlackFadeOut>,
    flash: Option<WhiteFlash>,
}

impl Logo {
    pub fn open(context: &mut Context) -> Result<Logo> {
        let room = Room::load(&context.maps_folder, RoomId::Logo, &context.canvas.sprites)?;
        let (logo_layer, placed) = room.placed(ObjectId::Logo).next().context("room_logo has no obj_logo")?;
        let logo_position = (placed.x, placed.y);
        Ok(Logo {
            room,
            logo_layer,
            logo_position,
            logo: 0,
            logo_count: context.canvas.sprites.get(sprite::SPR_LOGO_LOGOS).frame_count as i32,
            started: false,
            alarm: ALARM_OFF,
            fade: None,
            flash: None,
        })
    }

    pub fn tick(&mut self, context: &mut Context) -> Option<RoomId> {
        let mut next_room = None;
        if count_down_alarm(&mut self.alarm) {
            next_room = self.next_logo();
        }
        // Keyboard_1 and Step_0: a key or a mouse button *pressed* skips. Held keys do
        // not, or the game would skip the logos when it starts with a key down, and the
        // window taking the pointer would count as one.
        if context.input.any_key_pressed() || context.input.mouse_any_pressed() {
            self.logo = self.logo_count;
            self.alarm = 1;
            context.audio.stop_all();
        }
        self.fade = self.fade.take().and_then(|mut fade| fade.step().then_some(fade));
        self.flash = self.flash.take().and_then(|mut flash| flash.step().then_some(flash));

        self.draw(context);
        if !self.started {
            self.started = true;
            self.fade = Some(BlackFadeOut::new());
            context.audio.play(sound::MUS_LOGO, false);
            self.alarm = ticks(LOGO_SECONDS[0]);
        }
        if let Some(fade) = &self.fade {
            fade.draw_gui(&mut context.canvas);
        }
        next_room
    }

    /// Alarm_0: the next logo with a flash, or the menu after the last one. The menu
    /// opens with its own flash, so none is started here for that one frame.
    fn next_logo(&mut self) -> Option<RoomId> {
        self.logo += 1;
        if self.logo > self.logo_count - 1 {
            return Some(RoomId::Menu);
        }
        self.flash = Some(WhiteFlash::new());
        if let Some(&seconds) = LOGO_SECONDS.get(self.logo as usize) {
            self.alarm = ticks(seconds);
        }
        None
    }

    fn draw(&self, context: &mut Context) {
        let (x, y) = self.logo_position;
        self.room.draw_layers_with(&mut context.canvas, |canvas, layer| {
            if layer == self.logo_layer {
                canvas.draw_sprite_ext(sprite::SPR_LOGO_LOGOS, self.logo as f64, x, y, 1.0, 1.0, 0.0, C_WHITE, 1.0);
            }
        });
        if let Some(flash) = &self.flash {
            flash.draw(&mut context.canvas);
        }
    }
}
