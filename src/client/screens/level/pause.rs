//! The pause menu of a round (Esc), which the original has no room for: the options,
//! the chat and the way out without closing the game.
//!
//! In a round on a server the level runs on while this is open, because the server
//! never stops: the others are only shown that this player is away (CLIENT_ROUND_PAUSE,
//! the AFK flag of the snapshot). Alone in a level the simulation stands still.
//! Either way the sounds fall silent and the music is left with its low end.

use crate::client::canvas::{Canvas, VIEW_WIDTH};
use crate::client::input::key_name;
use crate::client::options::{self, BoundKey, FULLSCREEN_MODE};
use crate::client::screens::chat::Chat;
use crate::client::Context;
use macroquad::input::KeyCode;
use crate::core::resources::names::{sound, sprite};
use crate::core::world::C_WHITE;

/// image_blend of the selected row (#910000), as in the main menu.
const SELECTED_BLEND: u32 = crate::client::canvas::make_color_rgb(0x91, 0x00, 0x00);
const OVERLAY_ALPHA: f64 = 0.7;
const TITLE_Y: f64 = 54.0;
const FIRST_ROW_Y: f64 = 92.0;
const ROW_HEIGHT: f64 = 16.0;
/// The controls page has too many rows for one column.
const CONTROLS_COLUMN_ROWS: usize = 7;
const CONTROLS_COLUMN_MIDDLE: [f64; 2] = [VIEW_WIDTH / 4.0, VIEW_WIDTH * 3.0 / 4.0];
/// A row answers the mouse around its letters as the main menu's buttons do.
const ROW_INK_HEIGHT: f64 = 8.0;
const ROW_CLICK_MARGIN_X: f64 = 10.0;
const ROW_CLICK_MARGIN_Y: f64 = 4.0;
/// The chat page: its lines above the box at the bottom of the screen. A spectator's
/// chat, which is not in this menu at all, stands in the same place (level/mod.rs).
pub(super) const CHAT_MAX_LENGTH: usize = 36;
pub(super) const CHAT_LINES_TOP: f64 = 150.0;
pub(super) const CHAT_BOX_TOP: f64 = 251.0;
const HINT_Y: f64 = 232.0;

/// What the level does after one step of the pause menu.
#[derive(PartialEq)]
pub enum Action {
    None,
    /// The player asked to leave the round.
    Leave,
}

/// The key bindings the controls page shows, by the name obj_config gave each panel.
const BINDINGS: [(&str, &str); 13] = [
    ("left", "left"),
    ("right", "right"),
    ("up", "up"),
    ("down", "down"),
    ("jump", "jump"),
    ("special1", "special 1"),
    ("special2", "special 2"),
    ("emotion1", "emote 1"),
    ("emotion2", "emote 2"),
    ("emotion3", "emote 3"),
    ("emotion4", "idle"),
    ("hidegui", "hide gui"),
    ("playerlist", "player list"),
];

#[derive(Default)]
pub struct Pause {
    pub open: bool,
    page: Page,
    /// The selected row of the page.
    row: usize,
    /// The binding waiting for the next key press (the controls page).
    binding: Option<usize>,
}

#[derive(Clone, Copy, Default, PartialEq)]
enum Page {
    #[default]
    Root,
    Options,
    Controls,
    Chat,
}

impl Pause {
    /// Esc and the keys of the menu. `online` leaves out what a level alone has no use
    /// for; the chat is answered with the level's own.
    pub fn step(&mut self, context: &mut Context, chat: &mut Chat, online: bool) -> Action {
        if self.binding.is_some() {
            self.take_binding(context);
            return Action::None;
        }
        if self.page == Page::Chat {
            return self.step_chat(context, chat);
        }
        let back = context.input.pressed(KeyCode::Escape) || context.input.pressed(context.options.keys.special1.0);
        // Only Escape opens the menu. The back key of the menus is special1, which in
        // a level is the first ability key: opening the pause menu with it would take
        // that ability away from every player who has it bound there (x by default).
        if !self.open {
            if context.input.pressed(KeyCode::Escape) {
                self.set_open(context, true);
            }
            return Action::None;
        }
        if back {
            self.go_back(context);
            return Action::None;
        }

        let rows = self.rows(context, online).len();
        let keys = context.options.keys.clone();
        if context.input.pressed(keys.up.0) || context.input.pressed(KeyCode::Up) {
            self.row = (self.row + rows - 1) % rows;
            context.audio.play_interface(sound::SND_MENU_SELECT);
        }
        if context.input.pressed(keys.down.0) || context.input.pressed(KeyCode::Down) {
            self.row = (self.row + 1) % rows;
            context.audio.play_interface(sound::SND_MENU_SELECT);
        }
        let clicked = self.clicked_row(context, online);
        if let Some(row) = clicked {
            self.row = row;
        }
        if context.input.pressed(keys.jump.0) || context.input.pressed(KeyCode::Enter) || clicked.is_some() {
            return self.press(context, chat, online);
        }
        Action::None
    }

    /// The chat page takes the keyboard, so only Enter and Esc are read as keys.
    fn step_chat(&mut self, context: &mut Context, chat: &mut Chat) -> Action {
        chat.type_message(&mut context.input, CHAT_MAX_LENGTH);
        if context.input.pressed(KeyCode::Enter) {
            // Sends what was typed and starts the next line.
            chat.toggle(context);
            chat.open(&mut context.input);
        }
        if context.input.pressed(KeyCode::Escape) {
            chat.is_open = false;
            context.input.keyboard_string.clear();
            self.page = Page::Root;
            self.row = 0;
            context.audio.play_interface(sound::SND_MENU_PRESS);
        }
        Action::None
    }

    /// The next key pressed becomes the binding being changed (bindChanged).
    fn take_binding(&mut self, context: &mut Context) {
        if !context.input.any_key_pressed() {
            return;
        }
        let Some(index) = self.binding.take() else { return };
        let Some(key) = context.input.last_key else { return };
        if key != KeyCode::Escape {
            if let Some(binding) = context.options.keys.by_panel_name(BINDINGS[index].0) {
                *binding = BoundKey(key);
            }
            context.options.save();
        }
    }

    fn go_back(&mut self, context: &mut Context) {
        match self.page {
            Page::Root => self.set_open(context, false),
            Page::Controls => {
                self.page = Page::Options;
                self.row = 0;
                context.audio.play_interface(sound::SND_MENU_PRESS);
            }
            _ => {
                self.page = Page::Root;
                self.row = 0;
                context.audio.play_interface(sound::SND_MENU_PRESS);
            }
        }
    }

    /// Opening and closing also mutes the sounds and muffles the music.
    fn set_open(&mut self, context: &mut Context, open: bool) {
        self.open = open;
        self.page = Page::Root;
        self.row = 0;
        context.audio.set_paused(open);
        context.audio.play_interface(sound::SND_MENU_PRESS);
        // The others see the player standing there, marked as away.
        if context.net.is_connected {
            context.net.send_round_pause(open);
        }
    }

    /// The level is over (the round ended, the player left): only the sound effect is
    /// undone, because there is no round left to tell and no button was pressed.
    pub fn end(&mut self, context: &mut Context) {
        if self.open {
            self.open = false;
            context.audio.set_paused(false);
        }
    }

    fn press(&mut self, context: &mut Context, chat: &mut Chat, online: bool) -> Action {
        context.audio.play_interface(sound::SND_MENU_PRESS);
        let row = self.row;
        match self.page {
            Page::Root => match self.root_row(row, online) {
                RootRow::Resume => self.set_open(context, false),
                RootRow::Options => {
                    self.page = Page::Options;
                    self.row = 0;
                }
                RootRow::Chat => {
                    self.page = Page::Chat;
                    chat.open(&mut context.input);
                }
                RootRow::Leave => {
                    self.set_open(context, false);
                    return Action::Leave;
                }
            },
            Page::Options => match row {
                0 => {
                    let options = &mut context.options;
                    options.screen_mode = if options.screen_mode >= FULLSCREEN_MODE { 0 } else { options.screen_mode + 1 };
                    options::apply_screen_mode(options.screen_mode);
                    options.save();
                }
                1 => {
                    context.options.show_ping = !context.options.show_ping;
                    context.options.save();
                }
                2 => {
                    context.options.show_fps = !context.options.show_fps;
                    context.options.save();
                }
                3 => {
                    self.page = Page::Controls;
                    self.row = 0;
                }
                _ => self.go_back(context),
            },
            Page::Controls => {
                if row < BINDINGS.len() {
                    self.binding = Some(row);
                } else {
                    self.go_back(context);
                }
            }
            Page::Chat => {}
        }
        Action::None
    }

    fn root_row(&self, row: usize, online: bool) -> RootRow {
        match (row, online) {
            (0, _) => RootRow::Resume,
            (1, _) => RootRow::Options,
            (2, true) => RootRow::Chat,
            _ => RootRow::Leave,
        }
    }

    /// The rows of the page as they are shown.
    fn rows(&self, context: &Context, online: bool) -> Vec<String> {
        match self.page {
            Page::Root => {
                let mut rows = vec!["resume".to_string(), "options".to_string()];
                if online {
                    rows.push("chat".to_string());
                }
                rows.push(if online { "leave the round".to_string() } else { "leave".to_string() });
                rows
            }
            Page::Options => vec![
                format!("screen size: {}", screen_size_name(context.options.screen_mode)),
                format!("ping: {}", on_off(context.options.show_ping)),
                format!("fps: {}", on_off(context.options.show_fps)),
                "controls".to_string(),
                "back".to_string(),
            ],
            Page::Controls => {
                let mut keys = context.options.keys.clone();
                let mut rows: Vec<String> = BINDINGS
                    .iter()
                    .enumerate()
                    .map(|(index, (panel, label))| {
                        let key = keys.by_panel_name(panel).map_or("???", |bound| key_name(bound.0));
                        let shown = if self.binding == Some(index) { "..." } else { key };
                        format!("{label}: {shown}")
                    })
                    .collect();
                rows.push("back".to_string());
                rows
            }
            Page::Chat => Vec::new(),
        }
    }

    /// The row the mouse is over and pressed this step.
    fn clicked_row(&self, context: &Context, online: bool) -> Option<usize> {
        if !context.input.mouse_left_pressed() {
            return None;
        }
        let (mouse_x, mouse_y) = (context.input.mouse_x, context.input.mouse_y);
        self.rows(context, online).iter().enumerate().find_map(|(index, text)| {
            let (left, top) = self.row_place(&context.canvas, index, text);
            let right = left + context.canvas.menu_text_width(text);
            let over = (left - ROW_CLICK_MARGIN_X..right + ROW_CLICK_MARGIN_X).contains(&mouse_x)
                && (top - ROW_CLICK_MARGIN_Y..top + ROW_INK_HEIGHT + ROW_CLICK_MARGIN_Y).contains(&mouse_y);
            over.then_some(index)
        })
    }

    /// Where a row's letters start: in the middle of the screen, or in the middle of
    /// one of the two columns the bindings stand in.
    fn row_place(&self, canvas: &Canvas, index: usize, text: &str) -> (f64, f64) {
        let width = canvas.menu_text_width(text);
        if self.page == Page::Controls {
            let column = (index / CONTROLS_COLUMN_ROWS).min(CONTROLS_COLUMN_MIDDLE.len() - 1);
            let left = (CONTROLS_COLUMN_MIDDLE[column] - width / 2.0).floor();
            return (left, FIRST_ROW_Y + (index % CONTROLS_COLUMN_ROWS) as f64 * ROW_HEIGHT);
        }
        (((VIEW_WIDTH - width) / 2.0).floor(), FIRST_ROW_Y + index as f64 * ROW_HEIGHT)
    }

    /// Draw GUI: over everything the level drew.
    pub fn draw(&self, context: &mut Context, chat: &Chat, online: bool) {
        if !self.open {
            return;
        }
        let rows = self.rows(context, online);
        let canvas = &mut context.canvas;
        canvas.draw_sprite_ext(sprite::SPR_BLACK, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, C_WHITE, OVERLAY_ALPHA);
        let title = if self.page == Page::Chat { "chat" } else { "paused" };
        canvas.draw_menu_text(centered(canvas, title), TITLE_Y, title, C_WHITE, 1.0);
        if self.page == Page::Chat {
            chat.draw(canvas, CHAT_LINES_TOP, CHAT_BOX_TOP);
            let hint = "enter sends, esc goes back";
            canvas.draw_menu_text(centered(canvas, hint), HINT_Y, hint, C_WHITE, 1.0);
            return;
        }
        for (index, text) in rows.iter().enumerate() {
            let colour = if index == self.row { SELECTED_BLEND } else { C_WHITE };
            let (left, top) = self.row_place(canvas, index, text);
            canvas.draw_menu_text(left, top, text, colour, 1.0);
        }
        if online {
            let hint = "the round runs on: the others see you as afk";
            canvas.draw_menu_text(centered(canvas, hint), HINT_Y, hint, C_WHITE, 1.0);
        }
    }
}

enum RootRow {
    Resume,
    Options,
    Chat,
    Leave,
}

fn centered(canvas: &Canvas, text: &str) -> f64 {
    ((VIEW_WIDTH - canvas.menu_text_width(text)) / 2.0).floor()
}

fn on_off(on: bool) -> &'static str {
    if on {
        "shown"
    } else {
        "hidden"
    }
}

fn screen_size_name(screen_mode: u8) -> String {
    if screen_mode >= FULLSCREEN_MODE {
        "fullscreen".to_string()
    } else {
        format!("{}x", screen_mode + 1)
    }
}
