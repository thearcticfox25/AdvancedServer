//! room_results (obj_results, obj_results_icon, net_state_results): how the round
//! ended, its time and a row of numbers for every player, over the map's pictures.

use super::fades::BlackFadeOut;
use super::map_preview::MapPreview;
use super::{count_down_alarm, go_to_error, ALARM_OFF};
use crate::client::net::{self, Notice};
use crate::client::room::Room;
use crate::client::text::{draw_counter, draw_counter_padded, draw_text};
use crate::client::Context;
use anyhow::{Context as _, Result};
use crate::packet::{Packet, PacketType};
use crate::core::resources::names::{sound, sprite};
use crate::core::resources::SpriteId;
use crate::core::objects::ids::ObjectId;
use crate::core::rooms::ids::RoomId;
use crate::core::world::{C_GREY, C_WHITE};
use crate::core::config::step;
use crate::core::config::ticks;

/// Rows start here and go down by this much (instance_create_layer(3, 33 + 30 * index)).
const ROW_LEFT: f64 = 3.0;
const ROW_TOP: f64 = 33.0;
const ROW_STEP: f64 = 30.0;
/// The music starts a few steps after the first row arrives (alarm[0] = 4).
const MUSIC_DELAY_SECONDS: f64 = 4.0 / 60.0;
/// SERVER_RESULTS_DATA end byte: Ending of the server.
const EXE_WON: u8 = 0;
const SURVIVORS_WON: u8 = 1;
/// spr_counter frame of the colon between minutes and seconds.
const COUNTER_COLON_FRAME: f64 = 10.0;
/// An EXE icon animates at this many frames per step.
const EXE_ICON_FRAMES_PER_STEP: f64 = 0.15;

/// How a player left the round, the type byte of SERVER_RESULTS_DATA.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Outcome {
    Exe,
    Demonized,
    Dead,
    Alive,
    Escaped,
}

/// One SERVER_RESULTS_DATA.
#[derive(Clone, Debug, PartialEq)]
pub struct ResultsRow {
    nickname: String,
    character: i8,
    ending: u8,
    /// Seconds the round lasted.
    round_seconds: u16,
    has_quit: bool,
    outcome: Outcome,
    rings: u16,
    kills: u16,
    damage: u16,
    damage_taken: u16,
    stun_time: u16,
    stuns: u16,
    hp_restored: u16,
    survive_seconds: u32,
    danger_seconds: u32,
}

impl ResultsRow {
    pub fn read(packet: &mut Packet) -> Option<ResultsRow> {
        let nickname = packet.read_str()?;
        let character = packet.read_i8()?;
        let ending = packet.read_u8()?;
        let round_seconds = packet.read_u16()?;
        let has_quit = packet.read_u8()? != 0;
        let outcome = match packet.read_u8()? {
            0 => Outcome::Exe,
            1 => Outcome::Demonized,
            2 => Outcome::Dead,
            4 => Outcome::Escaped,
            _ => Outcome::Alive,
        };
        let mut numbers = [0u16; 7];
        for number in &mut numbers {
            *number = packet.read_u16()?;
        }
        let [rings, kills, damage, damage_taken, stun_time, stuns, hp_restored] = numbers;
        // The server writes the times in ticks as a float followed by four zero bytes.
        let mut ticks_as_seconds = || -> Option<u32> {
            let ticks = packet.read_f32()?;
            packet.read_u32()?;
            Some((ticks / 60.0).floor().max(0.0) as u32)
        };
        let survive_seconds = ticks_as_seconds()?;
        let danger_seconds = ticks_as_seconds()?;
        Some(ResultsRow {
            nickname,
            character,
            ending,
            round_seconds,
            has_quit,
            outcome,
            rings,
            kills,
            damage,
            damage_taken,
            stun_time,
            stuns,
            hp_restored,
            survive_seconds,
            danger_seconds,
        })
    }
}

pub struct Results {
    room: Room,
    preview_layer: usize,
    results_layer: usize,
    icons_layer: usize,
    fade_layer: usize,
    bar_position: (f64, f64),
    preview: MapPreview,
    rows: Vec<(ResultsRow, f64)>,
    minutes: i32,
    seconds: i32,
    /// result: the ending shown by the bar and the music.
    ending: u8,
    show_black: bool,
    music_alarm: i32,
    fade_out: Option<BlackFadeOut>,
}

impl Results {
    pub fn open(context: &mut Context) -> Result<Results> {
        let room = Room::load(&context.maps_folder, RoomId::Results, &context.canvas.sprites)?;
        let (preview_layer, _) = room.placed(ObjectId::LobbyProlet).next().context("room_results has no obj_lobby_prolet")?;
        let (results_layer, placed) = room.placed(ObjectId::Results).next().context("room_results has no obj_results")?;
        let bar_position = (placed.x, placed.y);
        let icons_layer = layer_named(&room, "Results")?;
        let fade_layer = layer_named(&room, "Instances_1")?;
        let mut preview = MapPreview::new();
        preview.visible = true;
        preview.set_zone(context.net.level.unwrap_or(0) as usize);
        // Create_0 of obj_results.
        context.net.send_reliable(&Packet::new(PacketType::CLIENT_RESULTS_REQUEST));
        context.audio.stop_all();
        Ok(Results {
            room,
            preview_layer,
            results_layer,
            icons_layer,
            fade_layer,
            bar_position,
            preview,
            rows: Vec::new(),
            minutes: 0,
            seconds: 0,
            ending: EXE_WON,
            show_black: true,
            music_alarm: ALARM_OFF,
            fade_out: None,
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
        for notice in net::update(context) {
            match notice {
                Notice::GoTo(room) => return Some(room),
                Notice::ShowError(code) => return Some(go_to_error(context, code)),
                Notice::ResultsRow(row) => self.add_row(row),
                _ => {}
            }
        }
        if count_down_alarm(&mut self.music_alarm) {
            let music = match self.ending {
                EXE_WON => sound::MUS_EXEWIN,
                SURVIVORS_WON => sound::MUS_SURVWIN,
                _ => sound::MUS_TIMEOVER,
            };
            context.audio.play_music(music);
        }
        if self.fade_out.as_mut().is_some_and(|fade| !fade.step()) {
            self.fade_out = None;
        }
        None
    }

    /// SERVER_RESULTS_DATA
    fn add_row(&mut self, row: ResultsRow) {
        self.minutes = (row.round_seconds / 60) as i32;
        self.seconds = (row.round_seconds % 60) as i32;
        self.show_black = false;
        self.ending = row.ending;
        self.music_alarm = ticks(MUSIC_DELAY_SECONDS);
        if row.ending == EXE_WON {
            self.preview.black_white = true;
        }
        self.fade_out = Some(BlackFadeOut::new());
        self.rows.push((row, 0.0));
    }

    fn draw(&mut self, context: &mut Context, current_time_ms: f64) {
        let canvas = &mut context.canvas;
        for layer_index in 0..self.room.layers.len() {
            let layer = &self.room.layers[layer_index];
            if !layer.visible {
                continue;
            }
            self.room.draw_layer(canvas, layer);
            if layer_index == self.preview_layer {
                self.preview.draw(canvas, current_time_ms);
            }
            if layer_index == self.results_layer {
                let (x, y) = self.bar_position;
                canvas.draw_sprite(sprite::SPR_RESULTS_BAR, self.ending as f64, x, y);
                canvas.draw_sprite(sprite::SPR_COUNTER, COUNTER_COLON_FRAME, 233.0, 5.0);
                draw_counter(canvas, self.minutes, 224.0, 5.0);
                draw_counter_padded(canvas, self.seconds, 244.0, 5.0, 2);
            }
            if layer_index == self.icons_layer {
                for (index, (row, frame)) in self.rows.iter_mut().enumerate() {
                    draw_row(canvas, row, frame, ROW_LEFT, ROW_TOP + ROW_STEP * index as f64);
                }
            }
            if layer_index == self.fade_layer {
                if let Some(fade) = &self.fade_out {
                    fade.draw_gui(canvas);
                }
            }
        }
        if self.show_black {
            canvas.draw_sprite(sprite::SPR_BLACK, 0.0, 0.0, 0.0);
        }
    }
}

fn layer_named(room: &Room, name: &str) -> Result<usize> {
    room.layers.iter().position(|layer| layer.name == name).with_context(|| format!("room_results has no layer {name}"))
}

/// Draw_0 of obj_results_icon: the player's numbers, icon, name and frame.
fn draw_row(canvas: &mut crate::client::canvas::Canvas, row: &ResultsRow, frame: &mut f64, x: f64, y: f64) {
    let blend = if row.has_quit { C_GREY } else { C_WHITE };
    let character = row.character as f64;
    let (icon, icon_frame): (SpriteId, f64) = match row.outcome {
        Outcome::Exe => {
            let icons = [sprite::SPR_LOBBY_EXEICON, sprite::SPR_LOBBY_EXEICON2, sprite::SPR_LOBBY_EXEICON3, sprite::SPR_LOBBY_EXEICON4];
            (icons.get(row.character as usize).copied().unwrap_or(sprite::SPR_LOBBY_EXEICON), *frame)
        }
        Outcome::Demonized => (sprite::SPR_PLAYERHEALTH_DEMON, character),
        Outcome::Dead => (sprite::SPR_PLAYERHEALTH, 6.0 + character * 7.0),
        Outcome::Alive => (sprite::SPR_LOBBY_ICON, 2.0 + character),
        Outcome::Escaped => (sprite::SPR_PLAYERESCAPED, character),
    };
    let (icon_x, icon_y) = match row.outcome {
        Outcome::Exe => {
            *frame += EXE_ICON_FRAMES_PER_STEP * step();
            (26.0 / 2.0, 26.0 / 2.0)
        }
        Outcome::Alive => (24.0 / 2.0, 24.0 / 2.0 + 1.0),
        _ => (0.0, canvas.sprites.get(icon).height as f64),
    };

    let text = |canvas: &mut crate::client::canvas::Canvas, offset: f64, value: &str| {
        draw_text(canvas, x + offset, y + 17.0, value, blend, 1.0);
    };
    let frame_kind = match row.outcome {
        Outcome::Exe | Outcome::Demonized => {
            text(canvas, 47.0, &row.kills.to_string());
            text(canvas, 83.0, &row.damage.to_string());
            text(canvas, 116.0, &row.rings.to_string());
            text(canvas, 152.0, &format!("{} sec.", row.stun_time));
            if row.outcome == Outcome::Demonized {
                text(canvas, 194.0 + 9.0, &minutes_and_seconds(row.survive_seconds));
                1.0
            } else {
                2.0
            }
        }
        _ => {
            text(canvas, 47.0, &minutes_and_seconds(row.survive_seconds));
            text(canvas, 92.0, &minutes_and_seconds(row.danger_seconds));
            text(canvas, 132.0, &row.rings.to_string());
            text(canvas, 168.0, &row.hp_restored.to_string());
            text(canvas, 197.0, &row.damage_taken.to_string());
            text(canvas, 226.0, &row.stuns.to_string());
            0.0
        }
    };
    canvas.draw_sprite_ext(icon, icon_frame, x + icon_x, y + icon_y, 1.0, 1.0, 0.0, blend, 1.0);
    draw_text(canvas, x + 34.0, y + 3.0, &row.nickname, blend, 1.0);
    canvas.draw_sprite_ext(sprite::SPR_RESULTS_ICONS, frame_kind, x, y, 1.0, 1.0, 0.0, blend, 1.0);
}

/// floor(t / 60):string_format(t % 60, 2, 0) with the padding as zeros.
fn minutes_and_seconds(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_results_row_reads_as_the_server_writes_it() {
        let mut packet = Packet::new(PacketType::SERVER_RESULTS_DATA);
        let _ = packet.write_str("fox");
        let _ = packet.write_u8(2);
        let _ = packet.write_u8(1);
        let _ = packet.write_u16(95);
        let _ = packet.write_u8(0);
        let _ = packet.write_u8(4);
        for number in [5u16, 0, 0, 40, 0, 1, 20] {
            let _ = packet.write_u16(number);
        }
        for ticks in [3660.0f32, 600.0] {
            let _ = packet.write_f32(ticks);
            let _ = packet.write_u32(0);
        }
        packet.pos = 2;
        let row = ResultsRow::read(&mut packet).expect("a whole row");
        assert_eq!((row.nickname.as_str(), row.outcome, row.rings, row.hp_restored), ("fox", Outcome::Escaped, 5, 20));
        assert_eq!((row.survive_seconds, row.danger_seconds), (61, 10));
        assert_eq!(minutes_and_seconds(row.survive_seconds), "1:01");
    }
}
