//! One screen per GameMaker room the client shows. Every tick a screen runs one
//! step and draws one picture; asking for another room (room_goto) switches the
//! screen after that tick, as GameMaker does at the end of a step.

mod character_art;
mod chat;
mod connecting;
mod fades;
mod level;
mod lobby;
mod logo;
mod map_preview;
mod menu;
mod message;
mod minigame;
mod results;
mod round_clock;
mod vote_kick;
mod waiting;

use crate::client::options::{self, FULLSCREEN_MODE};
use crate::client::Context;
use anyhow::{bail, Result};
use macroquad::input::{mouse_position, KeyCode};
use crate::core::rooms::ids::RoomId;

pub use results::ResultsRow;

pub enum Screen {
    Logo(logo::Logo),
    Menu(menu::Menu),
    Connecting(connecting::Connecting),
    Message(message::Message),
    Waiting(waiting::Waiting),
    Lobby(lobby::Lobby),
    Level(level::Level),
    Results(results::Results),
}

/// alarm[n] == -1
pub const ALARM_OFF: i32 = -1;
/// The "back" button of the connecting, waiting and message rooms (point_in_rectangle 223, 153, 32x16).
const CANCEL_BUTTON: [f64; 4] = [223.0, 153.0, 255.0, 169.0];

impl Screen {
    /// room_goto
    pub fn open(room: RoomId, context: &mut Context) -> Result<Screen> {
        Ok(match room {
            RoomId::Logo => Screen::Logo(logo::Logo::open(context)?),
            RoomId::Menu => Screen::Menu(menu::Menu::open(context)?),
            RoomId::Connecting => Screen::Connecting(connecting::Connecting::open(context)?),
            RoomId::Message => Screen::Message(message::Message::open(context)?),
            RoomId::Waiting => Screen::Waiting(waiting::Waiting::open(context)?),
            RoomId::Lobby => Screen::Lobby(lobby::Lobby::open(context)?),
            RoomId::Results => Screen::Results(results::Results::open(context)?),
            level if crate::client::levels::LEVELS.iter().any(|known| known.room == level) => Screen::Level(level::Level::open(context, level)?),
            other => bail!("room {other:?} is not ported yet"),
        })
    }

    /// One step and one picture. Returns the room to go to, if the screen asked for one.
    pub fn tick(&mut self, context: &mut Context) -> Option<RoomId> {
        let (mouse_x, mouse_y) = mouse_position();
        let mouse = context.canvas.window_to_view(mouse_x, mouse_y);
        context.input.begin_tick(mouse);
        toggle_fullscreen_on_f4(context);
        context.canvas.begin();
        match self {
            Screen::Logo(logo) => logo.tick(context),
            Screen::Menu(menu) => menu.tick(context),
            Screen::Connecting(connecting) => connecting.tick(context),
            Screen::Message(message) => message.tick(context),
            Screen::Waiting(waiting) => waiting.tick(context),
            Screen::Lobby(lobby) => lobby.tick(context),
            Screen::Level(level) => level.tick(context),
            Screen::Results(results) => results.tick(context),
        }
    }
}

/// alarm[n]: counts down one step; true on the step the alarm rings.
pub fn count_down_alarm(alarm: &mut i32) -> bool {
    if *alarm <= 0 {
        return false;
    }
    *alarm -= 1;
    if *alarm > 0 {
        return false;
    }
    *alarm = ALARM_OFF;
    true
}

/// global.errorCode = code; room_goto(room_message)
pub fn go_to_error(context: &mut Context, code: u32) -> RoomId {
    context.error_code = code;
    RoomId::Message
}

/// Any mouse button pressed on the cancel button.
pub fn is_cancel_button_clicked(context: &Context) -> bool {
    let [left, top, right, bottom] = CANCEL_BUTTON;
    let (x, y) = (context.input.mouse_x, context.input.mouse_y);
    context.input.mouse_any_pressed() && (left..=right).contains(&x) && (top..=bottom).contains(&y)
}

/// obj_controls Begin Step: F4 switches to fullscreen and back to the last window size.
fn toggle_fullscreen_on_f4(context: &mut Context) {
    if !context.input.pressed(KeyCode::F4) {
        return;
    }
    let options = &mut context.options;
    if options.screen_mode == FULLSCREEN_MODE {
        options.screen_mode = context.windowed_screen_mode;
    } else {
        context.windowed_screen_mode = options.screen_mode;
        options.screen_mode = FULLSCREEN_MODE;
    }
    options::apply_screen_mode(options.screen_mode);
    options.save();
}
