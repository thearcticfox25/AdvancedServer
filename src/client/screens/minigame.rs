//! The waiting room's minigame (obj_minigame and its objects): Sonic flies right
//! through asteroids, spins through the grey ones to break them, and every 5000
//! points Eggman comes, to be hit with their pieces. Its best score is kept in
//! PersistentData/Game/Minigame.toml (the original's "hiscore").

use crate::client::canvas::Canvas;
use crate::client::input::Input;
use crate::client::options::KeyBindings;
use crate::client::text::draw_text;
use crate::core::collision::{sprite_bbox, Bbox};
use crate::core::config::{step, ticks};
use crate::core::resources::names::{sound, sprite};
use crate::core::resources::sprites::Sprites;
use crate::core::resources::SpriteId;
use crate::core::world::C_WHITE;
use macroquad::rand::gen_range;
use serde::{Deserialize, Serialize};
use std::path::Path;

const HISCORE_FILE: &str = "PersistentData/Game/Minigame.toml";

/// The playing field: Sonic keeps within it, rocks come in at its right.
const FIELD_TOP: f64 = 34.0;
const FIELD_BOTTOM: f64 = 115.0;
const FIELD_LEFT: f64 = 11.0;
const ROOM_WIDTH: f64 = 480.0;
const ROOM_HEIGHT: f64 = 270.0;
const START: (f64, f64) = (20.0, 68.0);
const SONIC_SPEED: f64 = 3.0;
const SONIC_SIDE_SPEED: f64 = 2.0;
/// alarm[0]: the spin recharges 3 seconds, and is first ready 10 steps (60 Hz) in; alarm[1]: it lasts 1 second.
const SPIN_RECHARGE_SECONDS: f64 = 3.0;
const FIRST_SPIN_TICKS: i32 = 10;
const SPIN_SECONDS: f64 = 1.0;
/// obj_minisnoc dead: it flies up and back, turning.
const DEATH_JUMP: f64 = -4.0;
const DEATH_DRIFT: f64 = 0.125;
const DEATH_GRAVITY: f64 = 0.24;
const DEATH_TURN: f64 = 25.0;
/// obj_minigame alarm[0]: 2 seconds after dying the game is put away.
const PUT_AWAY_SECONDS: f64 = 2.0;
/// obj_minigame alarm[1] and alarm[2]: the first grey rock's warning and the offer.
const WARNING_SECONDS: f64 = 2.0;
const OFFER_SECONDS: f64 = 2.0;
const OFFER_BLINK_MILLISECONDS: f64 = 480.0;
/// Rocks come in at x 496 between the field's edges; points for getting past one.
const ROCK_START_X: f64 = 496.0;
const ROCK_GONE_X: f64 = -60.0;
const ROCK_POINTS: u32 = 100;
/// The spawn interval: 1 to 3 (3 to 5 while Eggman is there) times (60 - elapsed)
/// steps; `elapsed` grows by 0.5 a rock up to 50, and speeds the border up too.
const ELAPSED_STEP: f64 = 0.5;
const ELAPSED_MAX: f64 = 50.0;
/// A spin through several grey rocks: 200 points each and a granny; one alone: 100.
const ROW_POINTS: u32 = 200;
const SPIN_POINTS: u32 = 100;
/// Eggman comes every 5000 points.
const BOSS_EVERY_POINTS: u32 = 5000;
const BOSS_HP: i32 = 5;
const BOSS_TOP: f64 = 34.0;
const BOSS_BOTTOM: f64 = 113.0;
const BOSS_SHOT_SPEED: f64 = 4.0;
const BOSS_SHOT_POINTS: u32 = 10;
const BORDER_Y: f64 = 22.0;
const BORDER_WIDTH: f64 = 512.0;
/// The HUD: score at the top right, the spin's charge at the left.
const SCORE_POSITION: (f64, f64) = (400.0, 34.0);
const CHARGE_Y: f64 = 110.0;

#[derive(Serialize, Deserialize, Default)]
struct Saved {
    hiscore: u32,
}

/// A thing of the game that moves by itself: GameMaker's hspeed and vspeed.
#[derive(Clone)]
struct Mover {
    x: f64,
    y: f64,
    hspeed: f64,
    vspeed: f64,
    angle: f64,
    frame: f64,
}

impl Mover {
    fn at(x: f64, y: f64) -> Mover {
        Mover { x, y, hspeed: 0.0, vspeed: 0.0, angle: 0.0, frame: 0.0 }
    }

    fn step(&mut self, sprites: &Sprites, sprite: SpriteId) {
        self.x += self.hspeed * step();
        self.y += self.vspeed * step();
        let meta = sprites.get(sprite);
        if meta.frame_count > 0 {
            self.frame = (self.frame + f64::from(meta.fps) / crate::core::config::ticks_per_second()) % meta.frame_count as f64;
        }
    }

    fn bbox(&self, sprites: &Sprites, sprite: SpriteId) -> Bbox {
        sprite_bbox(sprites.get(sprite), self.x, self.y, 1.0, 1.0, self.angle)
    }
}

struct Rock {
    body: Mover,
    /// obj_asteroid_breakable: the grey ones a spin breaks.
    breakable: bool,
}

/// obj_minisnoc_eggy: Eggman, who shoots at Sonic and is hit by the rocks' pieces.
struct Boss {
    body: Mover,
    hp: i32,
    /// Steps it flashes after a hit.
    hit: i32,
    cant_attack: i32,
    shooting: bool,
    change_in: i32,
    side_down: bool,
    moved: i32,
    dead: bool,
    yspd: f64,
}

/// What the game asks the waiting room to play: the room has the audio.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Cue {
    Play(crate::core::resources::SoundId),
    Music(crate::core::resources::SoundId),
    StopMusic,
}

pub struct Minigame {
    /// obj_minigame.show: the round is running, the offer may be taken.
    pub shown: bool,
    pub ingame: bool,
    score: u32,
    hiscore: u32,
    /// The score when Eggman last fell: the next one comes 5000 later.
    last: u32,
    elapsed: f64,
    interval: f64,
    border: f64,
    first_grey: bool,
    offer_ticks: i32,
    warning_ticks: i32,
    put_away_ticks: i32,
    sonic: Mover,
    sonic_dead: bool,
    sonic_xspd: f64,
    spin_ready_in: i32,
    spin_ticks: i32,
    /// Grey rocks broken in this spin.
    row: u32,
    ticks: u64,
    trail: Vec<(Mover, SpriteId, f64)>,
    rocks: Vec<Rock>,
    pieces: Vec<Mover>,
    boss: Option<Boss>,
    shots: Vec<Mover>,
    dust: Vec<(Mover, i32)>,
    grannies: Vec<Mover>,
    cues: Vec<Cue>,
}

impl Minigame {
    pub fn new() -> Minigame {
        let hiscore = std::fs::read_to_string(HISCORE_FILE).ok().and_then(|text| toml::from_str::<Saved>(&text).ok()).map_or(0, |saved| saved.hiscore);
        Minigame {
            shown: false,
            ingame: false,
            score: 0,
            hiscore,
            last: 0,
            elapsed: 0.0,
            interval: 0.0,
            border: 0.0,
            first_grey: true,
            offer_ticks: 0,
            warning_ticks: 0,
            put_away_ticks: 0,
            sonic: Mover::at(START.0, START.1),
            sonic_dead: false,
            sonic_xspd: 0.0,
            spin_ready_in: crate::core::config::original_ticks(FIRST_SPIN_TICKS),
            spin_ticks: 0,
            row: 0,
            ticks: 0,
            trail: Vec::new(),
            rocks: Vec::new(),
            pieces: Vec::new(),
            boss: None,
            shots: Vec::new(),
            dust: Vec::new(),
            grannies: Vec::new(),
            cues: Vec::new(),
        }
    }

    /// The sounds and music since the last call, for the waiting room to play.
    pub fn take_cues(&mut self) -> Vec<Cue> {
        std::mem::take(&mut self.cues)
    }

    /// net_state_pending: the round runs, the waiting room offers the game.
    pub fn show(&mut self) {
        self.shown = true;
        self.offer_ticks = ticks(OFFER_SECONDS);
    }

    fn spawn_interval(&self) -> f64 {
        let (low, high) = if self.boss.is_some() { (3.0, 5.0) } else { (1.0, 3.0) };
        gen_range(low, high) * (60.0 - self.elapsed) / step()
    }

    /// obj_minigame.activate
    fn start(&mut self) {
        self.boss = None;
        self.shots.clear();
        self.score = 0;
        self.last = 0;
        self.offer_ticks = ticks(OFFER_SECONDS);
        self.first_grey = true;
        self.border = 0.0;
        self.elapsed = 0.0;
        self.interval = self.spawn_interval();
        self.cues.push(Cue::Music(sound::MUS_MINIGAME));
        self.sonic = Mover::at(START.0, START.1);
        self.spin_ready_in = crate::core::config::original_ticks(FIRST_SPIN_TICKS);
        self.ingame = true;
    }

    /// obj_minigame.dead
    fn die(&mut self) {
        self.cues.push(Cue::Play(sound::SND_MINIDIE));
        self.cues.push(Cue::StopMusic);
        self.save_hiscore();
        self.first_grey = false;
        self.sonic.vspeed = DEATH_JUMP;
        self.sonic_dead = true;
        self.put_away_ticks = ticks(PUT_AWAY_SECONDS);
    }

    /// obj_minigame Other_5 and dead(): a better score is kept.
    pub fn save_hiscore(&mut self) {
        if self.score <= self.hiscore {
            return;
        }
        self.hiscore = self.score;
        let written = Path::new(HISCORE_FILE)
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|_| std::fs::write(HISCORE_FILE, toml::to_string(&Saved { hiscore: self.hiscore }).expect("a number always serializes")));
        if let Err(error) = written {
            eprintln!("failed to save {HISCORE_FILE}: {error}");
        }
    }

    /// obj_minigame Alarm_0: the game is put away and the waiting music comes back.
    fn put_away(&mut self) {
        self.sonic.hspeed = 0.0;
        self.sonic.vspeed = 0.0;
        self.sonic.angle = 0.0;
        self.sonic_dead = false;
        self.ingame = false;
        self.rocks.clear();
        self.cues.push(Cue::Music(sound::MUS_WAITING));
    }

    /// One step of every object of the game. `paused`: the chat or a vote holds obj_minigame.
    pub fn step(&mut self, input: &Input, keys: &KeyBindings, sprites: &Sprites, paused: bool) {
        self.ticks += 1;
        self.offer_ticks -= 1;
        self.warning_ticks -= 1;
        if self.put_away_ticks > 0 {
            self.put_away_ticks -= 1;
            if self.put_away_ticks == 0 {
                self.put_away();
            }
        }
        if !paused {
            self.step_game(input, keys);
        }
        self.step_sonic(input, keys, sprites);
        self.step_rocks(sprites);
        self.step_boss(sprites);
        self.step_small_things(sprites);
    }

    /// obj_minigame Step: the offer, the rocks coming in, the border moving.
    fn step_game(&mut self, input: &Input, keys: &KeyBindings) {
        if self.score.saturating_sub(self.last) >= BOSS_EVERY_POINTS && self.boss.is_none() {
            self.boss = Some(Boss::new());
        }
        if !self.ingame {
            if self.shown && input.pressed(keys.jump.0) {
                self.start();
            }
            return;
        }
        self.interval -= 1.0;
        if self.interval <= 0.0 {
            let y = gen_range(FIELD_TOP, FIELD_BOTTOM);
            let one_in_four = gen_range(0.0, 4.0) <= 1.0;
            // While Eggman is there most rocks are grey: their pieces hit him.
            let breakable = if self.boss.is_some() { !one_in_four } else { one_in_four };
            self.rocks.push(Rock::new(ROCK_START_X, y, breakable));
            if breakable && self.first_grey {
                self.warning_ticks = ticks(WARNING_SECONDS);
                self.first_grey = false;
            }
            self.interval = self.spawn_interval();
            if self.elapsed < ELAPSED_MAX {
                self.elapsed += ELAPSED_STEP;
            }
        }
        if self.sonic_dead {
            return;
        }
        self.border -= (1.0 + self.elapsed / 10.0) * step();
        if self.border <= -BORDER_WIDTH {
            self.border += BORDER_WIDTH;
        }
    }

    /// obj_minisnoc Step and alarms.
    fn step_sonic(&mut self, input: &Input, keys: &KeyBindings, sprites: &Sprites) {
        if !self.ingame {
            return;
        }
        if self.sonic_dead {
            self.sonic.hspeed -= DEATH_DRIFT * step();
            self.sonic.vspeed += DEATH_GRAVITY * step();
            self.sonic.angle += DEATH_TURN * step();
            self.sonic.step(sprites, sprite::SPR_MINISNOC_HIT);
            return;
        }
        if self.spin_ready_in > 0 {
            self.spin_ready_in -= 1;
        }
        if self.spin_ticks > 0 {
            self.spin_ticks -= 1;
            if self.spin_ticks == 0 {
                self.spin_over();
            }
        }
        if input.held(keys.up.0) {
            self.sonic.y -= SONIC_SPEED * step();
        } else if input.held(keys.down.0) {
            self.sonic.y += SONIC_SPEED * step();
        }
        self.sonic_xspd = if input.held(keys.left.0) {
            -SONIC_SIDE_SPEED
        } else if input.held(keys.right.0) {
            SONIC_SIDE_SPEED
        } else {
            0.0
        };
        self.sonic.x += self.sonic_xspd * step();
        if input.pressed(keys.jump.0) && self.spin_ready_in <= 0 {
            self.cues.push(Cue::Play(sound::SND_MINISPIN));
            self.spin_ticks = ticks(SPIN_SECONDS);
            self.spin_ready_in = ticks(SPIN_RECHARGE_SECONDS);
        }
        self.sonic.y = self.sonic.y.clamp(FIELD_TOP, FIELD_BOTTOM);
        self.sonic.x = self.sonic.x.clamp(FIELD_LEFT, ROOM_WIDTH - FIELD_LEFT);
        let look = self.sonic_sprite();
        self.sonic.step(sprites, look);
        // A fading copy left behind every third step.
        if self.ticks % crate::core::config::original_ticks(3).max(1) as u64 == 0 {
            self.trail.push((self.sonic.clone(), look, 1.0));
        }
    }

    /// obj_minisnoc Alarm_1: the spin is over; a row of broken grey rocks pays.
    fn spin_over(&mut self) {
        if self.row > 1 {
            self.score += self.row * ROW_POINTS;
            self.cues.push(Cue::Play(sound::SND_MINIEXTRA));
            self.grannies.push(Mover::at(self.sonic.x, self.sonic.y));
        } else {
            self.score += SPIN_POINTS;
        }
        self.row = 0;
    }

    fn sonic_sprite(&self) -> SpriteId {
        if self.sonic_dead {
            sprite::SPR_MINISNOC_HIT
        } else if self.spin_ticks > 0 {
            sprite::SPR_MINISNOC_SPIN
        } else {
            sprite::SPR_MINISNOC
        }
    }

    /// obj_asteroid and obj_asteroid_breakable Step.
    fn step_rocks(&mut self, sprites: &Sprites) {
        let sonic_box = self.sonic.bbox(sprites, self.sonic_sprite());
        let mut died = false;
        let mut index = 0;
        while index < self.rocks.len() {
            let rock = &mut self.rocks[index];
            let look = rock.sprite();
            rock.body.angle += rock.body.hspeed / 2.0 * step();
            rock.body.step(sprites, look);
            if self.sonic_dead || !self.ingame {
                index += 1;
                continue;
            }
            if rock.body.bbox(sprites, look).overlaps(&sonic_box) {
                if rock.breakable && self.spin_ticks > 0 {
                    for _ in 0..gen_range(4, 7) {
                        let mut piece = Mover::at(rock.body.x + gen_range(-4.0, 4.0), rock.body.y + gen_range(-4.0, 4.0));
                        piece.hspeed = self.sonic_xspd * 1.2 + gen_range(-2.0, 2.0);
                        piece.vspeed = gen_range(-4.0, -2.0);
                        self.pieces.push(piece);
                    }
                    self.cues.push(Cue::Play(sound::SND_MINIDESTROY));
                    self.row += 1;
                    self.rocks.remove(index);
                    continue;
                }
                died = true;
            }
            if rock.body.x <= ROCK_GONE_X {
                if !rock.breakable {
                    self.score += ROCK_POINTS;
                }
                self.rocks.remove(index);
                continue;
            }
            index += 1;
        }
        if died {
            self.die();
        }
    }

    /// obj_minisnoc_eggy and obj_minisnoc_eggy2 Step.
    fn step_boss(&mut self, sprites: &Sprites) {
        let sonic = (self.sonic.x, self.sonic.y);
        let sonic_box = self.sonic.bbox(sprites, self.sonic_sprite());
        let mut died = false;
        if let Some(boss) = self.boss.as_mut() {
            let outcome = boss.step(sprites, sonic, &sonic_box, self.ingame && !self.sonic_dead, &mut self.pieces, &mut self.shots, &mut self.dust, &mut self.cues);
            match outcome {
                BossStep::Falling => self.last = self.score,
                BossStep::Gone => {
                    // obj_minisnoc_eggy CleanUp
                    self.last = self.score;
                    self.boss = None;
                    self.elapsed /= 2.0;
                }
                BossStep::TouchedSonic => died = true,
                BossStep::Fine => {}
            }
        }
        let ingame = self.ingame;
        let mut passed = 0;
        self.shots.retain_mut(|shot| {
            shot.x -= BOSS_SHOT_SPEED * step();
            if ingame && shot.bbox(sprites, sprite::SPR_MINISNOC_MCBIGTASTY).overlaps(&sonic_box) {
                died = true;
            }
            if shot.x <= 0.0 {
                passed += 1;
                return false;
            }
            true
        });
        self.score += passed * BOSS_SHOT_POINTS;
        if died && !self.sonic_dead && self.ingame {
            self.die();
        }
    }

    /// The pieces, the trail, the dust and the grannies: they only move and go.
    fn step_small_things(&mut self, sprites: &Sprites) {
        for piece in &mut self.pieces {
            piece.vspeed += 0.128 * step();
            piece.step(sprites, sprite::SPR_SOTA3);
        }
        self.pieces.retain(|piece| piece.y < ROOM_HEIGHT + 20.0);
        for (copy, _, alpha) in &mut self.trail {
            *alpha -= 0.128 * step();
            copy.x -= step();
        }
        self.trail.retain(|(_, _, alpha)| *alpha > 0.0);
        for (dust, left) in &mut self.dust {
            dust.step(sprites, sprite::SPR_DUST);
            *left -= 1;
        }
        self.dust.retain(|(_, left)| *left > 0);
        for granny in &mut self.grannies {
            granny.y -= 0.8 * step();
        }
        self.grannies.retain(|granny| granny.y > -ROOM_HEIGHT);
    }

    /// obj_minigame Draw and the objects of the Minigame layers, while the game is on.
    pub fn draw(&self, canvas: &mut Canvas) {
        if !self.ingame {
            return;
        }
        canvas.draw_sprite(sprite::SPR_MINI_BORDER, 0.0, self.border, BORDER_Y);
        canvas.draw_sprite(sprite::SPR_MINI_BORDER, 0.0, self.border + BORDER_WIDTH, BORDER_Y);
        let draw = |canvas: &mut Canvas, body: &Mover, look: SpriteId, alpha: f64| {
            canvas.draw_sprite_ext(look, body.frame, body.x, body.y, 1.0, 1.0, body.angle, C_WHITE, alpha);
        };
        for (copy, look, alpha) in &self.trail {
            draw(canvas, copy, *look, *alpha);
        }
        draw(canvas, &self.sonic, self.sonic_sprite(), 1.0);
        for rock in &self.rocks {
            draw(canvas, &rock.body, rock.sprite(), 1.0);
        }
        for piece in &self.pieces {
            draw(canvas, piece, sprite::SPR_SOTA3, 1.0);
        }
        for shot in &self.shots {
            draw(canvas, shot, sprite::SPR_MINISNOC_MCBIGTASTY, 1.0);
        }
        if let Some(boss) = &self.boss {
            canvas.draw_sprite_ext(sprite::SPR_MINISNOC_GRAVEYARD, f64::from(boss.hit.max(0)), boss.body.x, boss.body.y, 1.0, 1.0, boss.body.angle, C_WHITE, 1.0);
        }
        for (dust, _) in &self.dust {
            draw(canvas, dust, sprite::SPR_DUST, 1.0);
        }
        for granny in &self.grannies {
            draw(canvas, granny, sprite::SPR_MINISNOC_EXTRA, 1.0);
        }
    }

    /// obj_minigame Draw_64: the offer, the score, the spin's charge, the first grey rock's warning.
    pub fn draw_gui(&self, canvas: &mut Canvas, current_time_ms: f64) {
        if !self.shown {
            return;
        }
        if !self.ingame && self.offer_ticks > 0 && current_time_ms % OFFER_BLINK_MILLISECONDS > OFFER_BLINK_MILLISECONDS / 2.0 {
            canvas.draw_sprite(sprite::SPR_MINISNOC_TEXT, 0.0, ROOM_WIDTH / 2.0, FIELD_TOP + 81.0 / 2.0 + 28.0);
        }
        if self.ingame {
            let text = format!("score {:05}\n\\hi~    {:05}", self.score, self.hiscore);
            draw_text(canvas, SCORE_POSITION.0, SCORE_POSITION.1, &text, C_WHITE, 1.0);
            let charge = (1.0 - self.spin_ready_in.max(0) as f64 / ticks(SPIN_RECHARGE_SECONDS) as f64).clamp(0.0, 1.0);
            let colour = if charge >= 1.0 { C_WHITE } else { 0x0000FF };
            canvas.draw_sprite_ext(sprite::SPR_MINISNOC_CHARG, 0.0, 2.0 + charge * 3.0, CHARGE_Y, 1.0, 1.0, 0.0, colour, charge);
        }
        if self.warning_ticks > 0 {
            canvas.draw_sprite(sprite::SPR_MINISNOC_TEXT, 1.0, ROOM_WIDTH / 2.0, FIELD_TOP + 4.0);
        }
    }
}

impl Rock {
    /// obj_asteroid Create: it comes in at 2 to 4 px a step.
    fn new(x: f64, y: f64, breakable: bool) -> Rock {
        let mut body = Mover::at(x, y);
        body.hspeed = -gen_range(2.0, 4.0);
        Rock { body, breakable }
    }

    fn sprite(&self) -> SpriteId {
        if self.breakable { sprite::SPR_SOTA } else { sprite::SPR_SOTA2 }
    }
}

enum BossStep {
    Fine,
    Falling,
    Gone,
    TouchedSonic,
}

impl Boss {
    /// obj_minisnoc_eggy Create.
    fn new() -> Boss {
        Boss {
            body: Mover::at(ROOM_WIDTH + 20.0, f64::from(gen_range(54, 96))),
            hp: BOSS_HP,
            hit: 0,
            cant_attack: ticks(2.0),
            shooting: false,
            change_in: ticks(7.0),
            side_down: gen_range(0, 2) == 1,
            moved: 0,
            dead: false,
            yspd: 0.0,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn step(&mut self, sprites: &Sprites, sonic: (f64, f64), sonic_box: &Bbox, playing: bool, pieces: &mut Vec<Mover>, shots: &mut Vec<Mover>, dust: &mut Vec<(Mover, i32)>, cues: &mut Vec<Cue>) -> BossStep {
        if self.dead {
            if self.yspd < 6.0 {
                self.yspd += 0.26 * step();
            }
            self.body.angle += step();
            self.body.x -= 2.0 * step();
            self.body.y += self.yspd * step();
            self.cant_attack += 1;
            if self.cant_attack % crate::core::config::original_ticks(3).max(1) == 0 {
                let mut puff = Mover::at(self.body.x + gen_range(-10.0, 10.0), self.body.y + gen_range(-10.0, 10.0));
                puff.vspeed = gen_range(-4.0, -1.0);
                dust.push((puff, ticks(f64::from(gen_range(1, 3)))));
            }
            return if self.body.y >= ROOM_HEIGHT + 10.0 { BossStep::Gone } else { BossStep::Falling };
        }
        if !playing {
            return BossStep::Fine;
        }
        // Alarm_0: every 7 seconds it may start or stop shooting at Sonic.
        self.change_in -= 1;
        if self.change_in <= 0 {
            self.shooting = gen_range(0.0, 15.0) <= 10.0;
            self.change_in = ticks(7.0);
        }
        let own_box = self.body.bbox(sprites, sprite::SPR_MINISNOC_GRAVEYARD);
        if own_box.overlaps(sonic_box) {
            return BossStep::TouchedSonic;
        }
        self.hit -= 1;
        if self.hit >= 0 {
            return BossStep::Fine;
        }
        if let Some(piece) = pieces.iter().position(|piece| piece.bbox(sprites, sprite::SPR_SOTA3).overlaps(&own_box)) {
            pieces.remove(piece);
            cues.push(Cue::Play(sound::SND_MINIDESTROY));
            self.hit = ticks(0.5);
            self.cant_attack = ticks(1.5);
            self.hp -= 1;
            if self.hp <= 0 {
                self.dead = true;
                self.yspd = f64::from(gen_range(-5, -1));
            }
            return BossStep::Fine;
        }
        let width = f64::from(sprites.get(sprite::SPR_MINISNOC_GRAVEYARD).width);
        if self.body.x > ROOM_WIDTH - width * 1.5 {
            self.body.x -= step();
            return BossStep::Fine;
        }
        if self.cant_attack > 0 {
            self.cant_attack -= 1;
        }
        let (moves, wait) = if self.shooting { (2.0, (30, 70)) } else { (1.0, (40, 50)) };
        for _ in 0..moves as i32 {
            if self.shooting {
                self.side_down = self.body.y <= sonic.1;
            }
            if self.side_down {
                if self.body.y < BOSS_BOTTOM {
                    self.body.y += step();
                } else if !self.shooting {
                    self.side_down = false;
                }
            } else if self.body.y > BOSS_TOP {
                self.body.y -= step();
            } else if !self.shooting {
                self.side_down = true;
            }
        }
        // cantattack <= 0 && --moved <= 0: the wait only runs down while it may attack.
        if self.cant_attack <= 0 {
            self.moved -= 1;
            if self.moved <= 0 {
                cues.push(Cue::Play(sound::SND_MINIBAL));
                shots.push(Mover::at(self.body.x, self.body.y));
                self.moved = crate::core::config::original_ticks(gen_range(wait.0, wait.1));
            }
        }
        BossStep::Fine
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn sprites() -> Arc<Sprites> {
        let textures = Path::new(env!("CARGO_MANIFEST_DIR")).join("Resources/Textures");
        Arc::new(Sprites::index(&textures).unwrap())
    }

    fn playing() -> Minigame {
        let mut game = Minigame::new();
        game.show();
        game.start();
        game.take_cues();
        game.interval = f64::MAX;
        game
    }

    fn step(game: &mut Minigame, sprites: &Sprites) {
        game.step(&Input::default(), &crate::client::options::Options::default().keys, sprites, false);
    }

    #[test]
    fn a_rock_that_meets_sonic_ends_the_game() {
        let sprites = sprites();
        let mut game = playing();
        game.rocks.push(Rock::new(game.sonic.x, game.sonic.y, false));
        step(&mut game, &sprites);
        assert!(game.sonic_dead);
        assert!(game.take_cues().contains(&Cue::Play(sound::SND_MINIDIE)));
        for _ in 0..ticks(PUT_AWAY_SECONDS) {
            step(&mut game, &sprites);
        }
        assert!(!game.ingame && game.rocks.is_empty(), "put away two seconds later");
        assert!(game.take_cues().contains(&Cue::Music(sound::MUS_WAITING)));
    }

    #[test]
    fn a_spin_breaks_grey_rocks_and_pays_for_the_row() {
        let sprites = sprites();
        let mut game = playing();
        game.spin_ticks = ticks(SPIN_SECONDS);
        for _ in 0..2 {
            game.rocks.push(Rock::new(game.sonic.x, game.sonic.y, true));
        }
        step(&mut game, &sprites);
        assert!(!game.sonic_dead && game.rocks.is_empty(), "both broken");
        assert_eq!(game.row, 2);
        assert!(game.pieces.len() >= 8, "four to six pieces a rock");
        game.spin_ticks = 1;
        step(&mut game, &sprites);
        assert_eq!(game.score, 2 * ROW_POINTS, "a row of two");
        assert_eq!(game.grannies.len(), 1);
    }

    #[test]
    fn a_rock_that_gets_past_is_worth_points() {
        let sprites = sprites();
        let mut game = playing();
        game.rocks.push(Rock::new(ROCK_GONE_X + 1.0, FIELD_BOTTOM, false));
        for _ in 0..3 {
            step(&mut game, &sprites);
        }
        assert_eq!((game.score, game.rocks.len()), (ROCK_POINTS, 0));
    }
}
