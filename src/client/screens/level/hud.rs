//! Draw GUI of a level, back to front: the health, name and rings above other
//! players (obj_player_puppet) and above this player (the character objects), then
//! obj_level (screen tint, hiding and hit flashes, clock, everyone's health icons,
//! title card, ping, the player list).

use crate::client::canvas::{make_color_rgb, Canvas, C_DKGRAY, C_GREEN, C_RED, VIEW_WIDTH};
use crate::client::screens::round_clock::RoundClock;
use crate::client::text::{draw_text, text_width};
use crate::core::resources::names::sprite;
use crate::core::resources::SpriteId;
use crate::core::config::GameplayConfig;
use crate::core::player::{Character, ExeCharacter, Player};
use crate::core::snapshot::HealthView;
use crate::core::rooms::ids::RoomId;
use crate::core::world::C_WHITE;
use crate::core::config::step;

/// Rooms without the dark screen overlay.
/// spr_badconnection: its frames, shown one every 200 ms.
const BAD_CONNECTION_FRAMES: f64 = 6.0;
const ROOMS_WITHOUT_OVERLAY: [RoomId; 10] = [
    RoomId::Notperfect,
    RoomId::Act9,
    RoomId::Youcantrun,
    RoomId::Nastyparadise,
    RoomId::Ravinemist,
    RoomId::Volcanovalley,
    RoomId::Greenhill,
    RoomId::Angelisland,
    RoomId::Fartzone,
    RoomId::Weedzone,
];
const OVERLAY_ALPHA: f64 = 0.7;
const FLASH_FADE_PER_STEP: f64 = 0.02;
/// The HUD waits for the title card to get this far.
const TITLE_CARD_HUD_FROM: f64 = 4.0;
const TITLE_CARD_STEP: f64 = 0.12;
const TITLE_CARD_SLIDE: f64 = 12.0;
/// The title card slides in until 8, the black screen fades out from 8 to 16,
/// the card slides out after 18 and is gone at 30.
const TITLE_CARD_IN_UNTIL: f64 = 8.0;
const TITLE_CARD_BLACK_FROM: f64 = 8.0;
const TITLE_CARD_BLACK_UNTIL: f64 = 16.0;
const TITLE_CARD_BLACK_SHOWN_UNTIL: f64 = 18.0;
const TITLE_CARD_OUT_FROM: f64 = 18.0;
const TITLE_CARD_GONE_AT: f64 = 30.0;
/// spr_playerhealth: 7 frames per survivor, health in 5 steps, then downed and dead.
use crate::client::screens::character_art::HEALTH_FRAMES_PER_CHARACTER;
const HEALTH_STEPS: f64 = 5.0;
const DOWNED_FRAME: f64 = 5.0;
const DEAD_FRAME: f64 = 6.0;
const ICON_SPACING: f64 = 28.0;
const ICON_Y: f64 = 268.0;
/// The death countdown shows once it is below its start value.
const DEATH_COUNTDOWN_START: i32 = 31;
/// Above-head bars: this far above someone else's player; above this player it depends on the character.
const OTHER_BAR_ABOVE: f64 = 45.0;
const OTHER_NAME_ABOVE: f64 = 43.0;
const RINGS_ABOVE_BAR: f64 = 12.0;
/// obj_exe Draw_64 draws the frozen mark 20 px above EXE.
const EXE_FROZEN_ABOVE: f64 = 20.0;
/// obj_majong_controller's copy of a name plate at the other end of Majin Forest.
const WRAPPED_BAR_ABOVE: f64 = 50.0;
const WRAPPED_NAME_ABOVE: f64 = 40.0;
const PING_WARNING_MS: i32 = 80;
const PING_BAD_MS: i32 = 160;
/// The sign over a player whose pause menu is open, above their name plate.
const AFK_SIGN: &str = "afk";
const AFK_ABOVE: f64 = 54.0;
const PLAYER_LIST_TOP: f64 = 44.0;
const PLAYER_LIST_ROW: f64 = 30.0;
const NUMBER_SPACING: f64 = 5.0;

fn shards_colour() -> u32 {
    make_color_rgb(0x27, 0xFF, 0x23)
}

fn ping_good_colour() -> u32 {
    make_color_rgb(0x0F, 0xFF, 0x39)
}

/// What the HUD shows of a player of the round.
pub struct HudPlayer<'a> {
    pub nickname: &'a str,
    /// None once the player has escaped, and while this client may not see them
    /// (states.gameplay.limit_player_view): then only `health` is known about them.
    pub player: Option<&'a Player>,
    /// What the health row and the player list show about them wherever they are.
    /// None once they have escaped, and for a spectator, who plays nobody.
    pub health: Option<HealthView>,
    /// The character they play, known even after escaping (1..6 survivors).
    pub character: Character,
    pub is_exe: bool,
    /// SERVER_GAME_DEATHTIMER_TICK: seconds left and whether EXE camps the body.
    pub death_timer: Option<(u8, bool)>,
    pub ping_ms: i32,
}

pub struct Hud {
    room: RoomId,
    /// A level played alone (Singleplayer): no clock, no health icons, no ping and no
    /// player list, because there is neither a round nor anyone else.
    solo: bool,
    /// Watching a round nobody here plays (states::spectate): the row of health icons
    /// and the player list leave out the row this client would have had.
    spectating: bool,
    /// global.showHud
    pub shown: bool,
    pub clock: RoundClock,
    /// bloodFade: set by a hit, fades out.
    pub blood_fade: f64,
    hide_fade: f64,
    /// obj_level state 1: the big ring is out, survivors can escape.
    pub escape: bool,
    /// obj_netclient.udp_timeout < 60 * 3: the server has been silent for a while.
    pub bad_connection: bool,
    title_card: f64,
    card_x: f64,
    card_y: f64,
    level_number: f64,
    /// HurtRules.max_hp: how many hits a survivor has, which the bars and the icons
    /// of the bottom row draw one step per.
    max_hp: i32,
}

impl Hud {
    pub fn new(room: RoomId, level_number: usize, solo: bool, spectating: bool, max_hp: i32) -> Hud {
        Hud {
            room,
            solo,
            spectating,
            shown: true,
            clock: RoundClock::default(),
            blood_fade: 0.0,
            hide_fade: 0.0,
            escape: false,
            bad_connection: false,
            title_card: 0.0,
            card_x: crate::client::canvas::VIEW_WIDTH,
            card_y: -crate::client::canvas::VIEW_HEIGHT,
            level_number: level_number as f64,
            max_hp,
        }
    }

    /// obj_level Draw_64. `own` is this player, `others` everyone else of the round.
    pub fn draw(&mut self, canvas: &mut Canvas, own: &HudPlayer, others: &[HudPlayer], options: &crate::client::options::Options, current_time_ms: f64, player_list_held: bool) {
        if !ROOMS_WITHOUT_OVERLAY.contains(&self.room) {
            canvas.draw_sprite_ext(sprite::SPR_SCREENOVERLAY, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, C_WHITE, OVERLAY_ALPHA);
        }
        canvas.draw_sprite_ext(sprite::SPR_HIDEGUI, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, C_WHITE, self.hide_fade);
        canvas.draw_sprite_ext(sprite::SPR_ATTACKGUI, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, C_WHITE, self.blood_fade);
        if self.blood_fade > 0.0 {
            self.blood_fade -= FLASH_FADE_PER_STEP * step();
        }
        let hiding = own.player.is_some_and(|player| player.is_hiding);
        if hiding && self.hide_fade < 1.0 {
            self.hide_fade += FLASH_FADE_PER_STEP * step();
        } else if !hiding && self.hide_fade > 0.0 {
            self.hide_fade -= FLASH_FADE_PER_STEP * step();
        }

        if self.shown && !self.solo && self.title_card > TITLE_CARD_HUD_FROM {
            if self.bad_connection {
                canvas.draw_sprite(sprite::SPR_BADCONNECTION, ((current_time_ms / 200.0).floor() % BAD_CONNECTION_FRAMES).floor(), 0.0, 0.0);
            }
            self.clock.draw(canvas);
            if self.escape && (current_time_ms / 30.0).floor() % 20.0 < 10.0 {
                let demon = own.is_exe || own.player.is_some_and(|player| player.revival_times >= 2);
                canvas.draw_sprite(sprite::SPR_STATUS, if demon { 1.0 } else { 0.0 }, 240.0, 32.0);
            }
            self.draw_health_icons(canvas, own, others);
        }
        self.draw_title_card(canvas);
        // A spectator has no round trip time of its own to show: it sends nothing to
        // be answered, and the round is not waiting for it either.
        if self.shown && !self.solo && !self.spectating {
            if own.ping_ms <= 0 {
                draw_text(canvas, 4.0, 4.0, &format!("waiting for players{}", loading_dots(current_time_ms)), C_WHITE, 1.0);
            } else if options.show_ping {
                draw_text(canvas, 4.0, 4.0, &format!("{}ms", own.ping_ms), ping_colour(own.ping_ms), 1.0);
            }
        }
        if self.shown && options.show_fps {
            draw_text(canvas, 4.0, 10.0, &format!("{}fps", macroquad::time::get_fps()), C_WHITE, 1.0);
        }
        if player_list_held && !self.solo {
            self.draw_player_list(canvas, own, others, current_time_ms);
        }
    }

    /// The row of health icons at the bottom: this player first, then the others, EXE left out.
    fn draw_health_icons(&self, canvas: &mut Canvas, own: &HudPlayer, others: &[HudPlayer]) {
        let mut x = crate::client::canvas::VIEW_WIDTH / 2.0 - (others.len() as f64 * ICON_SPACING) / 2.0;
        if !own.is_exe && !self.spectating {
            draw_health_icon(canvas, own, x, true, self.max_hp);
            x += ICON_SPACING;
        }
        for (place, other) in others.iter().filter(|other| !other.is_exe).enumerate() {
            draw_health_icon(canvas, other, x + place as f64 * ICON_SPACING, false, self.max_hp);
        }
    }

    fn draw_title_card(&mut self, canvas: &mut Canvas) {
        if self.title_card < TITLE_CARD_IN_UNTIL {
            if self.card_x > 0.0 {
                self.card_x -= TITLE_CARD_SLIDE * step();
            }
            if self.card_y < 0.0 {
                self.card_y += TITLE_CARD_SLIDE * step();
            }
        }
        if self.title_card < TITLE_CARD_BLACK_SHOWN_UNTIL {
            let faded = (self.title_card.clamp(TITLE_CARD_BLACK_FROM, TITLE_CARD_BLACK_UNTIL) - TITLE_CARD_BLACK_FROM) / (TITLE_CARD_BLACK_UNTIL - TITLE_CARD_BLACK_FROM);
            canvas.draw_sprite_ext(sprite::SPR_BLACK, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, C_WHITE, 1.0 - faded);
        }
        if self.title_card > TITLE_CARD_OUT_FROM {
            if self.card_x < crate::client::canvas::VIEW_WIDTH {
                self.card_x += TITLE_CARD_SLIDE * step();
            }
            if self.card_y > -crate::client::canvas::VIEW_HEIGHT {
                self.card_y -= TITLE_CARD_SLIDE * step();
            }
        }
        if self.title_card < TITLE_CARD_GONE_AT {
            self.card_x = self.card_x.clamp(0.0, crate::client::canvas::VIEW_WIDTH);
            self.card_y = self.card_y.clamp(-crate::client::canvas::VIEW_HEIGHT, 0.0);
            canvas.draw_sprite(sprite::SPR_TITLECARD2, self.level_number, 0.0, self.card_y);
            canvas.draw_sprite(sprite::SPR_TITLECARD1, self.level_number, self.card_x, 0.0);
            self.title_card += TITLE_CARD_STEP * step();
        }
    }

    /// The player list while its key is held (drawPing of obj_level).
    fn draw_player_list(&self, canvas: &mut Canvas, own: &HudPlayer, others: &[HudPlayer], current_time_ms: f64) {
        canvas.draw_sprite(sprite::SPR_TAB, 0.0, 0.0, 0.0);
        let own_row = (!self.spectating).then_some(own);
        for (row, entry) in own_row.into_iter().chain(others).enumerate() {
            let y = PLAYER_LIST_TOP + row as f64 * PLAYER_LIST_ROW;
            let frame = list_frame(entry);
            canvas.draw_sprite(sprite::SPR_TAB_ENTRY, frame, 240.0, y);
            let revival_times = entry.health.map_or(0, |health| health.revival_times);
            if !entry.is_exe {
                if revival_times >= 2 {
                    canvas.draw_sprite(sprite::SPR_PLAYERHEALTH_DEMON, frame, 240.0 - 113.0 + 1.0, y - 1.0 + 28.0);
                } else if revival_times >= 1 {
                    canvas.draw_sprite(sprite::SPR_PLAYERHEALTH_HIT, frame, 240.0 - 113.0 + 1.0, y - 1.0 + 28.0);
                }
            }
            let (ping_text, colour) = if entry.ping_ms <= 0 {
                (loading_dots(current_time_ms), C_RED)
            } else {
                (format!("{}ms", entry.ping_ms), ping_colour(entry.ping_ms))
            };
            draw_text(canvas, 127.0 + 59.0, y + 11.0, entry.nickname, C_WHITE, 1.0);
            draw_text(canvas, 127.0 + 183.0, y + 11.0, &ping_text, colour, 1.0);
        }
    }

    /// Draw_64 of the character objects: health and rings above this survivor.
    pub fn draw_above_own(&self, canvas: &mut Canvas, player: &Player, view: (f64, f64)) {
        if player.character == Character::Exe {
            // obj_exe Draw_64: no health above EXE, only the mark of being slowed.
            if player.is_slow {
                let (x, y) = ((player.x - view.0).ceil(), (player.y - view.1).ceil() - EXE_FROZEN_ABOVE);
                canvas.draw_sprite(sprite::SPR_FROZEN, 0.0, x, y);
            }
            return;
        }
        if player.hp <= 0 {
            return;
        }
        let colour = if self.room == RoomId::Act9 { C_DKGRAY } else { C_WHITE };
        let above = match player.character {
            Character::Knux | Character::Amy | Character::Sally => 25.0,
            Character::Eggman => 36.0,
            _ => 20.0,
        };
        let (x, y) = ((player.x - view.0).ceil(), (player.y - view.1).ceil() - above);
        let bar = if player.red_ring_timer > 0 { sprite::SPR_HP_RR } else { sprite::SPR_HP };
        let frame = if player.revival_times == 2 { 0.0 } else { health_bar_frame(player.hp, self.max_hp) };
        canvas.draw_sprite_ext(bar, frame, x, y, 1.0, 1.0, 0.0, colour, 1.0);
        if player.is_slow {
            canvas.draw_sprite(sprite::SPR_FROZEN, 0.0, x, y);
        }
        if player.revival_times < 2 {
            draw_number(canvas, x - 2.0, y - RINGS_ABOVE_BAR, player.rings, colour);
        }
    }

    /// Draw_64 of obj_player_puppet: health, name and rings above someone else.
    pub fn draw_above_other(&self, canvas: &mut Canvas, other: &HudPlayer, view: (f64, f64)) {
        let Some(player) = other.player else { return };
        // CLIENT_ROUND_PAUSE: whoever is in their pause menu stands there as away.
        if player.paused {
            let (x, y) = ((player.x - view.0).ceil(), (player.y - view.1).ceil() - AFK_ABOVE);
            draw_text(canvas, x - text_width(canvas, AFK_SIGN) / 2.0, y, AFK_SIGN, C_WHITE, 1.0);
        }
        let plate = Plate { x: player.x, bar_above: OTHER_BAR_ABOVE, name_above: OTHER_NAME_ABOVE, name_width: text_width(canvas, other.nickname), name_shown: self.shown };
        self.draw_plate(canvas, other, player, &plate, view);
    }

    /// obj_majong_controller Draw GUI: on Majin Forest someone near one end also shows
    /// at the other, where the room repeats itself, with slightly different spacing.
    pub fn draw_wrapped_above_other(&self, canvas: &mut Canvas, other: &HudPlayer, view: (f64, f64), room_width: f64) {
        let Some(player) = other.player else { return };
        let x = if (0.0..=VIEW_WIDTH).contains(&player.x) {
            room_width - VIEW_WIDTH + player.x
        } else if (room_width - VIEW_WIDTH..=room_width).contains(&player.x) {
            player.x - (room_width - VIEW_WIDTH)
        } else {
            return;
        };
        let plate = Plate { x, bar_above: WRAPPED_BAR_ABOVE, name_above: WRAPPED_NAME_ABOVE, name_width: other.nickname.len() as f64 * 8.0, name_shown: true };
        self.draw_plate(canvas, other, player, &plate, view);
    }

    fn draw_plate(&self, canvas: &mut Canvas, other: &HudPlayer, player: &Player, plate: &Plate, view: (f64, f64)) {
        if player.is_hiding {
            return;
        }
        let act9 = self.room == RoomId::Act9;
        let colour = if act9 { C_DKGRAY } else { C_WHITE };
        let (x, y) = ((plate.x - view.0).floor(), (player.y - view.1).floor());
        let demon = other.is_exe || player.revival_times >= 2;
        if player.hp > 0 && !other.is_exe {
            let bar = if player.red_ring_timer > 0 { sprite::SPR_HP_RR } else { sprite::SPR_HP };
            let frame = if demon { 0.0 } else { health_bar_frame(player.hp, self.max_hp) };
            canvas.draw_sprite_ext(bar, frame, x, y - plate.bar_above, 1.0, 1.0, 0.0, colour, 1.0);
        }
        if plate.name_shown {
            let name_colour = if act9 {
                C_DKGRAY
            } else if demon {
                C_RED
            } else if player.red_ring_timer > 0 {
                make_color_rgb(0x99, 0x43, 0xAD)
            } else {
                C_GREEN
            };
            draw_text(canvas, x - plate.name_width / 2.0, y - plate.name_above, other.nickname, name_colour, 1.0);
        }
        if !demon && player.hp > 0 {
            draw_number(canvas, x - 2.0, y - plate.bar_above - RINGS_ABOVE_BAR, player.rings, colour);
        }
    }
}

/// Where a name plate stands above someone.
struct Plate {
    x: f64,
    bar_above: f64,
    name_above: f64,
    name_width: f64,
    name_shown: bool,
}

/// One health icon of the bottom row. It is drawn from what every client knows about
/// everyone (HealthView), not from the body, which distance may hide.
fn draw_health_icon(canvas: &mut Canvas, entry: &HudPlayer, x: f64, is_own: bool, max_hp: i32) {
    let character = entry.character as usize as f64 - 1.0;
    let Some(health) = entry.health else {
        canvas.draw_sprite(sprite::SPR_PLAYERESCAPED, character, x, ICON_Y);
        return;
    };
    if health.revival_times >= 2 {
        canvas.draw_sprite(sprite::SPR_PLAYERHEALTH_DEMON, character, x, ICON_Y);
        return;
    }
    let base = character * HEALTH_FRAMES_PER_CHARACTER;
    let frame = if health.hp <= 0 {
        base + if health.revival_times == 0 { DOWNED_FRAME } else { DEAD_FRAME }
    } else {
        base + hits_lost(health.hp, max_hp)
    };
    let icon = if health.red_ring { sprite::SPR_PLAYERHEALTH_REDRING } else { sprite::SPR_PLAYERHEALTH };
    canvas.draw_sprite(icon, frame, x, ICON_Y);
    if health.revival_times >= 1 && health.hp > 0 {
        canvas.draw_sprite(sprite::SPR_PLAYERHEALTH_HIT, character, x, ICON_Y);
    }
    let countdown = entry.death_timer.filter(|&(seconds, _)| health.revival_times == 0 && (seconds as i32) < DEATH_COUNTDOWN_START);
    let downed = if is_own { health.is_dead } else { health.hp <= 0 };
    if let (true, Some((seconds, exe_near))) = (downed, countdown) {
        draw_number(canvas, x + 12.0, ICON_Y - 4.0, seconds as i32, if exe_near { C_RED } else { C_WHITE });
    } else if let (true, Some(player)) = (is_own, entry.player) {
        if !player.is_dead && player.shards > 0 {
            draw_number(canvas, x + 12.0, ICON_Y - 4.0, player.shards, shards_colour());
        }
    }
}

/// spr_tab_entry frame: survivors 0..5, then exe ... exeller.
fn list_frame(entry: &HudPlayer) -> f64 {
    match entry.health.filter(|_| entry.is_exe) {
        Some(health) => 6.0 + health.exe_character as usize as f64,
        None => entry.character as usize as f64 - 1.0,
    }
}

/// spr_hp frame of a health value: 5 full, 1 nearly empty.
/// spr_hp: one frame per hit left, the last of them for the last hit.
/// ponytail: the picture has five frames, so a server that gives survivors more hits
/// than that shows a full bar until the last five; draw a number beside it if that happens.
fn health_bar_frame(hp: i32, max_hp: i32) -> f64 {
    (HEALTH_STEPS - hits_lost(hp, max_hp)).max(1.0)
}

/// How many steps of the five the health has fallen by: none while untouched, all
/// five once it is gone. A server with a different `max_hp` spreads the same five over it.
fn hits_lost(hp: i32, max_hp: i32) -> f64 {
    let left = (hp as f64 / max_hp.max(1) as f64).clamp(0.0, 1.0);
    ((1.0 - left) * HEALTH_STEPS).round_ties_even()
}

/// string_copy("...", 0, (current_time / 500) % 4)
fn loading_dots(current_time_ms: f64) -> String {
    ".".repeat(((current_time_ms / 500.0) % 4.0).floor() as usize)
}

fn ping_colour(ping: i32) -> u32 {
    if ping >= PING_BAD_MS {
        C_RED
    } else if ping >= PING_WARNING_MS {
        make_color_rgb(0xFF, 0xC4, 0x00)
    } else {
        ping_good_colour()
    }
}

/// scr_number_spr: digits of spr_number centred a little to the left per extra digit.
fn draw_number(canvas: &mut Canvas, x: f64, y: f64, number: i32, colour: u32) {
    let digits = number.to_string();
    let shift = (digits.len() as f64 - 1.0) * 2.0;
    for (place, digit) in digits.chars().enumerate() {
        let frame = digit as i32 - '0' as i32;
        canvas.draw_sprite_ext(sprite::SPR_NUMBER, frame as f64, x - shift + place as f64 * NUMBER_SPACING, y, 1.0, 1.0, 0.0, colour, 1.0);
    }
}

/// obj_playerui Draw GUI: how ready this player's abilities are, a gauge per
/// ability that slides in and turns white once ready. `up_or_down_held`: Exeller
/// shows the teleport gauge while either is held.
pub fn draw_ability_gauges(canvas: &mut Canvas, player: &Player, cfg: &GameplayConfig, controls_enabled: bool, up_or_down_held: bool, shown: bool) {
    if !shown || player.is_dead {
        return;
    }
    let ticks = |seconds: f64| crate::core::config::ticks(seconds) as f64;
    let demon = player.revival_times >= 2;
    let frame_if = |on: bool| if on { 1.0 } else { 0.0 };
    // A gauge filled to `progress`, sliding in from the left, white once `ready`.
    let gauge = |canvas: &mut Canvas, sprite: SpriteId, frame: f64, slide: f64, y: f64, progress: f64, ready: bool| {
        let colour = if ready { C_WHITE } else { C_RED };
        canvas.draw_sprite_ext(sprite, frame, progress * slide, y, 1.0, 1.0, 0.0, colour, progress);
    };
    let recharged = |timer: i32, recharge: f64| 1.0 - (timer.max(0) as f64) / recharge;
    // The free jump sign stands still: white and solid while a jump can be steered.
    let free_jump = controls_enabled && player.is_jumping;
    let free_jump_sign = |canvas: &mut Canvas, can_show: bool| {
        let (colour, alpha) = match (can_show, free_jump) {
            (false, _) => (C_RED, 0.0),
            (true, true) => (C_WHITE, 1.0),
            (true, false) => (C_RED, 0.5),
        };
        canvas.draw_sprite_ext(sprite::SPR_GUI_EXEFREEJUMP, 0.0, 3.0, 218.0, 1.0, 1.0, 0.0, colour, alpha);
    };

    match player.character {
        Character::Exe => match player.exe_character {
            ExeCharacter::Original => {
                let attack = 1.0 - (player.attack_timer.min(180) as f64) / 180.0;
                gauge(canvas, sprite::SPR_GUI_EXEATTACK, 0.0, 10.0, 240.0, attack, attack == 1.0);
                free_jump_sign(canvas, true);
                let invisibility = (player.invis_timer.min(0) as f64) / -ticks(cfg.exe.invisibility_recharge_seconds);
                gauge(canvas, sprite::SPR_GUI_EXEINVISABILITY, 0.0, 4.0, 250.0, invisibility, invisibility == 1.0);
            }
            ExeCharacter::Chaos => {
                if player.stuck_timer <= 30 / 2 {
                    let attack = if player.slime_timer > 0 { 0.0 } else { 1.0 - (player.attack_timer.min(120) as f64) / 120.0 };
                    gauge(canvas, sprite::SPR_GUI_CHAOSATTACK, frame_if(!player.is_grounded), 10.0, 240.0, attack, attack == 1.0);
                } else {
                    let frame = match (player.is_grounded, player.stuck_dir) {
                        (true, dir) if dir > 0.0 => 1.0,
                        (true, dir) if dir < 0.0 => 0.0,
                        _ => 2.0,
                    };
                    gauge(canvas, sprite::SPR_GUI_CHAOSWALLDASH, frame, 10.0, 240.0, 1.0, true);
                }
                free_jump_sign(canvas, player.slime_timer <= 0);
                let liquid = if player.slime_timer > 0 { 1.0 } else { (player.slime_timer.min(0) as f64) / -ticks(cfg.chaos.liquid_recharge_seconds) };
                gauge(canvas, sprite::SPR_GUI_CHAOSSLIME, 0.0, 4.0, 250.0, liquid, liquid == 1.0);
            }
            ExeCharacter::Exetior => {
                let attack = 1.0 - (player.attack_timer.min(180) as f64) / 180.0;
                gauge(canvas, sprite::SPR_GUI_EXETIORATTACK, frame_if(!player.is_grounded), 10.0, 240.0, attack, attack == 1.0);
                free_jump_sign(canvas, true);
                let black_ring = 1.0 - player.bring_timer as f64 / ticks(cfg.exetior.black_ring_recharge_seconds);
                gauge(canvas, sprite::SPR_GUI_EXETIORRING, 0.0, 4.0, 250.0, black_ring, black_ring == 1.0);
            }
            ExeCharacter::Exeller => {
                let attack = 1.0 - (player.attack_timer.min(180) as f64) / 180.0;
                gauge(canvas, sprite::SPR_GUI_EXEATTACK, 0.0, 10.0, 240.0, attack, attack == 1.0);
                free_jump_sign(canvas, true);
                let mut clone = 1.0 - player.clone_timer as f64 / ticks(cfg.exeller.clone_recharge_seconds);
                let mut clone_sprite = sprite::SPR_GUI_EXELLERCLONE;
                if up_or_down_held && player.clone_count > 0 {
                    clone_sprite = sprite::SPR_GUI_EXELLERCLONE2;
                    clone = 1.0;
                }
                if clone_sprite == sprite::SPR_GUI_EXELLERCLONE && player.clone_count >= 2 {
                    clone = 0.0;
                }
                gauge(canvas, clone_sprite, 0.0, 4.0, 250.0, clone, clone == 1.0);
            }
        },
        Character::Tails => {
            let flight = (player.fly_timer.min(0) as f64) / -ticks(cfg.tails.flight_recharge_seconds);
            gauge(canvas, sprite::SPR_GUI_TAILSFLY, frame_if(demon), 10.0, 240.0, flight, flight == 1.0);
            let recharge = if demon { cfg.tails.demonized_attack_recharge_seconds } else { cfg.tails.attack_recharge_seconds };
            let attack = 1.0 - player.attack_timer as f64 / ticks(recharge);
            gauge(canvas, sprite::SPR_GUI_TAILSATTACK, 0.0, 12.0, 250.0, attack, attack == 1.0);
        }
        Character::Knux => {
            let glide_recharge = ticks(cfg.knuckles.glide_recharge_seconds);
            let glide = 1.0 - (player.glide_timer as f64).min(glide_recharge) / glide_recharge;
            gauge(canvas, sprite::SPR_GUI_KNUXGLIDE, frame_if(demon), 10.0, 240.0, glide, glide == 1.0);
            let recharge = ticks(if demon { cfg.knuckles.demonized_attack_recharge_seconds } else { cfg.knuckles.attack_recharge_seconds });
            let attack = 1.0 - (player.attack_timer as f64).min(recharge) / recharge;
            gauge(canvas, sprite::SPR_GUI_KNUXATTACK, 0.0, 10.0, 258.0, attack, attack == 1.0);
        }
        Character::Eggman => {
            let double_jump = recharged(player.djump_recharge, ticks(cfg.eggman.double_jump_recharge_seconds));
            gauge(canvas, sprite::SPR_GUI_EGGDJUMP, 0.0, 10.0, 222.0, double_jump, double_jump == 1.0);
            let shield = recharged(player.shield_recharge, ticks(cfg.eggman.shield_recharge_seconds));
            gauge(canvas, sprite::SPR_GUI_EGGSHIELD, 0.0, 10.0, 240.0, shield, shield == 1.0);
            let tracker = recharged(player.tracker_recharge, ticks(cfg.eggman.tracker_recharge_seconds));
            gauge(canvas, sprite::SPR_GUI_EGGTRACK, 0.0, 10.0, 258.0, tracker, player.is_colliding && tracker == 1.0);
        }
        Character::Amy => {
            let big_jump = recharged(player.hjump_timer, ticks(cfg.amy.big_jump_recharge_seconds));
            gauge(canvas, sprite::SPR_GUI_AMYHJUMP, 0.0, 10.0, 240.0, big_jump, big_jump == 1.0);
            // The original shows Amy's hammer against Knuckles' recharge times.
            let recharge = if demon { cfg.knuckles.demonized_attack_recharge_seconds } else { cfg.knuckles.attack_recharge_seconds };
            let attack = recharged(player.attack_timer, ticks(recharge));
            gauge(canvas, sprite::SPR_GUI_AMYATTACK, 0.0, 10.0, 258.0, attack, attack == 1.0);
        }
        Character::Cream => {
            let flight = recharged(player.fly_timer, ticks(cfg.cream.fly_recharge_seconds));
            gauge(canvas, sprite::SPR_GUI_CREAMFLY, frame_if(demon), 10.0, 222.0, flight, flight == 1.0);
            let dash = recharged(player.dash_timer, ticks(cfg.cream.dash_recharge_seconds));
            gauge(canvas, sprite::SPR_GUI_CREAMDASH, 0.0, 10.0, 240.0, dash, dash == 1.0);
            // Undemonized, the original measures the rings against the dash recharge.
            let recharge = if demon { cfg.cream.demonized_rings_recharge_seconds } else { cfg.cream.dash_recharge_seconds };
            let rings = recharged(player.rings_timer, ticks(recharge));
            gauge(canvas, sprite::SPR_GUI_CREAMRINGS, frame_if(demon), 10.0, 257.0, rings, rings == 1.0 && !player.is_colliding);
        }
        Character::Sally => {
            let recharge = if demon { cfg.sally.demonized_attack_recharge_seconds } else { cfg.sally.attack_recharge_seconds };
            let attack = recharged(player.attack_timer, ticks(recharge));
            gauge(canvas, sprite::SPR_GUI_SALLYATTACK, frame_if(demon), 10.0, 240.0, attack, attack == 1.0 && !player.is_grounded);
            let recharge = if demon { cfg.sally.demonized_shield_recharge_seconds } else { cfg.sally.shield_recharge_seconds };
            let shield = recharged(player.shield_recharge, ticks(recharge));
            gauge(canvas, sprite::SPR_GUI_SALLYSHIELD, frame_if(demon), 10.0, 258.0, shield, shield == 1.0);
        }
    }
    if player.character != Character::Exe && (player.state == crate::core::player::IDLE || player.emotion) {
        canvas.draw_sprite(sprite::SPR_GUI_EMOTIONS, 0.0, 470.0, 222.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Health is counted in hits now, and the pictures still have five steps: a
    /// survivor with every hit left shows a full icon and a full bar, one hit from
    /// the end shows the last of each.
    #[test]
    fn the_health_pictures_follow_the_hits_a_survivor_has() {
        let max_hp = GameplayConfig::default().hurt.max_hp;
        assert_eq!(hits_lost(max_hp, max_hp), 0.0, "untouched");
        assert_eq!(hits_lost(1, max_hp), HEALTH_STEPS - 1.0, "one hit left");
        assert_eq!(hits_lost(0, max_hp), HEALTH_STEPS, "gone");
        assert_eq!(health_bar_frame(max_hp, max_hp), HEALTH_STEPS, "a full bar");
        assert_eq!(health_bar_frame(1, max_hp), 1.0, "the last of the bar");
        // A server that gives survivors more or fewer hits spreads the same five steps over them.
        assert_eq!(hits_lost(5, 10), (HEALTH_STEPS / 2.0).round_ties_even(), "half of ten hits");
    }
}
