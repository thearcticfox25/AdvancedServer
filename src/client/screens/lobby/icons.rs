//! obj_lobby_icon: a player's icon and nickname in the lobby. The local player
//! stands in the middle, everyone else in a row along the top. A row longer than
//! the screen is split into pages that left and right turn.

use super::super::character_art::icon_arrow_frame;
use crate::client::canvas::{make_color_rgb, Canvas};
use crate::client::net::NetClient;
use crate::client::text::{draw_text, text_width};
use crate::client::unlockables::Unlockables;
use crate::core::resources::names::sprite;
use crate::core::resources::sprites::Sprites;
use crate::core::resources::SpriteId;
use crate::core::world::{C_GREY, C_WHITE};
use crate::core::config::step;

const OWN_ICON_X: f64 = 240.0;
const OWN_ICON_Y: f64 = 115.0;
/// Seven places a page, centred on the screen: 48 to 432 around the middle at 240.
const ROW_LEFT: f64 = 48.0;
const ROW_STEP: f64 = 64.0;
const ROW_PLACES: usize = 7;
const ROW_Y: f64 = 58.0;
/// spr_lobby_icon_arrow is a pair of arrows 40 px wide; one half of it points to
/// the next page, drawn this far outside the first and the last place.
const ARROW_HALF_WIDTH: f64 = 20.0;
const ARROW_HEIGHT: f64 = 24.0;
const PAGE_ARROW_OUTSIDE: f64 = 32.0;
/// Where lobby_player_joined creates an icon, before its first step moves it.
const SPAWN_X: f64 = 44.0;
const SPAWN_Y: f64 = 140.0;
const NICKNAME_BELOW: f64 = 16.0;
const GROWTH_PER_STEP: f64 = 0.2;
const JUMP_HEIGHT: f64 = 3.0;
const JUMP_FALL_PER_STEP: f64 = 0.2;
const C_LIME: u32 = make_color_rgb(0x00, 0xFF, 0x00);

pub struct PlayerIcon {
    /// master_id
    pub player: u16,
    pub nickname: String,
    /// The player has picked a character.
    pub is_selected: bool,
    is_ready: bool,
    /// Hidden during the vote.
    pub hidden: bool,
    anchor_y: f64,
    offset: f64,
    pub x: f64,
    pub y: f64,
    pub sprite: SpriteId,
    pub image_index: f64,
    image_speed: f64,
    xscale: f64,
    blend: u32,
    visible: bool,
}

/// What an icon needs to know about the lobby around it.
pub struct LobbyView<'a> {
    pub state: super::LobbyState,
    /// obj_lobby.isReady: the local player's own readiness.
    pub is_ready: bool,
    pub net: &'a NetClient,
    /// The page of the row of other players on the screen.
    pub row_page: usize,
}

/// How many pages the row of `others` players takes.
pub fn row_pages(others: usize) -> usize {
    others.div_ceil(ROW_PLACES)
}

/// The arrow halves beside the row that point to the pages before and after this one.
pub fn draw_page_arrows(canvas: &mut Canvas, page: usize, pages: usize, current_time_ms: f64) {
    let frame = icon_arrow_frame(current_time_ms);
    let top = ROW_Y - ARROW_HEIGHT / 2.0;
    if page > 0 {
        let x = ROW_LEFT - PAGE_ARROW_OUTSIDE - ARROW_HALF_WIDTH / 2.0;
        canvas.draw_sprite_part_ext(sprite::SPR_LOBBY_ICON_ARROW, frame, [0.0, 0.0, ARROW_HALF_WIDTH, ARROW_HEIGHT], x, top, 1.0, 1.0, C_WHITE, 1.0);
    }
    if page + 1 < pages {
        let last_place = ROW_LEFT + (ROW_PLACES - 1) as f64 * ROW_STEP;
        let x = last_place + PAGE_ARROW_OUTSIDE - ARROW_HALF_WIDTH / 2.0;
        canvas.draw_sprite_part_ext(sprite::SPR_LOBBY_ICON_ARROW, frame, [ARROW_HALF_WIDTH, 0.0, ARROW_HALF_WIDTH, ARROW_HEIGHT], x, top, 1.0, 1.0, C_WHITE, 1.0);
    }
}

impl PlayerIcon {
    /// lobby_player_joined creates it.
    pub fn new(player: u16, nickname: String, is_ready: bool) -> PlayerIcon {
        PlayerIcon {
            player,
            nickname,
            is_selected: false,
            is_ready,
            hidden: false,
            anchor_y: ROW_Y,
            offset: 0.0,
            x: SPAWN_X,
            y: SPAWN_Y,
            sprite: sprite::SPR_LOBBY_ICON,
            image_index: 0.0,
            image_speed: 0.0,
            xscale: 0.0,
            blend: C_WHITE,
            visible: false,
        }
    }

    pub fn is_own(&self, net: &NetClient) -> bool {
        net.id == Some(self.player)
    }

    /// Step_0: grows in, jumps when readiness changes, and takes its place.
    pub fn step(&mut self, lobby: &LobbyView, sprites: &Sprites) {
        let meta = sprites.get(self.sprite);
        self.image_index += self.image_speed * meta.fps as f64 / crate::core::config::ticks_per_second();
        if self.xscale < 1.0 {
            self.xscale += GROWTH_PER_STEP * step();
        }
        if self.offset > 0.0 {
            self.offset -= JUMP_FALL_PER_STEP * step();
        }
        let in_lobby = lobby.state == super::LobbyState::Lobby;

        if self.is_own(lobby.net) {
            (self.x, self.y, self.anchor_y) = (OWN_ICON_X, OWN_ICON_Y, OWN_ICON_Y);
            if in_lobby && lobby.is_ready != self.is_ready {
                self.offset = JUMP_HEIGHT;
                self.blend = if lobby.is_ready { C_LIME } else { C_WHITE };
                self.is_ready = lobby.is_ready;
            }
            self.visible = true;
            return;
        }

        let Some(place) = lobby.net.players.keys().position(|id| *id == self.player) else { return };
        let player = &lobby.net.players[&self.player];
        self.x = ROW_LEFT + (place % ROW_PLACES) as f64 * ROW_STEP;
        self.y = ROW_Y;
        self.blend = if player.is_ready { C_LIME } else { C_WHITE };
        if in_lobby && player.is_ready != self.is_ready {
            self.offset = JUMP_HEIGHT;
            self.is_ready = player.is_ready;
        } else if !in_lobby {
            self.blend = if self.is_selected { C_WHITE } else { C_GREY };
        }
        self.visible = place / ROW_PLACES == lobby.row_page;
    }

    /// Draw_0
    pub fn draw(&mut self, canvas: &mut Canvas, lobby: &LobbyView, unlockables: &Unlockables) {
        if self.hidden || !self.visible {
            return;
        }
        if lobby.state == super::LobbyState::Lobby {
            self.sprite = sprite::SPR_LOBBY_DICON;
            let icon = if self.is_own(lobby.net) {
                Some(unlockables.lobby_icon as f64)
            } else {
                lobby.net.players.get(&self.player).map(|player| player.icon as f64)
            };
            if let Some(icon) = icon {
                self.image_index = icon;
            }
        }
        // ponytail: choosing a character on the icon (obj_lobby_icon state 2) waits for the "ready" preference cards
        if self.sprite != sprite::SPR_LOBBY_EXEICON5 && lobby.net.exe_ids.contains(&self.player) {
            self.image_speed = 1.0;
        }
        self.y = self.anchor_y - self.offset;
        canvas.draw_sprite_ext(self.sprite, self.image_index, self.x, self.y, self.xscale, 1.0, 0.0, self.blend, 1.0);
        draw_text(canvas, self.x - text_width(canvas, &self.nickname) / 2.0, self.anchor_y + NICKNAME_BELOW, &self.nickname, C_WHITE, 1.0);
        if self.is_selected {
            self.blend = C_WHITE;
        }
    }
}

/// obj_lobby_blood: a splash where a leaving player's icon was, gone after its animation.
pub struct Splash {
    x: f64,
    y: f64,
    image_index: f64,
}

impl Splash {
    pub fn new(x: f64, y: f64) -> Splash {
        Splash { x, y, image_index: 0.0 }
    }

    /// Returns false once the animation reached its last frame.
    pub fn step(&mut self, sprites: &Sprites) -> bool {
        let meta = sprites.get(sprite::SPR_BLOOD2);
        self.image_index += meta.fps as f64 / crate::core::config::ticks_per_second();
        self.image_index < meta.frame_count as f64 - 1.0
    }

    pub fn draw(&self, canvas: &mut Canvas) {
        canvas.draw_sprite(sprite::SPR_BLOOD2, self.image_index, self.x, self.y);
    }
}
