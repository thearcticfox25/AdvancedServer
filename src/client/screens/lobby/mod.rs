//! room_lobby: players getting ready, the map vote and the start of a round
//! (obj_lobby with obj_lobby_icon, obj_lobby_prolet, obj_lobby_waiting and
//! obj_lobby_votekick).

mod cards;
mod icons;

use super::character_art::{art_frame, art_index, ART_POSITION, CHARACTER_ART, DESCRIPTION_POSITION};
use super::chat::Chat;
use super::fades::{BlackFadeOut, WhiteFlash};
use super::go_to_error;
use crate::client::canvas::{make_color_rgb, Canvas, C_DKGRAY, VIEW_HEIGHT, VIEW_WIDTH};
use crate::client::net::{self, Notice};
use crate::client::room::Room;
use crate::client::text::draw_text;
use crate::client::Context;
use anyhow::{Context as _, Result};
use cards::{CardsInput, PreferenceCards, EXE_NAMES, SURVIVOR_NAMES};
use icons::{LobbyView, PlayerIcon, Splash};
use macroquad::input::KeyCode;
use super::map_preview::MapPreview;
use crate::packet::{Packet, PacketType};
use crate::core::resources::names::{sound, sprite};
use crate::core::objects::ids::ObjectId;
use crate::core::rooms::ids::RoomId;
use crate::core::world::C_WHITE;
use super::vote_kick::{PickerOutcome, PlayerPicker};
use crate::core::config::step;

/// obj_lobby.state
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LobbyState {
    Lobby,
    Vote,
    /// After the vote, before the level: the map, this player's character and
    /// every character of the round. Replaces the character select stage.
    RoundIntro,
}

const CHAT_MAX_LENGTH: usize = 48;
const CHAT_LINES_TOP: f64 = 142.0;
const CHAT_BOX_TOP: f64 = 211.0;
const EXE_CHANCE_TEXT_POSITION: (f64, f64) = (170.0, 256.0);
const SIGN_POSITION: (f64, f64) = (240.0, 8.0);
const SIGN_FRAME_MILLISECONDS: f64 = 300.0;
const COUNTER_POSITION: (f64, f64) = (240.0, 13.0);
/// The key hints of the bottom bar, where spr_lobby_bars had them written: the first from
/// the left, the second around the middle, the last up to the right.
const HINTS_TOP: f64 = 242.0;
const HINT_LEFT: f64 = 28.0;
const HINT_RIGHT_END: f64 = 459.0;
/// A countdown number pops in bigger and shrinks back.
const COUNTER_POP_SCALE: f64 = 1.5;
const COUNTER_SHRINK_PER_STEP: f64 = 0.02;
/// Vote timer values that tick audibly.
const VOTE_WARNING_SECONDS: u8 = 3;

/// The three map cards of the vote.
const MAP_CARD_X: [f64; 3] = [16.0, 179.0, 341.0];
const MAP_CARD_TOP: f64 = 58.0;
/// The cards slide down this far while fading in.
const MAP_CARD_SLIDE: f64 = 20.0;
const MAP_CARD_FADE_PER_STEP: f64 = 0.016;
/// The selected card jumps up this far and settles.
const MAP_CARD_JUMP: f64 = 4.0;
const MAP_CARD_JUMP_FALL_PER_STEP: f64 = 0.32;
const VOTE_COUNT_OFFSET: (f64, f64) = (61.0, 53.0);
const MAP_DESCRIPTION_POSITION: (f64, f64) = (4.0, 233.0);
const VOTED_MAP_BLEND: u32 = make_color_rgb(0x27, 0xFF, 0x23);

/// The round intro, laid out as the character select stage of obj_lobby.
/// The chosen map's card, at the right under the top bar.
const INTRO_MAP_CARD_POSITION: (f64, f64) = (341.0, 32.0);
/// The other players' characters stand small and dimmed under the card, right
/// of this player's character, spread between these x.
const INTRO_CROWD_LEFT: f64 = 230.0;
const INTRO_CROWD_RIGHT: f64 = 480.0;
const INTRO_CROWD_SCALE: f64 = 0.5;
const INTRO_CROWD_BLEND: u32 = C_DKGRAY;
/// spr_char_bars has the title "select the character" in its top bar; there is
/// no selection here, so the top bar is its title-free left end stretched.
const CHAR_BARS_TOP_HEIGHT: f64 = 27.0;
const CHAR_BARS_UNTITLED_WIDTH: f64 = 64.0;
/// The intro stays this long once the round starts, then fades (obj_lobby state 3).
/// The original faded at once after a selection stage; here the round starts in
/// the same server tick as the vote result, so the intro would only be seen fading.
const INTRO_HOLD_SECONDS: f64 = 2.0;
const INTRO_FADE_PER_STEP: f64 = 0.01;

/// global.levelDescriptions, by level number.
const LEVEL_DESCRIPTIONS: [&str; 20] = [
    "long and spacious map with no less spacious boxes to hide behind, using       \nsprings echoes all over the cave, revealing your location to exe |+25 sec.",
    "6 shards have to be collected to access the exit on the last minute, collected\nshards will lead you to the exit. avoid slugs that feed on rings, kill them  \nto obtain the ring if they devour one",
    "long stairway with three floors, you can't run on stairs and the lower you go \n- the darker it gets |+25 sec.",
    "high buildings will serve you a hideout, it's hard to break away here, but you\ncan hide almost everywhere",
    "constant gas leaks will fill the areas poisoning you and causing you to       \nhallucinate, revealing your location to exe ",
    "the city is limp but not blind, you can be watched through eye cameras or use \nthem yourself, there are the circuits that periodically activate blocking the \npassages |-25 sec.",
    "map rotates every 30 seconds, the floor becomes walls and the walls become the\nfloor, the exit is available on only one turn |-25 sec.",
    "avoid breaking speed monitors, as they will launch you right in the spikes    \nwithout letting you stop running until you meet a wall or get hurt",
    "you can see only the silhouettes, the walls are slowly shrickening, avoid     \ngetting cornered if you don't want to be squished |-50 sec. +10 sec. for each\nsurvivor instead of 25",
    "giant snowballs roll down the slopes, damaging and slowing you down.\nsometimes springs and rings cover with ice |-25 sec.",
    "escape with elevators and avoid black rings that will drain your health upon  \ngetting them, damage from them can be avoided by having 5 rings |-25 sec. +10 \nsec. for each survivor instead of 25",
    "long map filled with lava pits, some of which burst with lava pillars. break  \nthe pots to get more rings",
    "long linear map filled with water that slows you down. when the lightning     \nstrikes, don't touch the water",
    "wherever you run - you will never reach the end, there are no bounds, fun is   \ninfinite |-25 sec. +10 sec. for each survivor instead of 25",
    "hide behind palm trees and bushes, moving along the ziplines",
    "an abandoned mine illuminated only by searchlights, sometimes acid evaporates \nincreasing its area, stay away from it unless you're a fan of sulfuric acid  \nbaths |-25 sec.",
    "don't stand under stalactites, they will fall if you walk under them. every 10 \nseconds it gets dark for a while, at this time try not to run into the owner  \nof this place |+25 sec.",
    "you will be able to open the passages by activating the crystals. hold the jump  \nor up on special springs so that they throw you higher, and most importantly, \ndo not wake up |+25 sec.",
    "be careful on conveyor belts and get to ignited lanterns to banish spirit that \nhaunts the forest, before things get even darker |-25 sec",
    "you better not touch the crystals releasing the phantom energy, otherwise it \nwill be difficult for you to move and avoid traps of the ancient temple \n|+25 sec",
];

pub struct Lobby {
    room: Room,
    /// The layer of obj_lobby, which also holds obj_lobby_waiting.
    lobby_layer: usize,
    preview_layer: usize,
    /// obj_lobby_waiting: its position, hidden once the round's EXE is known.
    waiting_sign_position: Option<(f64, f64)>,
    state: LobbyState,
    countdown: u8,
    is_counting: bool,
    /// isReady: the local player pressed ready.
    is_ready: bool,
    count_scale: f64,
    chat: Chat,
    /// The vote: the three maps, the one under the cursor, and the votes so far.
    maps: [u8; 3],
    chosen_map: usize,
    chosen_map_jump: f64,
    votes: [u8; 3],
    has_voted: bool,
    map_fade: f64,
    vote_timer: u8,
    /// canUse: the server confirmed this player, so the lobby takes input.
    can_use: bool,
    icons: Vec<PlayerIcon>,
    /// The page of the row of other players on the screen (icons::row_pages).
    row_page: usize,
    splashes: Vec<Splash>,
    preview: MapPreview,
    /// obj_lobby_votekick
    picker: Option<PlayerPicker>,
    /// The preference cards that replace the character select stage.
    cards: PreferenceCards,
    flash: Option<WhiteFlash>,
    fade_in: Option<BlackFadeOut>,
    /// The round intro: its map, ticks shown, and the fade into the level once the round starts.
    round_map: Option<u16>,
    intro_ticks: f64,
    round_starting_ticks: Option<i32>,
    fade_out: f64,
}

impl Lobby {
    pub fn open(context: &mut Context) -> Result<Lobby> {
        let room = Room::load(&context.maps_folder, RoomId::Lobby, &context.canvas.sprites)?;
        let (lobby_layer, _) = room.placed(ObjectId::Lobby).next().context("room_lobby has no obj_lobby")?;
        let (preview_layer, _) = room.placed(ObjectId::LobbyProlet).next().context("room_lobby has no obj_lobby_prolet")?;
        let waiting_sign_position = room.placed(ObjectId::LobbyWaiting).next().map(|(_, placed)| (placed.x, placed.y));
        // RoomCreationCode
        context.audio.play_music(sound::MUS_LOBBY);
        let net = &context.net;
        let fade_in = (net.is_connected && net.is_ready && net.id.is_some()).then(BlackFadeOut::new);
        Ok(Lobby {
            room,
            lobby_layer,
            preview_layer,
            waiting_sign_position,
            state: LobbyState::Lobby,
            countdown: 0,
            is_counting: false,
            is_ready: false,
            count_scale: 1.0,
            chat: Chat::default(),
            maps: [0; 3],
            chosen_map: 0,
            chosen_map_jump: MAP_CARD_JUMP,
            votes: [0; 3],
            has_voted: false,
            map_fade: 0.0,
            vote_timer: 20,
            can_use: false,
            icons: Vec::new(),
            row_page: 0,
            splashes: Vec::new(),
            preview: MapPreview::new(),
            picker: None,
            cards: PreferenceCards::new(&context.options),
            flash: None,
            fade_in,
            round_map: None,
            intro_ticks: 0.0,
            round_starting_ticks: None,
            fade_out: 0.0,
        })
    }

    pub fn tick(&mut self, context: &mut Context) -> Option<RoomId> {
        let current_time_ms = (macroquad::time::get_time() * 1000.0).floor();
        let next_room = self.step(context);
        self.draw(context, current_time_ms);
        next_room
    }

    fn step(&mut self, context: &mut Context) -> Option<RoomId> {
        self.room.step(&context.canvas.sprites);
        let chat_allowed = self.state == LobbyState::Lobby && self.can_use && self.picker.is_none();
        if chat_allowed && context.input.pressed(KeyCode::Enter) {
            self.chat.toggle(context);
        }
        if chat_allowed && context.input.pressed(KeyCode::T) {
            self.chat.open(&mut context.input);
        }
        if let Some(room) = self.step_input(context) {
            return Some(room);
        }
        if let Some(level_room) = self.step_round_start() {
            return Some(level_room);
        }

        // A player leaving may take the last page with them.
        let last_page = icons::row_pages(context.net.players.len()).saturating_sub(1);
        self.row_page = self.row_page.min(last_page);
        let view = LobbyView { state: self.state, is_ready: self.is_ready, net: &context.net, row_page: self.row_page };
        for icon in &mut self.icons {
            icon.step(&view, &context.canvas.sprites);
        }
        self.splashes.retain_mut(|splash| splash.step(&context.canvas.sprites));
        self.flash = self.flash.take().and_then(|mut flash| flash.step().then_some(flash));
        self.fade_in = self.fade_in.take().and_then(|mut fade| fade.step().then_some(fade));

        for notice in net::update(context) {
            if let Some(room) = self.on_notice(context, notice) {
                return Some(room);
            }
        }
        None
    }

    /// Step_0: typing, moving the vote cursor, paging the row of players, voting,
    /// getting ready, leaving.
    fn step_input(&mut self, context: &mut Context) -> Option<RoomId> {
        if self.chat.is_open {
            self.chat.type_message(&mut context.input, CHAT_MAX_LENGTH);
        }
        if self.cards.is_open {
            if matches!(self.cards.step(context), CardsInput::Confirm) {
                self.get_ready_with_cards(context);
            }
            return None;
        }
        let keys = context.options.keys.clone();
        let input = &context.input;
        let can_move_vote_cursor = self.state == LobbyState::Vote && !self.has_voted && self.map_fade >= 1.0;
        let can_turn_row_page = self.state == LobbyState::Lobby && !self.chat.is_open && self.picker.is_none();
        let sideways = if input.pressed(keys.left.0) {
            -1
        } else if input.pressed(keys.right.0) {
            1
        } else {
            0
        };

        if sideways != 0 {
            if self.can_use && can_move_vote_cursor {
                self.chosen_map = (self.chosen_map as i32 + sideways).rem_euclid(3) as usize;
                self.chosen_map_jump = MAP_CARD_JUMP;
                context.audio.play(sound::SND_MENU_SELECT, false);
            } else if can_turn_row_page {
                self.turn_row_page(context, sideways);
            }
        } else if input.pressed(keys.jump.0) {
            if !self.can_use {
                return None;
            }
            if can_move_vote_cursor {
                let mut packet = Packet::new(PacketType::CLIENT_VOTE_REQUEST);
                let _ = packet.write_u8(self.chosen_map as u8);
                context.net.send_reliable(&packet);
                context.audio.play(sound::SND_MENU_PRESS, false);
                self.has_voted = true;
                return None;
            }
            if self.chat.is_open || self.picker.is_some() || !context.net.is_ready {
                return None;
            }
            if context.net.state == net::NetState::Lobby {
                // Getting ready goes through the preference cards; getting unready does
                // not, and neither does anything when the server lets nobody pick.
                if self.is_ready || !context.net.character_selection {
                    self.is_ready = !self.is_ready;
                    self.send_ready_state(context);
                    context.audio.play(sound::SND_READY, false);
                } else {
                    self.cards.is_open = true;
                    context.audio.play(sound::SND_MENU_SELECT, false);
                }
            }
        } else if input.pressed(keys.special1.0) {
            let may_leave = self.can_use && self.picker.is_none() && !self.chat.is_open && context.net.is_ready;
            if may_leave && self.state == LobbyState::Lobby {
                return Some(net::reset(context));
            }
        }
        None
    }

    /// The next or the previous page of the row of other players, if there is one.
    fn turn_row_page(&mut self, context: &mut Context, step: i32) {
        let pages = icons::row_pages(context.net.players.len());
        let Some(page) = self.row_page.checked_add_signed(step as isize).filter(|&page| page < pages) else { return };
        self.row_page = page;
        context.audio.play(sound::SND_MENU_SELECT, false);
    }

    /// Z on the open cards: ready with them, unless a card's character is taken.
    fn get_ready_with_cards(&mut self, context: &mut Context) {
        if let Some(taken) = self.cards.taken_character(&context.net) {
            self.refuse_card(context, Some(&taken));
            return;
        }
        self.cards.is_open = false;
        self.is_ready = true;
        self.send_ready_state(context);
        context.audio.play(sound::SND_READY, false);
        context.options.preferred_exe = self.cards.exe;
        context.options.preferred_survivor = self.cards.survivor;
        context.options.save();
    }

    /// CLIENT_LOBBY_READY_STATE with the preference cards, 1-based.
    fn send_ready_state(&self, context: &mut Context) {
        let mut packet = Packet::new(PacketType::CLIENT_LOBBY_READY_STATE);
        let _ = packet.write_u8(u8::from(self.is_ready));
        let _ = packet.write_u8(self.cards.exe);
        let _ = packet.write_u8(self.cards.survivor);
        context.net.send_reliable(&packet);
    }

    /// A card's character is someone else's: say so and let the player choose again.
    fn refuse_card(&mut self, context: &mut Context, character_name: Option<&&str>) {
        context.audio.play(sound::SND_NONO, false);
        let name = character_name.copied().unwrap_or("that character");
        self.chat.add_message(context, "\\(lobby)~", &format!("{name} is already taken"));
        self.is_ready = false;
        self.cards.is_open = true;
    }

    fn on_notice(&mut self, context: &mut Context, notice: Notice) -> Option<RoomId> {
        match notice {
            Notice::GoTo(room) => return Some(room),
            Notice::ShowError(code) => return Some(go_to_error(context, code)),
            Notice::PlayerJoined { id, play_sound } => {
                if let Some(player) = context.net.players.get(&id) {
                    let (nickname, is_ready) = (player.nickname.clone(), player.is_ready);
                    self.player_joined(context, id, nickname, is_ready, play_sound);
                }
            }
            Notice::JoinedLobby => {
                if let Some(id) = context.net.id {
                    let nickname = context.options.nickname.clone();
                    self.player_joined(context, id, nickname, false, true);
                }
                self.can_use = true;
            }
            Notice::PlayerLeft(id) => self.player_left(context, id),
            Notice::ChatMessage { sender, text } => self.chat.add_message(context, &sender, &text),
            Notice::Countdown { counting, seconds } => {
                self.count_scale = COUNTER_POP_SCALE;
                self.is_counting = counting;
                self.countdown = seconds;
            }
            Notice::VoteMaps(maps) => {
                self.state = LobbyState::Vote;
                self.maps = maps;
                for icon in &mut self.icons {
                    icon.hidden = true;
                }
                self.room.set_layer_visible("Background", false);
            }
            Notice::VoteTime(seconds) => {
                self.count_scale = 1.0;
                self.vote_timer = seconds;
                if seconds <= VOTE_WARNING_SECONDS {
                    self.count_scale = COUNTER_POP_SCALE;
                    context.audio.play(sound::SND_CLOCK, false);
                }
            }
            Notice::VoteCounts(votes) => self.votes = votes,
            Notice::ExeChosen { map } => self.exe_chosen(map),
            Notice::SurvivorTaken(survivor) => self.refuse_card(context, SURVIVOR_NAMES.get(survivor.wrapping_sub(1) as usize)),
            Notice::ExeTaken(exe) => self.refuse_card(context, EXE_NAMES.get(exe.wrapping_sub(1) as usize)),
            Notice::RoundStarts => {
                self.round_starting_ticks = Some(0);
                context.audio.stop_music();
            }
            Notice::ChoosePlayer(request) => {
                self.chat.add_message(context, "\\()~", "choose the player");
                self.chat.add_message(context, "\\()~", "(press \\x~ to cancel)");
                self.picker = Some(PlayerPicker::new(request));
                self.chat.typed.clear();
                context.input.keyboard_string.clear();
            }
            _ => {}
        }
        None
    }

    /// lobby_player_joined
    fn player_joined(&mut self, context: &mut Context, id: u16, nickname: String, is_ready: bool, play_sound: bool) {
        if self.icons.iter().any(|icon| icon.player == id) {
            return;
        }
        self.icons.push(PlayerIcon::new(id, nickname, is_ready));
        if play_sound {
            context.audio.play(sound::SND_RING, false);
        }
    }

    /// lobby_player_left: the icon bursts into a splash.
    fn player_left(&mut self, context: &mut Context, id: u16) {
        let Some(index) = self.icons.iter().position(|icon| icon.player == id) else { return };
        let icon = self.icons.remove(index);
        self.splashes.push(Splash::new(icon.x, icon.y));
        context.audio.play(sound::SND_HURT, false);
    }

    /// SERVER_LOBBY_EXE: the vote is over; the round intro shows the map while the
    /// server hands out the characters. The player icons stay hidden, as in the vote.
    fn exe_chosen(&mut self, map: u16) {
        self.flash = Some(WhiteFlash::new());
        self.waiting_sign_position = None;
        self.state = LobbyState::RoundIntro;
        self.round_map = Some(map);
        self.preview.blend = C_WHITE;
        self.preview.set_zone(map as usize);
    }

    /// After the round starts the intro holds a moment, fades out, and the level opens.
    fn step_round_start(&mut self) -> Option<RoomId> {
        let ticks = self.round_starting_ticks.as_mut()?;
        *ticks += 1;
        if *ticks <= crate::core::config::ticks(INTRO_HOLD_SECONDS) {
            return None;
        }
        self.fade_out += INTRO_FADE_PER_STEP * step();
        if self.fade_out <= 1.0 {
            return None;
        }
        let level = crate::client::levels::LEVELS.get(self.round_map? as usize)?;
        Some(level.room)
    }

    fn draw(&mut self, context: &mut Context, current_time_ms: f64) {
        for layer_index in 0..self.room.layers.len() {
            if !self.room.layers[layer_index].visible {
                continue;
            }
            self.room.draw_layer(&mut context.canvas, &self.room.layers[layer_index]);
            if layer_index == self.preview_layer {
                self.preview.draw(&mut context.canvas, current_time_ms);
            }
            if layer_index == self.lobby_layer {
                if let Some((x, y)) = self.waiting_sign_position {
                    context.canvas.draw_sprite(sprite::SPR_LOBBY_WAITING, 0.0, x, y);
                }
                self.draw_lobby(context, current_time_ms);
                self.draw_picker(context, current_time_ms);
            }
        }
        let view = LobbyView { state: self.state, is_ready: self.is_ready, net: &context.net, row_page: self.row_page };
        for icon in &mut self.icons {
            icon.draw(&mut context.canvas, &view, &context.unlockables);
        }
        if self.state == LobbyState::Lobby {
            let pages = icons::row_pages(context.net.players.len());
            icons::draw_page_arrows(&mut context.canvas, self.row_page, pages, current_time_ms);
            if let Some(own_icon) = self.icons.iter().find(|icon| icon.is_own(&context.net)) {
                self.cards.draw(&mut context.canvas, own_icon.x, own_icon.y, &context.net, self.is_ready, current_time_ms);
            }
        }
        for splash in &self.splashes {
            splash.draw(&mut context.canvas);
        }
        if let Some(flash) = &self.flash {
            flash.draw(&mut context.canvas);
        }
        if let Some(fade) = &self.fade_in {
            fade.draw_gui(&mut context.canvas);
        }
        if self.fade_out > 0.0 {
            context.canvas.draw_sprite_ext(sprite::SPR_BLACK, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, C_WHITE, self.fade_out);
        }
    }

    /// The other players' characters dimmed behind, the map's card, and this
    /// player's character in front with its bars and description strip.
    fn draw_round_intro(&mut self, context: &mut Context) {
        let net = &context.net;
        let canvas = &mut context.canvas;
        let ticks = self.intro_ticks;
        let frame = |canvas: &Canvas, art: usize| art_frame(canvas, art, ticks);
        let (own_x, floor_y) = ART_POSITION;

        let crowd = others_characters(net);
        let crowd_spacing = (INTRO_CROWD_RIGHT - INTRO_CROWD_LEFT) / (crowd.len() + 1) as f64;
        for (place, &art) in crowd.iter().enumerate() {
            let x = INTRO_CROWD_LEFT + crowd_spacing * (place + 1) as f64;
            let scale = INTRO_CROWD_SCALE;
            canvas.draw_sprite_ext(CHARACTER_ART[art], frame(canvas, art), x, floor_y, scale, scale, 0.0, INTRO_CROWD_BLEND, 1.0);
        }
        if let Some(map) = self.round_map {
            let (card_x, card_y) = INTRO_MAP_CARD_POSITION;
            canvas.draw_sprite(sprite::SPR_MAPVOTE, map as f64, card_x, card_y);
        }
        let own_art = art_index(net.character, net.exe_character);
        if let Some(art) = own_art {
            canvas.draw_sprite(CHARACTER_ART[art], frame(canvas, art), own_x, floor_y);
        }
        let bars_frame = if net.character == net::EXE_CHARACTER { 1.0 } else { 0.0 };
        let top_bar = [0.0, 0.0, CHAR_BARS_UNTITLED_WIDTH, CHAR_BARS_TOP_HEIGHT];
        canvas.draw_sprite_part_ext(sprite::SPR_CHAR_BARS, bars_frame, top_bar, 0.0, 0.0, VIEW_WIDTH / CHAR_BARS_UNTITLED_WIDTH, 1.0, C_WHITE, 1.0);
        let rest = [0.0, CHAR_BARS_TOP_HEIGHT, VIEW_WIDTH, VIEW_HEIGHT - CHAR_BARS_TOP_HEIGHT];
        canvas.draw_sprite_part_ext(sprite::SPR_CHAR_BARS, bars_frame, rest, 0.0, CHAR_BARS_TOP_HEIGHT, 1.0, 1.0, C_WHITE, 1.0);
        if let Some(art) = own_art {
            let (strip_x, strip_y) = DESCRIPTION_POSITION;
            canvas.draw_sprite(sprite::SPR_CHAR_INFO, art as f64, strip_x, strip_y);
        }
        // Counted in 60 Hz steps, which the character art animates by.
        self.intro_ticks += crate::core::config::step();
    }

    fn draw_picker(&mut self, context: &mut Context, current_time_ms: f64) {
        let Some(picker) = self.picker.as_mut() else { return };
        let others: Vec<u16> = context.net.players.keys().copied().filter(|id| Some(*id) != context.net.id).collect();
        if self.state != LobbyState::Lobby || matches!(picker.draw(context, &others, current_time_ms), PickerOutcome::Closed) {
            self.picker = None;
        }
    }

    /// Draw_0 of obj_lobby.
    fn draw_lobby(&mut self, context: &mut Context, current_time_ms: f64) {
        if self.count_scale > 1.0 {
            self.count_scale -= COUNTER_SHRINK_PER_STEP * step();
        }
        match self.state {
            LobbyState::Lobby => self.draw_waiting_for_players(context, current_time_ms),
            LobbyState::Vote => self.draw_vote(context),
            LobbyState::RoundIntro => self.draw_round_intro(context),
        }
    }

    fn draw_waiting_for_players(&mut self, context: &mut Context, current_time_ms: f64) {
        let canvas = &mut context.canvas;
        canvas.draw_sprite(sprite::SPR_LOBBY_BARS, 0.0, 0.0, 0.0);
        // Written with the keys this player bound, not the defaults the picture showed.
        let keys = &context.options.keys;
        let ready = format!("ready <press {}>", crate::client::input::key_name(keys.jump.0));
        let chat = "open chat <press enter>";
        let exit = format!("exit <press {}>", crate::client::input::key_name(keys.special1.0));
        canvas.draw_menu_text(HINT_LEFT, HINTS_TOP, &ready, C_WHITE, 1.0);
        canvas.draw_menu_text((VIEW_WIDTH / 2.0 - canvas.menu_text_width(chat) / 2.0).floor(), HINTS_TOP, chat, C_WHITE, 1.0);
        canvas.draw_menu_text(HINT_RIGHT_END - canvas.menu_text_width(&exit), HINTS_TOP, &exit, C_WHITE, 1.0);
        let (counter_x, counter_y) = COUNTER_POSITION;
        if self.is_counting && self.picker.is_none() {
            let scale = self.count_scale;
            canvas.draw_sprite_ext(sprite::SPR_COUNTDOWN, self.countdown as f64 - 1.0, counter_x, counter_y, scale, scale, 0.0, C_WHITE, 1.0);
        } else {
            let sign = if self.picker.is_some() { sprite::SPR_LOBBY_VOTEKICK } else { sprite::SPR_LOBBY_WAITING };
            let (sign_x, sign_y) = SIGN_POSITION;
            canvas.draw_sprite(sign, (current_time_ms / SIGN_FRAME_MILLISECONDS) % 4.0, sign_x, sign_y);
        }
        self.chat.draw(canvas, CHAT_LINES_TOP, CHAT_BOX_TOP);
        let (text_x, text_y) = EXE_CHANCE_TEXT_POSITION;
        draw_text(canvas, text_x, text_y, &format!("\\your chance to be exe: {}%", context.net.exe_chance), C_WHITE, 1.0);
    }

    fn draw_vote(&mut self, context: &mut Context) {
        let canvas = &mut context.canvas;
        canvas.draw_sprite(sprite::SPR_VOTE_BARS, 0.0, 0.0, 0.0);
        let (counter_x, counter_y) = COUNTER_POSITION;
        let scale = self.count_scale;
        canvas.draw_sprite_ext(sprite::SPR_COUNTER3, self.vote_timer as f64, counter_x, counter_y, scale, scale, 0.0, C_WHITE, 1.0);

        for (card, &map) in self.maps.iter().enumerate() {
            let is_cursor = self.chosen_map == card && !self.has_voted;
            let blend = if is_cursor { C_WHITE } else { C_DKGRAY };
            canvas.draw_sprite_ext(sprite::SPR_MAPVOTE, map as f64, MAP_CARD_X[card], self.map_card_y(card), 1.0, 1.0, 0.0, blend, self.map_fade);
        }
        if self.map_fade < 1.0 {
            self.map_fade += MAP_CARD_FADE_PER_STEP * step();
        }
        if self.chosen_map_jump > 0.0 {
            self.chosen_map_jump -= MAP_CARD_JUMP_FALL_PER_STEP * step();
        }

        // The preview shows the map under the cursor from the next frame on.
        let map_under_cursor = self.maps[self.chosen_map] as usize;
        self.preview.visible = true;
        self.preview.blend = C_DKGRAY;
        if self.preview.map() != map_under_cursor {
            self.preview.set_zone(map_under_cursor);
        }
        let (text_x, text_y) = MAP_DESCRIPTION_POSITION;
        if let Some(description) = LEVEL_DESCRIPTIONS.get(map_under_cursor) {
            draw_text(canvas, text_x, text_y, description, C_WHITE, 1.0);
        }
        if !self.has_voted {
            return;
        }
        for (card, &count) in self.votes.iter().enumerate() {
            let blend = if self.chosen_map == card { VOTED_MAP_BLEND } else { C_WHITE };
            let (offset_x, offset_y) = VOTE_COUNT_OFFSET;
            canvas.draw_sprite_ext(sprite::SPR_COUNTER2, count as f64, MAP_CARD_X[card] + offset_x, self.map_card_y(card) + offset_y, 1.0, 1.0, 0.0, blend, 1.0);
        }
    }

    /// Cards slide down while fading in; the one under the cursor jumps.
    fn map_card_y(&self, card: usize) -> f64 {
        let jump = if self.chosen_map == card { self.chosen_map_jump } else { 0.0 };
        MAP_CARD_TOP + MAP_CARD_SLIDE * self.map_fade - jump
    }
}

/// The characters the other players got this round, each once, in CHARACTER_ART order.
fn others_characters(net: &net::NetClient) -> Vec<usize> {
    let mut arts: Vec<usize> = net
        .players
        .values()
        .filter_map(|player| art_index(player.character, player.exe_character))
        .collect();
    arts.sort_unstable();
    arts.dedup();
    arts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intro_shows_each_other_players_character_once() {
        let mut net = net::NetClient::new();
        net.character = 3;
        for (id, character, exe_character) in [(1, 2, -1), (2, 2, -1), (3, net::EXE_CHARACTER, 1), (4, -1, -1)] {
            let mut player = net::Player::new(format!("fox{id}"), 0, false);
            player.character = character;
            player.exe_character = exe_character;
            net.players.insert(id, player);
        }
        assert_eq!(others_characters(&net), vec![1, 7], "knuckles once, chaos; not this player's eggman");
    }
}
