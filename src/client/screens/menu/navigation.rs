//! obj_menu: moving the selection with the keyboard and what each button does
//! (Step_0, buttonListed, buttonSelected, buttonPressed, changeSubmenu,
//! textChanged, bindChanged). Ported branch by branch, quirks included, so it
//! can be compared with the GML side by side.

use super::submenus::{self, *};
use super::widgets::{ButtonKind, Kind, SELECTION_RAISE};
use super::Menu;
use crate::client::input::key_name;
use crate::client::options::{self, validate_nickname, BoundKey, KeyBindings, FULLSCREEN_MODE};
use crate::client::Context;
use macroquad::input::KeyCode;
use crate::core::resources::names::{sound, sprite};
use crate::core::rooms::ids::RoomId;
use crate::core::world::C_WHITE;

/// image_blend of the selected button (#910000).
const SELECTED_BLEND: u32 = crate::client::canvas::make_color_rgb(0x91, 0x00, 0x00);
/// Where the credits start scrolling from.
const CREDITS_START_Y: f64 = 196.0;
/// The pet pages in order, and their page row. The last two hold the former secret pets.
const PET_PAGES: [i32; 3] = [EXTRA_PETS, EXTRA_PETS2, EXTRA_PETS3];
const PET_PAGE_ROW: i32 = 3;
/// Column of the mouse-only page buttons on the icon and pet pages.
const PAGE_BUTTONS_COLUMN: i32 = 4;
/// spr_menu_buttons frames of the Extras tabs (achievements, skins, icons, pets, back),
/// of the page numbers, and of the character names in the order of their skin pages.
const TAB_FRAMES: [usize; 5] = [54, 56, 55, 57, 4];
const PAGE_NUMBER_FRAMES: [usize; 3] = [58, 59, 60];
const CHARACTER_NAME_FRAMES: [usize; 10] = [43, 44, 45, 46, 47, 48, 49, 52, 50, 51];
/// The page's own character name stands in the middle of the view.
const CHARACTER_NAME_CENTRE: f64 = 240.0;
/// The join page's bottom row as the mobile version had it (SUBMENU_JOIN_MOBILE): back
/// moved to the left, and right of it Singleplayer where Host started a server of its
/// own. The public servers of the original are gone, they no longer answer.
const JOIN_BOTTOM_ROW: i32 = 1;
const JOIN_BOTTOM_COLUMNS: i32 = 2;
const SINGLEPLAYER_COLUMN: i32 = 1;

impl Menu {
    /// Step_0: keyboard navigation.
    pub(super) fn step_navigation(&mut self, context: &mut Context) {
        self.pressed_this_step = false;

        if self.waiting_for_key {
            if context.input.any_key_pressed() {
                if let (Some(panel), Some(key)) = (self.binding_panel.clone(), context.input.last_key) {
                    self.bind_changed(context, &panel, key);
                }
                self.waiting_for_key = false;
            }
            return;
        }

        let keys = context.options.keys.clone();
        let pressed = |context: &Context, key: BoundKey| context.input.pressed(key.0);
        if self.grid {
            if pressed(context, keys.up) {
                self.selected_row -= 1;
                if self.selected_row < 0 {
                    self.selected_column = -1;
                    self.selected_row = self.rows - 1;
                }
                self.button_selected(context, self.selected_row, self.selected_column, 0, true);
                context.audio.play(sound::SND_MENU_SELECT, false);
            }
            if pressed(context, keys.down) {
                self.selected_row += 1;
                if self.selected_row >= self.rows {
                    self.selected_row = 0;
                }
                self.button_selected(context, self.selected_row, self.selected_column, 0, true);
                context.audio.play(sound::SND_MENU_SELECT, false);
            }
            if pressed(context, keys.left) {
                self.selected_column -= 1;
                if self.selected_column < 0 && !self.button_listed(context, self.selected_row, self.selected_column, -1) {
                    self.selected_column = self.columns - 1;
                }
                self.button_selected(context, self.selected_row, self.selected_column, -1, true);
                context.audio.play(sound::SND_MENU_SELECT, false);
            }
            if pressed(context, keys.right) {
                let mut listed = false;
                self.selected_column += 1;
                if self.selected_column >= self.columns {
                    if self.button_listed(context, self.selected_row, self.selected_column, 1) {
                        listed = true;
                    } else {
                        self.selected_column = 0;
                    }
                }
                self.button_selected(context, self.selected_row, self.selected_column, 1, true);
                context.audio.play(sound::SND_MENU_SELECT, false);
                if self.selected_column == -1 && !listed {
                    self.button_listed(context, self.selected_row, self.selected_column, 1);
                }
            }
        } else {
            if pressed(context, keys.left) {
                self.selected_row -= 1;
                if self.selected_row < 0 {
                    self.selected_row = self.rows - 1;
                }
                self.button_selected(context, self.selected_row, self.selected_column, 0, true);
                context.audio.play(sound::SND_MENU_SELECT, false);
            }
            if pressed(context, keys.right) {
                self.selected_row += 1;
                if self.selected_row >= self.rows {
                    self.selected_row = 0;
                }
                self.button_selected(context, self.selected_row, self.selected_column, 0, true);
                context.audio.play(sound::SND_MENU_SELECT, false);
            }
        }

        if !self.ignore_buttons {
            if pressed(context, keys.jump) {
                self.button_pressed(context, self.selected_row, self.selected_column);
            }
            if pressed(context, keys.special1) {
                self.change_submenu(context, self.previous_page());
            }
        }
        if context.input.pressed(KeyCode::Escape) {
            self.change_submenu(context, self.previous_page());
        }
        if context.input.pressed(KeyCode::Enter) {
            self.button_pressed(context, self.selected_row, self.selected_column);
        }
    }

    /// buttonListed: moving sideways past the first or last column. Some pages turn
    /// the page instead; true means the move was used up.
    /// `direction` is negative for left and positive for right.
    fn button_listed(&mut self, context: &mut Context, bid: i32, bcolumn: i32, direction: i32) -> bool {
        let right = direction > 0;
        match self.submenu {
            OPTIONS_GENERAL if bid == 1 => {
                let options = &mut context.options;
                options.screen_mode = if right {
                    if options.screen_mode >= FULLSCREEN_MODE { 0 } else { options.screen_mode + 1 }
                } else if options.screen_mode == 0 {
                    FULLSCREEN_MODE
                } else {
                    options.screen_mode - 1
                };
                options::apply_screen_mode(options.screen_mode);
                options.save();
                true
            }
            OPTIONS_GENERAL => {
                if bid != 4 || bcolumn > -1 {
                    return false;
                }
                if right {
                    self.change_submenu(context, OPTIONS_CONTROLS);
                    self.button_selected(context, 10, -1, -1, true);
                } else {
                    self.change_submenu(context, OPTIONS_BACK);
                    self.button_selected(context, 0, -1, 0, true);
                }
                true
            }
            OPTIONS_CONTROLS => {
                if bid != 10 {
                    return false;
                }
                if right {
                    self.change_submenu(context, OPTIONS_BACK);
                    self.button_selected(context, 0, -1, 0, true);
                } else {
                    self.change_submenu(context, OPTIONS_GENERAL);
                    self.button_selected(context, 4, -1, 0, true);
                }
                true
            }
            OPTIONS_BACK => {
                if right {
                    self.change_submenu(context, OPTIONS_GENERAL);
                    self.button_selected(context, 4, -1, 0, true);
                } else {
                    self.change_submenu(context, OPTIONS_CONTROLS);
                    self.button_selected(context, 10, -1, -1, true);
                }
                true
            }
            MECH_SPIN..=MECH_DIFF => {
                if bid == 0 {
                    // Wrapping around skips the animation restart below.
                    if !self.turn_page(context, MECH_SPIN, MECH_DIFF, right) {
                        return false;
                    }
                    self.restart_gifs();
                } else if right {
                    self.change_submenu(context, CHAR_TAILS);
                    self.button_selected(context, 1, 0, -1, true);
                } else {
                    self.change_submenu(context, ABOUT_BACK);
                }
                false
            }
            CHAR_TAILS..=CHAR_EXELLER => {
                if bid == 0 {
                    if !self.turn_page(context, CHAR_TAILS, CHAR_EXELLER, right) {
                        return false;
                    }
                    self.restart_gifs();
                } else if right {
                    self.change_submenu(context, ABOUT_CREDITS);
                } else {
                    self.change_submenu(context, MECH_SPIN);
                    self.button_selected(context, 1, 0, -1, false);
                }
                false
            }
            ABOUT_CREDITS => {
                if right {
                    self.change_submenu(context, ABOUT_BACK);
                } else {
                    self.change_submenu(context, CHAR_TAILS);
                    self.button_selected(context, 1, 0, 0, true);
                }
                false
            }
            ABOUT_BACK => {
                if right {
                    self.change_submenu(context, MECH_SPIN);
                    self.button_selected(context, 1, 0, 0, true);
                } else {
                    self.change_submenu(context, ABOUT_CREDITS);
                }
                false
            }
            EXTRA_ACHIEVEMENTS => {
                if right {
                    self.change_submenu(context, EXTRA_TAILS);
                    self.button_selected(context, 3, -1, 0, true);
                } else {
                    self.change_submenu(context, EXTRA_BACK);
                }
                true
            }
            EXTRA_BACK => {
                if right {
                    self.change_submenu(context, EXTRA_ACHIEVEMENTS);
                    self.button_selected(context, 0, -1, 0, true);
                } else {
                    self.change_submenu(context, EXTRA_PETS);
                    self.button_selected(context, 2, -1, 0, true);
                }
                true
            }
            EXTRA_TAILS..=EXTRA_EXELLER => {
                // Survivor pages have their tab row at bid 3, EXE pages at bid 2.
                let tab_row = if self.submenu <= EXTRA_SALLY { 3 } else { 2 };
                if bid == 0 && bcolumn <= -1 {
                    if right && self.submenu == EXTRA_EXELLER {
                        // Only this wrap selects a button, and without the raise animation.
                        self.change_submenu(context, EXTRA_TAILS);
                        self.button_selected(context, 0, -1, 0, false);
                        return false;
                    }
                    self.turn_page(context, EXTRA_TAILS, EXTRA_EXELLER, right);
                } else if bid == tab_row {
                    if right {
                        self.change_submenu(context, EXTRA_SKINS1);
                        self.button_selected(context, 3, -1, 0, true);
                    } else {
                        self.change_submenu(context, EXTRA_ACHIEVEMENTS);
                        self.button_selected(context, 0, -1, 0, true);
                    }
                }
                false
            }
            EXTRA_SKINS1 | EXTRA_SKINS2 | EXTRA_SKINS3 => self.skins_page_listed(context, bid, right),
            EXTRA_PETS | EXTRA_PETS2 | EXTRA_PETS3 => {
                if bid == PET_PAGE_ROW {
                    self.turn_pet_page(context, right);
                    return false;
                }
                if bid != 2 {
                    return false;
                }
                if right {
                    self.change_submenu(context, EXTRA_BACK);
                    return true;
                }
                self.change_submenu(context, EXTRA_SKINS1);
                self.button_selected(context, 3, -1, 0, true);
                false
            }
            _ => false,
        }
    }

    /// buttonListed for the three lobby icon pages. The original counts the move as
    /// used up for the tab row on the first and last page, and for the page row on
    /// the middle page only.
    fn skins_page_listed(&mut self, context: &mut Context, bid: i32, right: bool) -> bool {
        let page = self.submenu;
        let tab_row = if page == EXTRA_SKINS3 { 2 } else { 3 };
        if bid == tab_row {
            if right {
                self.change_submenu(context, EXTRA_PETS);
                self.button_selected(context, 2, -1, 0, true);
            } else {
                self.change_submenu(context, EXTRA_TAILS);
                self.button_selected(context, 3, -1, 0, true);
            }
            return page != EXTRA_SKINS2;
        }
        if bid != 0 {
            return false;
        }
        let (next, previous) = match page {
            EXTRA_SKINS1 => (EXTRA_SKINS2, EXTRA_SKINS3),
            EXTRA_SKINS2 => (EXTRA_SKINS3, EXTRA_SKINS1),
            _ => (EXTRA_SKINS1, EXTRA_SKINS2),
        };
        self.change_submenu(context, if right { next } else { previous });
        self.button_selected(context, 0, -1, 0, true);
        page == EXTRA_SKINS2
    }

    /// Next or previous page in `first..=last`. Returns false when it wrapped around.
    fn turn_page(&mut self, context: &mut Context, first: i32, last: i32, forward: bool) -> bool {
        if forward && self.submenu + 1 > last {
            self.change_submenu(context, first);
            return false;
        }
        if !forward && self.submenu - 1 < first {
            self.change_submenu(context, last);
            return false;
        }
        let next = if forward { self.submenu + 1 } else { self.submenu - 1 };
        self.change_submenu(context, next);
        true
    }

    /// with(obj_menu_gif) image_index = 0
    fn restart_gifs(&mut self) {
        for widget in &mut self.widgets {
            if matches!(widget.kind, Kind::Gif) {
                widget.builtins.image_index = 0.0;
            }
        }
    }

    /// buttonSelected: selects the button at row `bid`, column `bcolumn` (-1: any)
    /// on the current page. `direction` is nonzero for sideways moves.
    pub(super) fn button_selected(&mut self, context: &mut Context, bid: i32, mut bcolumn: i32, direction: i32, animate: bool) {
        if self.waiting_for_key {
            return;
        }
        self.selected_row = bid;

        // A text box on the selected row starts editing its text.
        let submenu = self.submenu;
        let textbox_text = self.widgets.iter().find_map(|widget| match &widget.kind {
            Kind::Textbox(textbox) if textbox.bid == bid && textbox.submenu == submenu => Some(textbox.text.clone()),
            _ => None,
        });
        if let Some(text) = textbox_text {
            context.input.keyboard_string = text;
        }

        if self.submenu == JOIN && bid == JOIN_BOTTOM_ROW {
            if bcolumn >= JOIN_BOTTOM_COLUMNS {
                bcolumn = 0;
            }
            self.columns = JOIN_BOTTOM_COLUMNS;
        }

        let mut found = false;
        for widget in &mut self.widgets {
            let Kind::Button(button) = &mut widget.kind else { continue };
            let other_column = button.bcolumn != bcolumn && button.bcolumn != -1 && bcolumn != -1;
            if button.submenu != submenu || other_column || button.bid != bid {
                widget.builtins.image_blend = C_WHITE;
                continue;
            }
            // A whole-row button stays unselected when the move is sideways; the
            // buttons after it keep their colours.
            if button.bcolumn == -1 && direction != 0 && self.selected_column == -1 {
                return;
            }
            if bcolumn == -1 && button.bcolumn != -1 && button.bcolumn != 0 {
                continue;
            }
            // Later matches keep whatever colour they had.
            if found {
                continue;
            }
            self.selected_column = button.bcolumn;
            bcolumn = button.bcolumn;
            found = true;
            // Skin cards colour themselves when drawn.
            if !matches!(button.kind, ButtonKind::Skin(_)) {
                widget.builtins.image_blend = SELECTED_BLEND;
            }
            if animate {
                button.offset = SELECTION_RAISE;
            }
        }
    }

    /// buttonPressed
    pub(super) fn button_pressed(&mut self, context: &mut Context, bid: i32, bcolumn: i32) {
        if self.pressed_this_step || self.waiting_for_key {
            return;
        }
        if self.selected_row != bid {
            self.button_selected(context, bid, bcolumn, 0, true);
        }
        self.pressed_this_step = true;

        let submenu = self.submenu;
        let mut pressed = None;
        for (index, widget) in self.widgets.iter_mut().enumerate() {
            let Kind::Button(button) = &mut widget.kind else { continue };
            let other_column = button.bcolumn != bcolumn && button.bcolumn != -1 && bcolumn != -1;
            if button.submenu != submenu || other_column || button.bid != bid {
                widget.builtins.image_blend = C_WHITE;
                continue;
            }
            pressed = Some(index);
            break;
        }
        if let Some(index) = pressed {
            // Customization cards do their own thing (their click functions) and nothing else.
            if self.click_card(context, index) {
                return;
            }
            if let Kind::Button(button) = &mut self.widgets[index].kind {
                if let ButtonKind::Toggle { on, .. } = &mut button.kind {
                    *on = !*on;
                }
            }
        }

        context.audio.play(sound::SND_MENU_PRESS, false);
        self.press_on_page(context, bid, bcolumn);
    }

    /// Where the back keys lead: the main page, except on the Singleplayer pages, which
    /// go back one step as their back buttons do.
    fn previous_page(&self) -> i32 {
        match self.submenu {
            SINGLE_CHARACTERS => SINGLE_MAPS,
            SINGLE_MAPS => JOIN,
            _ => NONE,
        }
    }

    /// What the pressed button of the page picks, for the Singleplayer pages. The row
    /// and column are matched as buttonPressed matches them, so a selection that is in
    /// no column yet presses the first button of its row.
    pub(super) fn pressed_kind(&self, bid: i32, bcolumn: i32) -> Option<ButtonKind> {
        self.widgets.iter().find_map(|widget| match &widget.kind {
            Kind::Button(button)
                if button.submenu == self.submenu
                    && button.bid == bid
                    && !(button.bcolumn != bcolumn && button.bcolumn != -1 && bcolumn != -1) =>
            {
                match button.kind {
                    ButtonKind::Level { index } => Some(ButtonKind::Level { index }),
                    ButtonKind::Character { character, exe_character, demonized } => Some(ButtonKind::Character { character, exe_character, demonized }),
                    ButtonKind::RandomCharacter => Some(ButtonKind::RandomCharacter),
                    ButtonKind::Freecam => Some(ButtonKind::Freecam),
                    _ => None,
                }
            }
            _ => None,
        })
    }

    /// What the Singleplayer page picked (NetClient::choose_alone), on the chosen map.
    fn start_alone(&mut self, context: &mut Context, character: i32, exe_character: i32, demonized: bool, freecam: bool) {
        context.net.choose_alone((character, exe_character, demonized), freecam);
        self.next_room = crate::client::levels::LEVELS.get(self.chosen_level).map(|level| level.room);
    }

    /// The click functions of obj_menu_skin, obj_menu_icon and obj_menu_pet.
    /// Returns false when the button is not a card.
    fn click_card(&mut self, context: &mut Context, index: usize) -> bool {
        let Kind::Button(button) = &self.widgets[index].kind else { return false };
        if !matches!(button.kind, ButtonKind::Skin(_) | ButtonKind::Icon { .. } | ButtonKind::Pet(_)) {
            return false;
        }
        let (bid, bcolumn) = (button.bid, button.bcolumn);
        self.button_selected(context, bid, bcolumn, 0, true);
        context.audio.play(sound::SND_MENU_PRESS, false);

        let Kind::Button(button) = &mut self.widgets[index].kind else { return false };
        match &mut button.kind {
            ButtonKind::Skin(skin) => skin.click(&mut context.unlockables),
            ButtonKind::Pet(pet) => pet.click(&mut context.unlockables),
            ButtonKind::Icon { index } => {
                context.unlockables.lobby_icon = *index;
                context.unlockables.save();
                // The icon's click plays the press sound a second time.
                context.audio.play(sound::SND_MENU_PRESS, false);
            }
            _ => {}
        }
        true
    }

    /// The switch(submenu) of buttonPressed: what the pressed button does on this page.
    fn press_on_page(&mut self, context: &mut Context, bid: i32, bcolumn: i32) {
        match self.submenu {
            NONE => match bid {
                0 => self.change_submenu(context, JOIN),
                1 => {
                    self.change_submenu(context, OPTIONS_GENERAL);
                    self.button_selected(context, 4, -1, 0, false);
                }
                2 => self.change_submenu(context, EXTRA_ACHIEVEMENTS),
                3 => {
                    self.change_submenu(context, MECH_SPIN);
                    self.button_selected(context, 1, -1, 0, false);
                }
                4 => context.quit = true,
                _ => {}
            },
            SINGLE_MAPS => match self.pressed_kind(bid, bcolumn) {
                Some(ButtonKind::Level { index }) => {
                    self.chosen_level = index;
                    self.change_submenu(context, SINGLE_CHARACTERS);
                }
                _ => self.change_submenu(context, JOIN),
            },
            SINGLE_CHARACTERS => match self.pressed_kind(bid, bcolumn) {
                Some(ButtonKind::Character { character, exe_character, demonized }) => self.start_alone(context, character, exe_character, demonized, false),
                Some(ButtonKind::RandomCharacter) => {
                    let (character, exe_character, demonized) = crate::client::net::random_alone_pick();
                    self.start_alone(context, character, exe_character, demonized, false);
                }
                // The free camera has no player to be, so the character it carries is
                // only what the level builds its room around (see sandbox_player).
                Some(ButtonKind::Freecam) => self.start_alone(context, 1, -1, false, true),
                _ => self.change_submenu(context, SINGLE_MAPS),
            },
            JOIN => {
                if bid == 0 {
                    let typed_address = self.widgets.iter().find_map(|widget| match &widget.kind {
                        Kind::Textbox(textbox) if textbox.bid == bid && textbox.submenu == JOIN => Some(textbox.text.clone()),
                        _ => None,
                    });
                    if let Some(address) = typed_address {
                        self.net_join(context, &address);
                    }
                } else if bid == JOIN_BOTTOM_ROW && bcolumn == SINGLEPLAYER_COLUMN {
                    // Singleplayer, which the original has no button for: the map, then the character.
                    self.change_submenu(context, SINGLE_MAPS);
                } else if bid == JOIN_BOTTOM_ROW {
                    self.change_submenu(context, NONE);
                }
            }
            OPTIONS_GENERAL => {
                if bid == 2 {
                    context.options.show_ping = self.toggle_state("ping");
                    context.options.save();
                }
                if bid == 3 {
                    context.options.show_fps = self.toggle_state("fps");
                    context.options.save();
                }
                if bcolumn == 1 {
                    if bid == 4 {
                        // The touch controls editor (Android only) is not ported.
                        self.change_submenu(context, OPTIONS_CONTROLS);
                        self.button_selected(context, 10, -1, 0, false);
                    }
                    if bid == 5 {
                        self.change_submenu(context, NONE);
                    }
                }
            }
            OPTIONS_CONTROLS => self.press_on_controls_page(context, bid, bcolumn),
            OPTIONS_BACK => match bid {
                1 => {
                    self.change_submenu(context, OPTIONS_GENERAL);
                    self.button_selected(context, 4, -1, 0, false);
                }
                2 => {
                    self.change_submenu(context, OPTIONS_CONTROLS);
                    self.button_selected(context, 8, -1, 0, false);
                }
                _ => self.change_submenu(context, NONE),
            },
            ABOUT_CREDITS => match bid {
                1 => self.change_submenu(context, NONE),
                2 => self.open_character_pages(context),
                3 => self.open_mechanics_pages(context),
                _ => {}
            },
            ABOUT_BACK => match bid {
                0 => self.change_submenu(context, NONE),
                1 => self.change_submenu(context, ABOUT_CREDITS),
                2 => self.open_character_pages(context),
                3 => self.open_mechanics_pages(context),
                _ => {}
            },
            MECH_SPIN..=MECH_DIFF => {
                match bid {
                    2 => self.open_character_pages(context),
                    3 => self.change_submenu(context, ABOUT_CREDITS),
                    4 => self.change_submenu(context, NONE),
                    _ => {}
                }
                // The page arrows; the page may have changed just above, as in the original.
                if bcolumn == 1 && self.turn_page(context, MECH_SPIN, MECH_DIFF, bid == 1) {
                    self.restart_gifs();
                }
            }
            CHAR_TAILS..=CHAR_EXELLER => {
                match bid {
                    2 => self.open_mechanics_pages(context),
                    3 => self.change_submenu(context, ABOUT_CREDITS),
                    4 => self.change_submenu(context, NONE),
                    _ => {}
                }
                if bcolumn == 1 {
                    self.turn_page(context, CHAR_TAILS, CHAR_EXELLER, bid == 1);
                }
            }
            EXTRA_ACHIEVEMENTS | EXTRA_BACK => match bid {
                0 if self.submenu == EXTRA_BACK => self.change_submenu(context, NONE),
                1 => self.open_extra_page(context, EXTRA_TAILS),
                2 => self.open_extra_page(context, EXTRA_SKINS1),
                3 => self.open_extra_page(context, EXTRA_PETS),
                4 if self.submenu == EXTRA_ACHIEVEMENTS => self.change_submenu(context, NONE),
                4 => {
                    self.change_submenu(context, EXTRA_ACHIEVEMENTS);
                    self.select_achievements_tab(context);
                }
                _ => {}
            },
            EXTRA_TAILS..=EXTRA_EXELLER => {
                if bcolumn == 5 {
                    self.press_extra_tab(context, bid, EXTRA_SKINS1);
                }
                if bcolumn != 4 {
                    return;
                }
                if bid == 1 && self.submenu + 1 > EXTRA_EXELLER {
                    self.change_submenu(context, EXTRA_TAILS);
                    self.button_selected(context, 0, -1, 0, false);
                    return;
                }
                self.turn_page(context, EXTRA_TAILS, EXTRA_EXELLER, bid == 1);
            }
            EXTRA_SKINS1 | EXTRA_SKINS2 | EXTRA_SKINS3 => {
                if bcolumn == 5 && bid == 0 {
                    self.change_submenu(context, EXTRA_ACHIEVEMENTS);
                    self.select_achievements_tab(context);
                    return;
                }
                if bcolumn == 5 {
                    self.press_extra_tab(context, bid, EXTRA_TAILS);
                }
                if bcolumn != PAGE_BUTTONS_COLUMN {
                    return;
                }
                let (with_bid_4, other) = match self.submenu {
                    EXTRA_SKINS1 => (EXTRA_SKINS2, EXTRA_SKINS3),
                    EXTRA_SKINS2 => (EXTRA_SKINS1, EXTRA_SKINS3),
                    _ => (EXTRA_SKINS1, EXTRA_SKINS2),
                };
                self.change_submenu(context, if bid == 4 { with_bid_4 } else { other });
            }
            EXTRA_PETS | EXTRA_PETS2 | EXTRA_PETS3 => {
                if bcolumn == PAGE_BUTTONS_COLUMN {
                    self.open_other_pet_page(context, bid);
                    return;
                }
                if bcolumn != 5 {
                    return;
                }
                match bid {
                    0 => {
                        self.change_submenu(context, EXTRA_ACHIEVEMENTS);
                        self.select_achievements_tab(context);
                    }
                    1 => self.open_extra_page(context, EXTRA_TAILS),
                    2 => self.open_extra_page(context, EXTRA_SKINS1),
                    4 => self.change_submenu(context, NONE),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn open_character_pages(&mut self, context: &mut Context) {
        self.change_submenu(context, CHAR_TAILS);
        self.button_selected(context, 1, 0, -1, false);
    }

    fn open_mechanics_pages(&mut self, context: &mut Context) {
        self.change_submenu(context, MECH_SPIN);
        self.button_selected(context, 1, 0, -1, false);
    }

    /// The page row of the pet pages: the next or previous page, wrapping around.
    fn turn_pet_page(&mut self, context: &mut Context, forward: bool) {
        let current = PET_PAGES.iter().position(|&page| page == self.submenu).unwrap_or(0);
        let next = if forward { current + 1 } else { current + PET_PAGES.len() - 1 } % PET_PAGES.len();
        self.change_submenu(context, PET_PAGES[next]);
        self.button_selected(context, PET_PAGE_ROW, -1, 0, true);
    }

    /// The page buttons of a pet page, as on the icon pages: bid 4 opens the first
    /// of the two other pages, bid 5 the second.
    fn open_other_pet_page(&mut self, context: &mut Context, bid: i32) {
        let submenu = self.submenu;
        let mut other_pages = PET_PAGES.iter().filter(|&&page| page != submenu);
        let chosen = if bid == 4 { other_pages.next() } else { other_pages.nth(1) };
        if let Some(&page) = chosen {
            self.change_submenu(context, page);
        }
    }

    /// The tab row of the skin and icon pages. `second_tab` is the page bid 1 opens.
    fn press_extra_tab(&mut self, context: &mut Context, bid: i32, second_tab: i32) {
        match bid {
            0 => {
                self.change_submenu(context, EXTRA_ACHIEVEMENTS);
                self.button_selected(context, 0, -1, 0, false);
            }
            1 => self.open_extra_page(context, second_tab),
            2 => self.open_extra_page(context, EXTRA_PETS),
            4 => self.change_submenu(context, NONE),
            _ => {}
        }
    }

    /// The achievements tab, row 0 of its page. The original selected row 1 here when the
    /// tab was clicked from another page, which left the tab white.
    fn select_achievements_tab(&mut self, context: &mut Context) {
        self.button_selected(context, 0, -1, 0, false);
    }

    /// Opens a customization page with its tab row selected.
    fn open_extra_page(&mut self, context: &mut Context, page: i32) {
        let tab_row = if page == EXTRA_PETS { 2 } else { 3 };
        self.change_submenu(context, page);
        self.button_selected(context, tab_row, -1, 0, false);
    }

    fn press_on_controls_page(&mut self, context: &mut Context, bid: i32, bcolumn: i32) {
        if bcolumn == 2 {
            if bid == 9 {
                self.change_submenu(context, OPTIONS_GENERAL);
                self.button_selected(context, 4, -1, 0, false);
            } else if bid == 10 {
                self.change_submenu(context, NONE);
            }
            return;
        }
        if bid == 10 {
            return;
        }
        if bid == 9 {
            let defaults = KeyBindings::default();
            for (panel, key) in [
                ("left", defaults.left),
                ("right", defaults.right),
                ("up", defaults.up),
                ("down", defaults.down),
                ("jump", defaults.jump),
                ("special1", defaults.special1),
                ("special2", defaults.special2),
                ("emotion1", defaults.emotion1),
                ("emotion2", defaults.emotion2),
                ("emotion3", defaults.emotion3),
                ("emotion4", defaults.emotion4),
                ("hidegui", defaults.hide_gui),
                ("playerlist", defaults.player_list),
            ] {
                self.bind_changed(context, panel, key.0);
            }
            return;
        }

        self.waiting_for_key = true;
        let panel = match (bcolumn, bid) {
            (0, 0) => Some("up"),
            (0, 1) => Some("down"),
            (0, 2) => Some("left"),
            (0, 3) => Some("right"),
            (1, 0) => Some("jump"),
            (1, 1) => Some("special1"),
            (1, 2) => Some("special2"),
            (1, 3) => Some("hidegui"),
            (0 | 1, _) => None,
            (_, 4) => Some("playerlist"),
            (_, 5) => Some("emotion1"),
            (_, 6) => Some("emotion2"),
            (_, 7) => Some("emotion3"),
            (_, 8) => Some("emotion4"),
            _ => None,
        };
        // A row without a key keeps the panel chosen before, as bindBtn did.
        if let Some(panel) = panel {
            self.binding_panel = Some(panel.to_string());
        }
        if let Some(panel) = self.binding_panel.clone() {
            self.set_panel_text(&panel, "press...");
        }
    }

    /// changeSubmenu
    pub(super) fn change_submenu(&mut self, context: &mut Context, submenu: i32) {
        self.submenu = submenu;
        for name in ALL_MENU_LAYERS {
            self.room.set_layer_visible(name, false);
        }
        let layout = submenus::layout(submenu);
        if let Some(bars) = self.room.background_mut("Bars") {
            bars.sprite = Some(sprite::SPR_MENU_BARS);
            bars.image_index = layout.bars_frame;
        }
        self.rows = layout.rows;
        self.columns = layout.columns;
        self.grid = layout.grid;
        for name in layout.layers {
            self.room.set_layer_visible(name, true);
        }
        self.arrange_shared_rows(context, submenu);
        if let Some(character_sprite) = layout.character_sprite {
            self.set_character_picture(character_sprite);
        }
        if submenu == ABOUT_CREDITS || submenu == ABOUT_BACK {
            self.set_credits_y(CREDITS_START_Y);
        }
        if submenu == ABOUT_CREDITS {
            context.input.keyboard_string.clear();
        }
        self.button_selected(context, 0, -1, 0, false);
    }

    /// The shared Extras rows join the page being opened, each button at the row and
    /// column its copy on that page had in the original, so presses and keys work as there.
    fn arrange_shared_rows(&mut self, context: &Context, page: i32) {
        let mut character_row_index = 0;
        for widget in &mut self.widgets {
            let Kind::Button(button) = &mut widget.kind else { continue };
            let Some(row) = button.shared_row else { continue };
            let frame = widget.builtins.image_index as usize;
            let place = match row {
                EXTRA_TABS => TAB_FRAMES.iter().position(|&tab| tab == frame).and_then(|tab| tab_place(page, tab)),
                EXTRA_PAGE_ROW => PAGE_NUMBER_FRAMES.iter().position(|&number| number == frame).and_then(|number| page_number_place(page, number)),
                _ => {
                    let role = character_row_index;
                    character_row_index += 1;
                    let Some(shown) = (EXTRA_TAILS..=EXTRA_EXELLER).contains(&page).then(|| (page - EXTRA_TAILS) as usize) else {
                        button.submenu = row;
                        continue;
                    };
                    let count = CHARACTER_NAME_FRAMES.len();
                    // Left the previous character, in the middle this one, right the next.
                    let (name, place) = match role {
                        0 => (CHARACTER_NAME_FRAMES[(shown + count - 1) % count], (0, PAGE_BUTTONS_COLUMN)),
                        1 => (CHARACTER_NAME_FRAMES[shown], (0, -1)),
                        _ => (CHARACTER_NAME_FRAMES[(shown + 1) % count], (1, PAGE_BUTTONS_COLUMN)),
                    };
                    widget.builtins.image_index = name as f64;
                    if role == 1 {
                        let width = context.canvas.menu_text_width(super::widgets::BUTTON_LABELS[name]);
                        widget.builtins.x = CHARACTER_NAME_CENTRE - (width / 2.0).floor() + super::LABEL_LEFT_OF_ORIGIN;
                    }
                    Some(place)
                }
            };
            match place {
                Some((bid, bcolumn)) => {
                    button.submenu = page;
                    button.bid = bid;
                    button.bcolumn = bcolumn;
                }
                None => button.submenu = row,
            }
        }
    }

    /// textChanged: the join page remembers the address, the options page the nickname.
    pub(super) fn text_changed(&mut self, context: &mut Context, text: &str) {
        if self.submenu == JOIN {
            context.options.ip = text.to_string();
            context.options.save();
        }
        if self.submenu == OPTIONS_GENERAL {
            context.options.nickname = validate_nickname(text);
            context.options.save();
        }
    }

    /// bindChanged
    fn bind_changed(&mut self, context: &mut Context, panel: &str, key: KeyCode) {
        self.set_panel_text(panel, key_name(key));
        if let Some(binding) = context.options.keys.by_panel_name(panel) {
            *binding = BoundKey(key);
        }
        context.options.save();
    }

    /// net_join: remembers the server and goes to the connecting screen.
    fn net_join(&mut self, context: &mut Context, host: &str) {
        if context.net.join(host, None) {
            self.next_room = Some(RoomId::Connecting);
        }
    }

    /// obj_config Alarm_0: the options page shows the saved options.
    pub(super) fn show_options(&mut self, context: &mut Context) {
        let options = &mut context.options;
        for widget in &mut self.widgets {
            match &mut widget.kind {
                Kind::Button(button) => {
                    if let ButtonKind::Toggle { on, .. } = &mut button.kind {
                        match button.tid.as_str() {
                            "ping" => *on = options.show_ping,
                            "fps" => *on = options.show_fps,
                            _ => {}
                        }
                    }
                }
                Kind::Textbox(textbox) => match textbox.tid.as_str() {
                    "ip" => textbox.text = options.ip.clone(),
                    "nickname" => textbox.text = options.nickname.clone(),
                    _ => {}
                },
                Kind::Textpanel { tid, text } => {
                    if let Some(key) = options.keys.by_panel_name(tid) {
                        *text = key_name(key.0).to_string();
                    }
                }
                _ => {}
            }
        }
    }

    fn toggle_state(&self, tid: &str) -> bool {
        self.widgets
            .iter()
            .find_map(|widget| match &widget.kind {
                Kind::Button(button) if button.tid == tid => match button.kind {
                    ButtonKind::Toggle { on, .. } => Some(on),
                    _ => None,
                },
                _ => None,
            })
            .unwrap_or(false)
    }

    fn set_panel_text(&mut self, panel: &str, new_text: &str) {
        for widget in &mut self.widgets {
            if let Kind::Textpanel { tid, text } = &mut widget.kind {
                if tid == panel {
                    *text = new_text.to_string();
                    return;
                }
            }
        }
    }

    fn set_character_picture(&mut self, character_sprite: crate::core::resources::SpriteId) {
        for widget in &mut self.widgets {
            if matches!(widget.kind, Kind::CharacterPicture) {
                widget.builtins.sprite = Some(character_sprite);
            }
        }
    }

    fn set_credits_y(&mut self, y: f64) {
        for widget in &mut self.widgets {
            if matches!(widget.kind, Kind::Credits) {
                widget.builtins.y = y;
            }
        }
    }
}

/// Where Extras tab `tab` (by TAB_FRAMES) sat on `page`: the tab of the page's own section
/// is its whole tab row, the others answer only the mouse (column 5) on the card pages.
/// On the achievements and back pages the tab rows run on without the 4 the cut taunts
/// tab had. The mouse-only tabs keep skipping 3, which is the survivor pages' tab row.
fn tab_place(page: i32, tab: usize) -> Option<(i32, i32)> {
    let places = match page {
        EXTRA_ACHIEVEMENTS => [(0, -1), (1, -1), (2, -1), (3, -1), (4, -1)],
        EXTRA_BACK => [(4, -1), (1, -1), (2, -1), (3, -1), (0, -1)],
        EXTRA_TAILS..=EXTRA_SALLY => [(0, 5), (3, -1), (1, 5), (2, 5), (4, 5)],
        EXTRA_EXE..=EXTRA_EXELLER => [(0, 5), (2, -1), (1, 5), (2, 5), (4, 5)],
        EXTRA_SKINS1 | EXTRA_SKINS2 => [(0, 5), (1, 5), (3, -1), (2, 5), (4, 5)],
        EXTRA_SKINS3 => [(0, 5), (1, 5), (2, -1), (2, 5), (4, 5)],
        EXTRA_PETS | EXTRA_PETS2 | EXTRA_PETS3 => [(0, 5), (1, 5), (2, 5), (2, -1), (4, 5)],
        _ => return None,
    };
    places.get(tab).copied()
}

/// Where page number `number` (0 to 2) sat on an icon or pet page: its own number is the
/// page row, the other two answer the mouse as the first (row 4) and second (row 5) other page.
fn page_number_place(page: i32, number: usize) -> Option<(i32, i32)> {
    let (pages, row) = if PET_PAGES.contains(&page) { (PET_PAGES, PET_PAGE_ROW) } else { ([EXTRA_SKINS1, EXTRA_SKINS2, EXTRA_SKINS3], 0) };
    let current = pages.iter().position(|&shown| shown == page)?;
    if number == current {
        return Some((row, -1));
    }
    let rank = if number < current { number } else { number - 1 };
    Some((4 + rank as i32, PAGE_BUTTONS_COLUMN))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::tiled;

    #[test]
    fn shared_rows_take_the_places_their_copies_had() {
        // ExtraSkins2 of the original map: page 1 at row 4 and page 3 at row 5, column 4.
        assert_eq!(page_number_place(EXTRA_SKINS2, 0), Some((4, PAGE_BUTTONS_COLUMN)));
        assert_eq!(page_number_place(EXTRA_SKINS2, 1), Some((0, -1)));
        assert_eq!(page_number_place(EXTRA_PETS3, 2), Some((PET_PAGE_ROW, -1)));
        assert_eq!(page_number_place(EXTRA_PETS3, 1), Some((5, PAGE_BUTTONS_COLUMN)));
        // ExtraExe: its skins tab is its tab row 2, icons answer the mouse at row 1.
        assert_eq!(tab_place(EXTRA_EXE, 1), Some((2, -1)));
        assert_eq!(tab_place(EXTRA_EXE, 2), Some((1, 5)));
        assert_eq!(tab_place(JOIN, 0), None);
    }

    /// The Options pages tell their bottom buttons apart by (bid, bcolumn): the original's
    /// map gave the Controls page's "general" tab the same pair as its "back", so pressing
    /// it left for the main menu instead of the other Options page (its own code reads 9
    /// for that tab). The map here says 9, and nothing else on the page answers to it.
    #[test]
    fn the_general_tab_of_the_controls_page_is_not_the_back_button() {
        let map = menu_map();
        let mut bottom_row = buttons(&map.layers, "OptionsControls");
        bottom_row.retain(|&(_, _, y)| y == 244);
        bottom_row.sort();
        assert_eq!(bottom_row.len(), 3, "general, controls and back: {bottom_row:?}");
        let pairs: Vec<(i64, i64)> = bottom_row.iter().map(|&(bid, bcolumn, _)| (bid, bcolumn)).collect();
        assert_eq!(pairs.len(), pairs.iter().collect::<std::collections::HashSet<_>>().len(), "each one its own: {pairs:?}");
        assert!(pairs.contains(&(9, 2)), "the general tab, which press_on_controls_page sends to the other page: {pairs:?}");
        assert!(pairs.contains(&(10, 2)), "back to the main menu: {pairs:?}");
    }

    /// The Singleplayer character page: three rows of six picks, each in its own
    /// place, covering the six survivors, the same six demonized, the four EXE, the
    /// random pick and the free camera. The page's layout (submenus.rs) counts on it.
    #[test]
    fn the_singleplayer_character_page_has_a_cell_for_every_pick() {
        let map = menu_map();
        let picks = picks(&map.layers, "SingleCharacters");

        let places: Vec<(i64, i64)> = picks.iter().map(|&(bid, bcolumn, _)| (bid, bcolumn)).collect();
        assert_eq!(places.len(), 18, "six survivors, six demons, four EXE, random and the free camera: {picks:?}");
        assert_eq!(places.len(), places.iter().collect::<std::collections::HashSet<_>>().len(), "each one its own place: {places:?}");
        for (bid, bcolumn) in &places {
            assert!((0..3).contains(bid) && (0..6).contains(bcolumn), "inside the three rows of six: {places:?}");
        }
        let count = |wanted: &str| picks.iter().filter(|(_, _, kind)| kind == wanted).count();
        assert_eq!((count("survivor"), count("demon"), count("exe")), (6, 6, 4), "{picks:?}");
        assert_eq!((count("random"), count("freecam")), (1, 1), "{picks:?}");
    }

    fn menu_map() -> tiled::Map {
        tiled::load_map(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Resources/Maps/menu.tmj")).unwrap()
    }

    /// The objects of the layer named `name`, looked for inside groups too.
    fn layer_objects<'a>(layers: &'a [tiled::Layer], name: &str) -> Vec<&'a tiled::Object> {
        let mut found = Vec::new();
        for layer in layers {
            match layer {
                tiled::Layer::Group(group) => found.extend(layer_objects(&group.layers, name)),
                tiled::Layer::ObjectGroup(group) if group.name == name => found.extend(&group.objects),
                _ => {}
            }
        }
        found
    }

    fn number(object: &tiled::Object, name: &str) -> Option<i64> {
        tiled::find_property(&object.properties, name).and_then(|value| value.as_f64()).map(|value| value as i64)
    }

    /// (bid, bcolumn, what it picks) of every button of `layer` that picks something.
    fn picks(layers: &[tiled::Layer], layer: &str) -> Vec<(i64, i64, String)> {
        let mut found = Vec::new();
        for object in layer_objects(layers, layer) {
            let pick = tiled::find_property(&object.properties, "pick").and_then(|value| value.as_str().map(str::to_string));
            let demon = tiled::find_property(&object.properties, "demon").is_some();
            let kind = match (number(object, "character"), demon, pick) {
                (_, _, Some(pick)) => pick,
                (Some(0), _, None) => "exe".to_string(),
                (Some(_), true, None) => "demon".to_string(),
                (Some(_), false, None) => "survivor".to_string(),
                (None, _, None) => continue,
            };
            found.push((number(object, "bid").unwrap_or(0), number(object, "bcolumn").unwrap_or(-1), kind));
        }
        found
    }

    /// (bid, bcolumn, y) of every button of `layer`.
    fn buttons(layers: &[tiled::Layer], layer: &str) -> Vec<(i64, i64, i64)> {
        layer_objects(layers, layer)
            .into_iter()
            .filter(|object| object.class == "obj_menu_button")
            .map(|object| (number(object, "bid").unwrap_or(0), number(object, "bcolumn").unwrap_or(-1), object.y as i64))
            .collect()
    }
}
