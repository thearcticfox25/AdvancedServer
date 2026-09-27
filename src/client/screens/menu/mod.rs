//! room_menu: the main menu with its pages (obj_menu, obj_menu_* widgets,
//! obj_menu_particle). Navigation lives in navigation.rs, the pages in
//! submenus.rs, the customization cards in skins.rs.

mod map_grid;
mod navigation;
mod skins;
mod submenus;
mod widgets;

use super::count_down_alarm;
use super::fades::WhiteFlash;
use super::character_art;
use crate::client::canvas::{make_color_rgb, Canvas, VIEW_HEIGHT, VIEW_WIDTH};
use crate::client::room::Room;
use crate::client::text::{draw_text, text_width};
use crate::client::Context;
use anyhow::Result;
use macroquad::input::KeyCode;
use macroquad::rand::{self, ChooseRandom};
use crate::core::resources::names::{sound, sprite};
use crate::core::collision::sprite_bbox;
use crate::core::objects::ids::ObjectId;
use crate::core::rooms::ids::RoomId;
use crate::core::world::{C_GREY, C_WHITE};
use widgets::{Builtins, Button, ButtonKind, Kind, Widget};
use crate::core::config::step;

/// image_blend of the selected card (#910000) and of an equipped card.
const SELECTED_CARD_BLEND: u32 = make_color_rgb(0x91, 0x00, 0x00);
const EQUIPPED_CARD_BLEND: u32 = make_color_rgb(0x00, 0x80, 0x00);
/// Buttons of the DISABLED page are drawn grey (#666666).
const DISABLED_BUTTON_BLEND: u32 = 0x666666;
/// Card layout: the picture is centered 32 px right of the card's corner.
const CARD_CENTER: f64 = 32.0;
/// A button's words start where spr_menu_buttons drew them: from the left edge of the
/// frame (its origin is 90, 8) and 4 px down into it.
pub(super) const LABEL_LEFT_OF_ORIGIN: f64 = 90.0;
const LABEL_ABOVE_ORIGIN: f64 = 4.0;
/// The letters are 8 px tall. A button answers the mouse a little around its words,
/// more to the sides, where the buttons stand far apart; 4 px up and down is as much
/// as rows 16 px apart allow without one button's area reaching into the next.
const LABEL_INK_HEIGHT: f64 = 8.0;
const LABEL_CLICK_MARGIN_X: f64 = 10.0;
const LABEL_CLICK_MARGIN_Y: f64 = 4.0;

/// What a Singleplayer page shows behind its buttons.
enum PageLook {
    /// The map vote's bars, and the chosen map's picture and name (see map_grid.rs);
    /// with Back chosen the menu's own background shows behind the same bars.
    MapVote { map: Option<(usize, String)> },
    /// The character select stage with this character (an index into CHARACTER_ART).
    CharacterSelect { art: usize },
}

/// The layers of the menu's own background, which a Singleplayer page's look replaces.
const MENU_BACKGROUND_LAYERS: [&str; 2] = ["Background", MENU_BARS_LAYER];
const MENU_BARS_LAYER: &str = "Bars";

/// obj_menu_particlespawner: 20 particles at random places, drawn at depth 1999.
const PARTICLE_COUNT: usize = 20;
const PARTICLE_DEPTH: i32 = 1999;
const PARTICLE_SPEEDS: [f64; 4] = [0.1, 0.2, 0.3, 0.4];
const PARTICLE_WRAP_LEFT: f64 = -16.0;


/// obj_menu_version: the words before the version; "&" after them is the colour code
/// that paints the version itself purple.
const VERSION_PREFIX: &str = "v ";

/// obj_menu_warning: its note stands this far left of the "!".
const POPUP_OFFSET_X: f64 = -4.0;

/// obj_menu_credits: scrolling speed and how far past the room it wraps.
const CREDITS_KEY_SCROLL: f64 = 2.0;
const CREDITS_IDLE_SCROLL: f64 = 0.25;
const CREDITS_WRAP_MARGIN: f64 = 38.0;
const CREDITS_LINE_HEIGHT: f64 = 8.0;

/// Where the lines sit in a row's frame.
const ACHIEVEMENT_TEXT_LEFT: f64 = 7.0;
const ACHIEVEMENT_LINE_HEIGHT: f64 = 8.0;
const ACHIEVEMENT_SCROLL_STEP: f64 = 8.0;
const ACHIEVEMENT_WHEEL_STEPS: f64 = 6.0;
const ACHIEVEMENT_SCROLL_SMOOTHING: f64 = 0.2;
const ACHIEVEMENT_LIST_TOP: f64 = 135.0;
/// The rows are seen between the two arrows: at the start of the list the first row
/// stands this far below the upper arrow, at its end the last row this far above the
/// lower one. Page Up and Page Down move by the height seen between them.
const ACHIEVEMENT_ARROW_GAP: f64 = 4.0;
const ACHIEVEMENT_UNLOCKED_BLEND: u32 = make_color_rgb(0x0F, 0xFF, 0x39);

pub struct Menu {
    room: Room,
    /// The placed obj_menu_* instances, in creation order.
    widgets: Vec<Widget>,
    particles: Vec<Particle>,
    /// scheme: up and down move between rows, left and right between columns.
    grid: bool,
    /// bSelected
    selected_row: i32,
    /// bCount
    rows: i32,
    /// bSelectedColumn
    selected_column: i32,
    /// bColumnCount
    columns: i32,
    /// buttonA: a button was pressed this step already.
    pressed_this_step: bool,
    submenu: i32,
    /// bind: the next key pressed becomes the binding shown by `binding_panel`.
    waiting_for_key: bool,
    /// bindBtn
    binding_panel: Option<String>,
    /// A selected text box takes the keyboard, so the jump and back keys type.
    ignore_buttons: bool,
    /// obj_menu alarm[0]: selects the first button.
    selection_alarm: i32,
    /// obj_config alarm[0]: shows the saved options.
    options_alarm: i32,
    next_room: Option<RoomId>,
    /// The map chosen on the Singleplayer page.
    chosen_level: usize,
    /// The Singleplayer page's cards: their scroll and the map pictures behind them.
    map_grid: map_grid::MapGrid,
    /// obj_lobby_white placed in room_menu: every way into the menu starts with a flash.
    flash: Option<WhiteFlash>,
}

struct Particle {
    x: f64,
    y: f64,
    start_y: f64,
    speed: f64,
    alpha: f64,
}

impl Menu {
    pub fn open(context: &mut Context) -> Result<Menu> {
        let room = Room::load(&context.maps_folder, RoomId::Menu, &context.canvas.sprites)?;
        let placed = room.placed_in_creation_order();
        let widgets = placed.iter().filter_map(|&(layer, instance)| widgets::build(layer, instance, &context.unlockables)).collect();
        let has_particles = placed.iter().any(|(_, instance)| instance.object == ObjectId::MenuParticlespawner);
        let particles = if has_particles { spawn_particles(&room) } else { Vec::new() };
        let flash = placed.iter().any(|(_, instance)| instance.object == ObjectId::LobbyWhite).then(WhiteFlash::new);
        // RoomCreationCode
        context.audio.play_music(sound::MUS_MENU);

        let mut menu = Menu {
            room,
            widgets,
            particles,
            grid: false,
            selected_row: 0,
            rows: 5,
            selected_column: 0,
            columns: 0,
            pressed_this_step: false,
            submenu: submenus::NONE,
            waiting_for_key: false,
            binding_panel: None,
            ignore_buttons: false,
            selection_alarm: 1,
            options_alarm: 1,
            next_room: None,
            chosen_level: 0,
            map_grid: map_grid::MapGrid::new(),
            flash,
        };
        menu.start_achievement_list(&context.canvas);
        Ok(menu)
    }

    pub fn tick(&mut self, context: &mut Context) -> Option<RoomId> {
        let current_time_ms = (macroquad::time::get_time() * 1000.0).floor();
        self.room.step(&context.canvas.sprites);
        for widget in &mut self.widgets {
            advance_animation(&mut widget.builtins, &context.canvas.sprites);
        }

        if count_down_alarm(&mut self.selection_alarm) {
            self.button_selected(context, 0, -1, 0, false);
        }
        if count_down_alarm(&mut self.options_alarm) {
            self.show_options(context);
        }
        self.scroll_achievements(context);

        self.step_navigation(context);
        self.step_widgets();
        if self.submenu == submenus::SINGLE_MAPS {
            let cards = self.map_cards(&context.canvas);
            self.map_grid.step(&context.canvas, &context.input, &cards);
        }
        for particle in &mut self.particles {
            particle.step(self.room.width, current_time_ms);
        }
        self.flash = self.flash.take().and_then(|mut flash| flash.step().then_some(flash));

        // Draw_76 (pre-draw)
        self.ignore_buttons = false;
        self.draw(context, current_time_ms);
        // Its Create sets depth -999999: above every layer of the room.
        if let Some(flash) = &self.flash {
            flash.draw(&mut context.canvas);
        }
        self.next_room.take()
    }

    fn step_widgets(&mut self) {
        for widget in &mut self.widgets {
            let Kind::Button(button) = &mut widget.kind else { continue };
            button.step(&mut widget.builtins.y);
            if let ButtonKind::Toggle { first_frame, on } = button.kind {
                widget.builtins.image_index = first_frame + if on { 1.0 } else { 0.0 };
            }
        }
    }

    /// Mouse_60 and Mouse_61 of obj_menu_achivements, and Page Up and Page Down, while
    /// the list is shown.
    fn scroll_achievements(&mut self, context: &Context) {
        if self.submenu != submenus::EXTRA_ACHIEVEMENTS {
            return;
        }
        let Some((top, bottom)) = self.achievement_window(&context.canvas) else { return };
        let page_steps = (bottom - top) / ACHIEVEMENT_SCROLL_STEP;
        let steps = if context.input.wheel_up() {
            ACHIEVEMENT_WHEEL_STEPS
        } else if context.input.wheel_down() {
            -ACHIEVEMENT_WHEEL_STEPS
        } else if context.input.pressed(KeyCode::PageUp) {
            page_steps
        } else if context.input.pressed(KeyCode::PageDown) {
            -page_steps
        } else {
            return;
        };
        self.move_achievement_list(&context.canvas, steps);
    }

    /// obj_menu_achivements.move: scrolls, but no further than the list's start and end.
    fn move_achievement_list(&mut self, canvas: &Canvas, steps: f64) {
        let Some((end, start)) = self.achievement_scroll_range(canvas) else { return };
        for widget in &mut self.widgets {
            let Kind::AchievementList { target_scroll, .. } = &mut widget.kind else { continue };
            *target_scroll = (*target_scroll + steps * ACHIEVEMENT_SCROLL_STEP).clamp(end, start);
        }
    }

    /// The list opens at its start, right below the upper arrow.
    fn start_achievement_list(&mut self, canvas: &Canvas) {
        let Some((_, start)) = self.achievement_scroll_range(canvas) else { return };
        for widget in &mut self.widgets {
            if let Kind::AchievementList { scroll, target_scroll } = &mut widget.kind {
                (*scroll, *target_scroll) = (start, start);
            }
        }
    }

    /// The space between the two arrows the rows are seen in: (top, bottom).
    fn achievement_window(&self, canvas: &Canvas) -> Option<(f64, f64)> {
        let arrows: Vec<crate::core::collision::Bbox> = self
            .widgets
            .iter()
            .filter(|widget| matches!(widget.kind, Kind::AchievementArrow))
            .filter_map(|widget| {
                let builtins = &widget.builtins;
                let sprite = builtins.sprite?;
                Some(sprite_bbox(canvas.sprites.get(sprite), builtins.x, builtins.y, builtins.xscale, builtins.yscale, builtins.angle))
            })
            .collect();
        let upper = arrows.iter().min_by(|a, b| a.top.total_cmp(&b.top))?;
        let lower = arrows.iter().max_by(|a, b| a.top.total_cmp(&b.top))?;
        Some((upper.bottom + ACHIEVEMENT_ARROW_GAP, lower.top - ACHIEVEMENT_ARROW_GAP))
    }

    /// The scroll at the end of the list and at its start, which the scroll stays between.
    fn achievement_scroll_range(&self, canvas: &Canvas) -> Option<(f64, f64)> {
        let (top, bottom) = self.achievement_window(canvas)?;
        let list_height = self.achievement_row_height(canvas)? * crate::client::achievements::ACHIEVEMENT_COUNT as f64;
        let start = top - ACHIEVEMENT_LIST_TOP;
        let end = bottom - list_height - ACHIEVEMENT_LIST_TOP;
        Some((end.min(start), start))
    }

    /// The height of one row: the frame the rows are drawn in.
    fn achievement_row_height(&self, canvas: &Canvas) -> Option<f64> {
        let list = self.widgets.iter().find(|widget| matches!(widget.kind, Kind::AchievementList { .. }))?;
        Some(canvas.sprites.get(list.builtins.sprite?).height as f64)
    }

    /// Layers from the back to the front; each visible layer draws its instances in
    /// creation order. A click handled while drawing can change pages mid-frame,
    /// and later layers see that, as in GameMaker.
    fn draw(&mut self, context: &mut Context, current_time_ms: f64) {
        // On the Singleplayer pages the chosen map or character brings the screen it is
        // chosen on in the rounds (the map vote, the character select stage) in place of
        // the menu's own background picture, bars and particles; back brings those back.
        let own_look = self.page_look();
        match &own_look {
            Some(PageLook::MapVote { map: Some((map, _)) }) => self.map_grid.draw_preview(&mut context.canvas, *map, current_time_ms),
            Some(PageLook::CharacterSelect { art }) => draw_character_select(&mut context.canvas, *art, current_time_ms),
            _ => {}
        }
        // Whether the look hides the menu's background picture (and its particles).
        let hides_background = matches!(own_look, Some(PageLook::MapVote { map: Some(_) } | PageLook::CharacterSelect { .. }));
        let map_name = match &own_look {
            Some(PageLook::MapVote { map }) => Some(map.as_ref().map(|(_, name)| name.clone())),
            _ => None,
        };
        let mut particles_drawn = hides_background;
        for layer_index in 0..self.room.layers.len() {
            let layer = &self.room.layers[layer_index];
            if !particles_drawn && layer.depth < PARTICLE_DEPTH {
                self.draw_particles(&mut context.canvas);
                particles_drawn = true;
            }
            if !layer.visible {
                continue;
            }
            // The map page's bars stand where the menu's are, chosen map or not.
            if let (Some(name), MENU_BARS_LAYER) = (&map_name, layer.name.as_str()) {
                self.map_grid.draw_bars(&mut context.canvas, name.as_deref());
                continue;
            }
            if hides_background && MENU_BACKGROUND_LAYERS.contains(&layer.name.as_str()) {
                continue;
            }
            self.room.draw_layer(&mut context.canvas, &self.room.layers[layer_index]);
            for widget_index in 0..self.widgets.len() {
                if self.widgets[widget_index].layer == layer_index {
                    self.draw_widget(context, widget_index, current_time_ms);
                }
            }
        }
        if !particles_drawn {
            self.draw_particles(&mut context.canvas);
        }
    }

    fn draw_particles(&self, canvas: &mut Canvas) {
        for particle in &self.particles {
            canvas.draw_sprite_ext(sprite::SPR_MENU_PARTICLE, 0.0, particle.x, particle.y, 1.0, 1.0, 0.0, C_WHITE, particle.alpha);
        }
    }

    /// The Singleplayer page's cards as the scrolling needs them.
    fn map_cards(&self, canvas: &Canvas) -> map_grid::Cards {
        let card_height = map_grid::MapGrid::card_height(canvas);
        let mut span = (f64::MAX, f64::MIN);
        let mut chosen = None;
        for (index, widget) in self.widgets.iter().enumerate() {
            let Kind::Button(button @ Button { kind: ButtonKind::Level { .. }, .. }) = &widget.kind else { continue };
            let top = button.rest_y - LABEL_ABOVE_ORIGIN;
            span = (span.0.min(top), span.1.max(top + card_height));
            if (button.bid, button.bcolumn) == (self.selected_row, self.selected_column) {
                chosen = Some((index, top));
            }
        }
        map_grid::Cards { span, chosen }
    }

    /// The look of a Singleplayer page while a map or a character is chosen on it.
    fn page_look(&self) -> Option<PageLook> {
        if self.submenu == submenus::SINGLE_MAPS {
            let map = match self.pressed_kind(self.selected_row, self.selected_column) {
                Some(ButtonKind::Level { index }) => crate::maps::MAP_LIST.get(index).map(|map| (index, map.name.to_lowercase())),
                _ => None,
            };
            return Some(PageLook::MapVote { map });
        }
        match self.pressed_kind(self.selected_row, self.selected_column)? {
            // ponytail: a demonized pick shows the survivor's own stage; the menu has no
            // picture and no description strip of a demon, and neither had the original.
            ButtonKind::Character { character, exe_character, .. } if self.submenu == submenus::SINGLE_CHARACTERS => {
                Some(PageLook::CharacterSelect { art: character_art::art_index(character, exe_character)? })
            }
            _ => None,
        }
    }

    /// The Draw event of one widget.
    fn draw_widget(&mut self, context: &mut Context, index: usize, current_time_ms: f64) {
        let clicked = context.input.mouse_left_pressed() && self.mouse_over(context, index);
        let widget = &mut self.widgets[index];
        match &mut widget.kind {
            Kind::Button(button) => {
                let (bid, bcolumn) = (button.bid, button.bcolumn);
                let is_selected = bid == self.selected_row && bcolumn == self.selected_column;
                match &mut button.kind {
                    ButtonKind::Level { index: map } => {
                        let (left, top) = label_corner(&widget.builtins);
                        self.map_grid.draw_card(&mut context.canvas, *map, left, top, is_selected);
                    }
                    kind @ (ButtonKind::Character { .. } | ButtonKind::RandomCharacter | ButtonKind::Freecam) => {
                        if let Some(icon) = CharacterIcon::of(kind, &widget.builtins, current_time_ms) {
                            icon.draw(&mut context.canvas, is_selected, current_time_ms);
                        }
                    }
                    ButtonKind::Plain | ButtonKind::Toggle { .. } => {
                        if button.submenu == submenus::DISABLED {
                            widget.builtins.image_blend = DISABLED_BUTTON_BLEND;
                        }
                        let label = button_label(&widget.builtins, button);
                        let builtins = &widget.builtins;
                        let (left, top) = label_corner(builtins);
                        context.canvas.draw_menu_text(left, top, &label, builtins.image_blend, builtins.image_alpha);
                    }
                    ButtonKind::Skin(skin) => {
                        widget.builtins.image_blend = card_blend(is_selected, skin.is_equipped(&context.unlockables));
                        draw_self(&mut context.canvas, &widget.builtins);
                        let height = card_height(&context.canvas, &widget.builtins);
                        skin.draw(&mut context.canvas, widget.builtins.x, widget.builtins.y, height, &context.unlockables);
                    }
                    ButtonKind::Icon { index: icon } => {
                        widget.builtins.image_blend = card_blend(is_selected, context.unlockables.lobby_icon == *icon);
                        draw_self(&mut context.canvas, &widget.builtins);
                        context.canvas.draw_sprite(sprite::SPR_MENU_LOBBYICON, *icon as f64, widget.builtins.x + CARD_CENTER, widget.builtins.y + CARD_CENTER);
                        let height = card_height(&context.canvas, &widget.builtins);
                        skins::draw_card_state(&mut context.canvas, widget.builtins.x, widget.builtins.y, height, context.unlockables.lobby_icon == *icon);
                    }
                    ButtonKind::Pet(pet) => {
                        widget.builtins.image_blend = card_blend(is_selected, pet.is_equipped(&context.unlockables));
                        draw_self(&mut context.canvas, &widget.builtins);
                        let height = card_height(&context.canvas, &widget.builtins);
                        pet.draw(&mut context.canvas, widget.builtins.x, widget.builtins.y, height, &context.unlockables, current_time_ms);
                    }
                }
                if clicked {
                    self.button_pressed(context, bid, bcolumn);
                }
            }
            Kind::Textbox(_) => self.draw_textbox(context, index, clicked),
            Kind::Textpanel { text, .. } => {
                let (x, y, text) = (widget.builtins.x, widget.builtins.y, text.clone());
                draw_self(&mut context.canvas, &widget.builtins);
                draw_text(&mut context.canvas, x + 4.0, y + 4.0, &text, C_WHITE, 1.0);
            }
            Kind::AboutText { text } => {
                draw_text(&mut context.canvas, widget.builtins.x, widget.builtins.y, text, C_WHITE, 1.0);
            }
            Kind::Gif | Kind::Picture | Kind::CharacterPicture => draw_self(&mut context.canvas, &widget.builtins),
            Kind::Link { url } => {
                if clicked {
                    open_in_browser(url);
                }
                draw_self(&mut context.canvas, &widget.builtins);
            }
            Kind::AchievementArrow => {
                draw_self(&mut context.canvas, &widget.builtins);
                let direction = widget.builtins.yscale;
                if context.input.mouse_any_held() && self.mouse_over(context, index) {
                    self.move_achievement_list(&context.canvas, direction);
                }
            }
            Kind::Version => {
                // obj_menu_version Draw: the chat lettering, which now has the greek
                // alpha the version is. The colour is what the code "&" gave it.
                let (x, y) = (widget.builtins.x, widget.builtins.y);
                let version = format!("{VERSION_PREFIX}&{}", crate::packet::VERSION);
                draw_text(&mut context.canvas, x, y, &version, C_WHITE, 1.0);
            }
            Kind::Warning { popup } => draw_warning(context, &widget.builtins, popup, clicked),
            Kind::Credits => {
                scroll_credits(context, &mut widget.builtins, self.room.height);
                draw_credits(&mut context.canvas, &widget.builtins);
            }
            Kind::AchievementList { .. } => self.draw_achievement_list(context, index),
        }
    }

    /// obj_menu_textbox Draw: the selected text box takes what is typed.
    fn draw_textbox(&mut self, context: &mut Context, index: usize, clicked: bool) {
        let Kind::Textbox(textbox) = &mut self.widgets[index].kind else { return };
        let is_selected = self.selected_row == textbox.bid;
        let mut changed_text = None;
        if is_selected {
            let input = &mut context.input;
            if input.control_held() && input.pressed(macroquad::input::KeyCode::V) {
                input.keyboard_string = macroquad::miniquad::window::clipboard_get().unwrap_or_default();
            }
            if input.keyboard_string != textbox.previous_keyboard_string {
                input.keyboard_string = input.keyboard_string.chars().take(textbox.limit).collect();
                textbox.text = input.keyboard_string.to_lowercase().replace(['\r', '\t', '\n'], "");
                changed_text = Some(textbox.text.clone());
                textbox.previous_keyboard_string = input.keyboard_string.clone();
            }
        }
        let bid = textbox.bid;
        let (shown, colour) = if textbox.text.is_empty() { (textbox.placeholder.clone(), C_GREY) } else { (textbox.text.clone(), C_WHITE) };

        if let Some(text) = changed_text {
            self.text_changed(context, &text);
        }
        if is_selected {
            self.ignore_buttons = true;
        }
        if clicked {
            // buttonSelected(bid, -1) with the last two arguments left out.
            self.button_selected(context, bid, -1, 1, false);
        }
        let widget = &self.widgets[index];
        draw_self(&mut context.canvas, &widget.builtins);
        draw_text(&mut context.canvas, widget.builtins.x + 2.0, widget.builtins.y + 5.0, &shown, colour, 1.0);
    }

    /// obj_menu_achivements Draw.
    fn draw_achievement_list(&mut self, context: &mut Context, index: usize) {
        let keys = &context.options.keys;
        let steps = if context.input.held(keys.up.0) {
            1.0
        } else if context.input.held(keys.down.0) {
            -1.0
        } else {
            0.0
        };
        let widget = &mut self.widgets[index];
        let Kind::AchievementList { scroll, target_scroll } = &mut widget.kind else { return };
        *scroll += (*target_scroll - *scroll) * crate::core::config::eased_share(ACHIEVEMENT_SCROLL_SMOOTHING);
        let scroll = *scroll;
        if steps != 0.0 {
            self.move_achievement_list(&context.canvas, steps);
        }

        let Some(frame) = self.widgets[index].builtins.sprite else { return };
        let Some(height) = self.achievement_row_height(&context.canvas) else { return };
        let width = context.canvas.sprites.get(frame).width as f64;
        let left = VIEW_WIDTH / 2.0 - width / 2.0;
        for row in 0..crate::client::achievements::ACHIEVEMENT_COUNT {
            let is_unlocked = context.achievements.is_unlocked(row);
            let top = ACHIEVEMENT_LIST_TOP + row as f64 * height + scroll;
            if top + height < 0.0 || top > VIEW_HEIGHT {
                continue;
            }
            let colour = if is_unlocked { ACHIEVEMENT_UNLOCKED_BLEND } else { C_WHITE };
            context.canvas.draw_sprite_ext(frame, 0.0, left, top, 1.0, 1.0, 0.0, colour, 1.0);
            // The original drew the lines into the sprite; here they are text in the frame.
            let text = crate::client::achievements::ACHIEVEMENTS[row];
            let lines = text.lines().count() as f64;
            let text_top = top + (height - lines * ACHIEVEMENT_LINE_HEIGHT).floor() / 2.0;
            draw_text(&mut context.canvas, left + ACHIEVEMENT_TEXT_LEFT, text_top, text, colour, 1.0);
        }
    }

    /// position_meeting(mouse_x, mouse_y, self); a worded button is hit around its words.
    fn mouse_over(&self, context: &Context, index: usize) -> bool {
        let widget = &self.widgets[index];
        let (mouse_x, mouse_y) = (context.input.mouse_x, context.input.mouse_y);
        if let Kind::Button(Button { kind: ButtonKind::Level { .. }, .. }) = &widget.kind {
            let (left, top) = label_corner(&widget.builtins);
            return self.map_grid.card_contains(&context.canvas, left, top, mouse_x, mouse_y);
        }
        if let Kind::Button(Button { kind: kind @ (ButtonKind::Character { .. } | ButtonKind::RandomCharacter | ButtonKind::Freecam), .. }) = &widget.kind {
            let now_ms = (macroquad::time::get_time() * 1000.0).floor();
            return CharacterIcon::of(kind, &widget.builtins, now_ms).is_some_and(|icon| icon.contains(mouse_x, mouse_y));
        }
        if let Kind::Button(button @ Button { kind: ButtonKind::Plain | ButtonKind::Toggle { .. }, .. }) = &widget.kind {
            let (left, top) = label_corner(&widget.builtins);
            // The last letter's spacing column is not part of the words.
            let right = left + context.canvas.menu_text_width(&button_label(&widget.builtins, button)) - 1.0;
            let horizontal = (left - LABEL_CLICK_MARGIN_X..right + LABEL_CLICK_MARGIN_X).contains(&mouse_x);
            return horizontal && (top - LABEL_CLICK_MARGIN_Y..top + LABEL_INK_HEIGHT + LABEL_CLICK_MARGIN_Y).contains(&mouse_y);
        }
        let Some(sprite) = widget.builtins.sprite else { return false };
        let bbox = sprite_bbox(context.canvas.sprites.get(sprite), widget.builtins.x, widget.builtins.y, widget.builtins.xscale, widget.builtins.yscale, widget.builtins.angle);
        bbox.contains_point(context.input.mouse_x, context.input.mouse_y)
    }
}

impl Particle {
    /// obj_menu_particle Step_0: drifts left and bobs; all particles bob and blink together.
    fn step(&mut self, room_width: f64, current_time_ms: f64) {
        self.x -= self.speed;
        if self.x < PARTICLE_WRAP_LEFT {
            self.x = room_width;
        }
        self.y = self.start_y + (current_time_ms / 500.0).sin() * 2.0;
        self.alpha = 0.5 + ((1.0 + (current_time_ms / 200.0).sin()) / 2.0) * 0.5;
    }
}

fn spawn_particles(room: &Room) -> Vec<Particle> {
    (0..PARTICLE_COUNT)
        .map(|_| {
            let y = rand::gen_range(0.0, room.height);
            Particle { x: rand::gen_range(0.0, room.width), y, start_y: y, speed: *PARTICLE_SPEEDS.choose().unwrap(), alpha: 1.0 }
        })
        .collect()
}

/// image_index advances by the sprite's speed and wraps after the last frame.
fn advance_animation(widget: &mut Builtins, sprites: &crate::core::resources::sprites::Sprites) {
    let Some(sprite) = widget.sprite else { return };
    let meta = sprites.get(sprite);
    widget.image_index += widget.image_speed * meta.fps as f64 / crate::core::config::ticks_per_second();
    let frame_count = meta.frame_count as f64;
    if widget.image_index >= frame_count {
        widget.image_index -= frame_count;
    }
}

/// The words of a spr_menu_buttons frame, or the button's own: a button this port adds
/// carries its words in the map.
fn button_label(widget: &Builtins, button: &Button) -> String {
    if let Some(label) = &button.label {
        return label.clone();
    }
    widgets::BUTTON_LABELS.get(widget.image_index as usize).copied().unwrap_or("").to_string()
}

fn label_corner(widget: &Builtins) -> (f64, f64) {
    (widget.x - LABEL_LEFT_OF_ORIGIN, widget.y - LABEL_ABOVE_ORIGIN)
}

fn draw_self(canvas: &mut Canvas, widget: &Builtins) {
    if let Some(sprite) = widget.sprite {
        canvas.draw_sprite_ext(sprite, widget.image_index, widget.x, widget.y, widget.xscale, widget.yscale, widget.angle, widget.image_blend, widget.image_alpha);
    }
}

fn card_blend(is_selected: bool, is_equipped: bool) -> u32 {
    if is_selected {
        SELECTED_CARD_BLEND
    } else if is_equipped {
        EQUIPPED_CARD_BLEND
    } else {
        C_WHITE
    }
}

/// sprite_height of a card.
fn card_height(canvas: &Canvas, widget: &Builtins) -> f64 {
    widget.sprite.map_or(0.0, |sprite| canvas.sprites.get(sprite).height as f64 * widget.yscale)
}

/// obj_menu_warning Draw (obj_menu_moneys of the original, whose popup asked for
/// donations to keep the official servers up): a click on the "!" opens or closes the
/// note that those servers are off, any key or click closes it.
fn draw_warning(context: &mut Context, widget: &Builtins, popup: &mut bool, clicked_self: bool) {
    if *popup {
        context.canvas.draw_sprite(sprite::SPR_MENU_WARNING2, 0.0, widget.x + POPUP_OFFSET_X, widget.y);
    }
    if context.input.mouse_left_pressed() {
        *popup = clicked_self && !*popup;
    }
    if *popup && context.input.any_key_pressed() {
        *popup = false;
    }
    draw_self(&mut context.canvas, widget);
}

/// obj_menu_credits Draw: scrolls up slowly, faster or backwards with the arrows, and wraps.
fn scroll_credits(context: &Context, widget: &mut Builtins, room_height: f64) {
    let keys = &context.options.keys;
    if context.input.held(keys.down.0) {
        widget.y -= CREDITS_KEY_SCROLL * step();
    } else if context.input.held(keys.up.0) {
        widget.y += CREDITS_KEY_SCROLL * step();
    } else {
        widget.y -= CREDITS_IDLE_SCROLL * step();
    }
    let height = widget.sprite.map_or(0.0, |sprite| context.canvas.sprites.get(sprite).height as f64 * widget.yscale);
    if widget.y <= -height - CREDITS_WRAP_MARGIN {
        widget.y = room_height - 1.0 - CREDITS_WRAP_MARGIN;
    }
    if widget.y >= room_height + CREDITS_WRAP_MARGIN {
        widget.y = -height + 1.0 + CREDITS_WRAP_MARGIN;
    }
}

/// The credits as text: each line centred on the picture's middle (its origin), 8 px apart.
/// spr_menu_credits is left empty and only gives the list its size.
fn draw_credits(canvas: &mut Canvas, widget: &Builtins) {
    for (index, line) in crate::client::credits::CREDITS.lines().enumerate() {
        let y = widget.y + index as f64 * CREDITS_LINE_HEIGHT;
        if y < -CREDITS_LINE_HEIGHT || y > VIEW_HEIGHT {
            continue;
        }
        draw_text(canvas, widget.x - text_width(canvas, line) / 2.0, y, line, C_WHITE, 1.0);
    }
}

/// obj_lobby Draw in the character select stage (state 2), without the timer: the
/// character's big picture, the bars (the EXE's for a killer), its description strip.
fn draw_character_select(canvas: &mut Canvas, art: usize, current_time_ms: f64) {
    let ticks = current_time_ms / 1000.0 * crate::core::config::ticks_per_second();
    let (x, y) = character_art::ART_POSITION;
    canvas.draw_sprite(character_art::CHARACTER_ART[art], character_art::art_frame(canvas, art, ticks), x, y);
    let is_exe = art >= character_art::SURVIVOR_ART_COUNT;
    canvas.draw_sprite(sprite::SPR_CHAR_BARS, if is_exe { 1.0 } else { 0.0 }, 0.0, 0.0);
    let (strip_x, strip_y) = character_art::DESCRIPTION_POSITION;
    canvas.draw_sprite(sprite::SPR_CHAR_INFO, art as f64, strip_x, strip_y);
}

/// One pick of the Singleplayer character page, standing in the middle of the cell its
/// button marks. Every pick is drawn centred in the same cell, at its own size, as the
/// lobby and the health row draw them: they are all drawn for a cell this big. The EXE
/// icons' pictures are 58 px only for the glow around their 26 px ring, and scaling
/// the pictures to the cell shrank the ring to a dot. The picks are
/// the health row's icon for a survivor and for its demon (screens/character_art.rs),
/// the killer's own lobby icon for an EXE, the "?" of spr_lobby_icon for the random pick
/// and Limp City's eye, the one thing in the game a player already watches through, for
/// the free camera.
struct CharacterIcon {
    sprite: crate::core::resources::SpriteId,
    frame: f64,
    /// The middle of the cell, which is where the button stands.
    centre: (f64, f64),
}

/// The side of a pick's cell: spr_playerhealth, the icon the health row draws, is 26 x 26.
const ICON_CELL: f64 = 26.0;

impl CharacterIcon {
    fn of(kind: &ButtonKind, widget: &Builtins, current_time_ms: f64) -> Option<CharacterIcon> {
        let (sprite, frame) = match *kind {
            ButtonKind::Character { character, exe_character, demonized } => {
                let art = character_art::art_index(character, exe_character)?;
                match art < character_art::SURVIVOR_ART_COUNT {
                    true => character_art::health_row_icon(art, demonized),
                    false => character_art::icon(art, current_time_ms),
                }
            }
            ButtonKind::RandomCharacter => (sprite::SPR_LOBBY_ICON, character_art::UNKNOWN_ICON_FRAME),
            ButtonKind::Freecam => (sprite::SPR_LIMPCITY_EYE, 0.0),
            _ => return None,
        };
        Some(CharacterIcon { sprite, frame, centre: (widget.x, widget.y) })
    }

    /// obj_lobby_icon while choosing: grey, and the chosen one white with its arrows.
    fn draw(&self, canvas: &mut Canvas, chosen: bool, current_time_ms: f64) {
        let blend = if chosen { C_WHITE } else { C_GREY };
        let (x, y) = self.origin_in_cell(canvas);
        canvas.draw_sprite_ext(self.sprite, self.frame, x, y, 1.0, 1.0, 0.0, blend, 1.0);
        if chosen {
            let arrow_frame = character_art::icon_arrow_frame(current_time_ms);
            canvas.draw_sprite(sprite::SPR_LOBBY_ICON_ARROW, arrow_frame, self.centre.0, self.centre.1);
        }
    }

    /// Where the sprite's origin goes for its picture to sit in the middle of the cell.
    fn origin_in_cell(&self, canvas: &Canvas) -> (f64, f64) {
        let picture = canvas.sprites.get(self.sprite);
        let offset = |origin: f64, size: u32| origin - size as f64 / 2.0;
        (self.centre.0 + offset(picture.origin_x, picture.width), self.centre.1 + offset(picture.origin_y, picture.height))
    }

    fn contains(&self, x: f64, y: f64) -> bool {
        (x - self.centre.0).abs() <= ICON_CELL / 2.0 && (y - self.centre.1).abs() <= ICON_CELL / 2.0
    }
}

/// url_open: the system's browser opens the page.
fn open_in_browser(url: &str) {
    #[cfg(target_os = "windows")]
    let opened = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn();
    #[cfg(target_os = "macos")]
    let opened = std::process::Command::new("open").arg(url).spawn();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let opened = std::process::Command::new("xdg-open").arg(url).spawn();
    if let Err(error) = opened {
        eprintln!("could not open {url}: {error}");
    }
}
