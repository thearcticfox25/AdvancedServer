//! The achievements (obj_achivements): what each one asks for, the player's record
//! of them (PersistentData/Game/Achievements.toml, the original's 3.bin), the counters
//! a round keeps for them, and the box that slides in when one is earned.
//! The original had every line drawn into one tall sprite; here the rows share one
//! frame (Textures/Main Menu/achivements.png) and the lines are text.
//!
//! Only rounds on a server count, as the original had no other: not Singleplayer, and
//! not Fart Zone, which the original left out of every one.

use crate::client::canvas::Canvas;
use crate::core::config::ticks;
use crate::core::events::SimEvent;
use crate::core::game::RoundEnding;
use crate::core::player::{Character, ExeCharacter};
use crate::core::resources::names::{sound, sprite};
use crate::core::rooms::ids::RoomId;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const ACHIEVEMENT_COUNT: usize = 50;
const ACHIEVEMENTS_FILE: &str = "PersistentData/Game/Achievements.toml";

/// The lines of spr_achivements, in its order. A row the original wrapped keeps its break.
pub const ACHIEVEMENTS: [&str; 50] = [
    "win 10 rounds",
    "win 50 rounds",
    "win 100 rounds",
    "revive 1 player in the round",
    "revive 2 players in the round",
    "revive 2 players in the round at once",
    "revive 3 players in the round",
    "kill a survivor after getting demonized",
    "kill a survivor 5 seconds after getting demonized",
    "stun exe 5 times in the round",
    "escape on last 5 seconds having 1 hp",
    "win the round without getting hurt",
    "finish survivors 3 times as exe",
    "finish survivors 10 times as exe",
    "finish survivors 50 times as exe",
    "finish survivors 100 times as exe",
    "restore your health 3 times in a round",
    "heal 2 different players in a round",
    "survive as every character",
    "win as every exe",
    "escape along with 5 survivors",
    "collect 25 rings in a round",
    "shoot exe down with full charged shot as tails",
    "shoot a survivor down with full charged shot as\ntails.exe",
    "stun two players at once with knuckles' uppercut",
    "hit two players at once with knuckles.exe's uppercut",
    "track down exe 4 times in a round as eggman",
    "track down survivors 3 times in a round as\neggman.exe",
    "win the round without stunning anyone as amy",
    "hit survivors 5 times in a round as amy.exe",
    "heal 3 teammates in a round as cream",
    "kill 2 survivors in a round as cream.exe",
    "block the attack on 1hp with the shield 2 times in a\nround as sally",
    "damage survivors with shield shards 2 times in a\nround as sally.exe",
    "kill a survivor right after exiting invisibility as\nsonic.exe",
    "dash through 2 survivors at once as chaos",
    "hit 2 survivors at once with shockwave as exetior",
    "kill a survivor 1 second after teleporting as exeller",
    "survive as tails on hide and seek",
    "survive as knuckles in you can't run",
    "survive as eggman in ...",
    "survive as amy in not perfect",
    "survive as cream in kind and fair",
    "survive as sally in act 9",
    "survive as tails in torture cave against chaos",
    "survive as cream in nasty paradise against exetior",
    "survive as sally in limp city against exeller",
    "survive in priceless freedom without getting black\nrings",
    "survive in ravine mist after collecting 6 shards on\nthe last minute",
    "survive on each map",
];

/// The achievements by the number the list and the original give them.
mod id {
    pub const WIN_10: usize = 0;
    pub const WIN_50: usize = 1;
    pub const WIN_100: usize = 2;
    pub const REVIVE_1: usize = 3;
    pub const REVIVE_2: usize = 4;
    pub const REVIVE_2_AT_ONCE: usize = 5;
    pub const REVIVE_3: usize = 6;
    pub const DEMON_KILL: usize = 7;
    pub const DEMON_KILL_RIGHT_AWAY: usize = 8;
    pub const STUN_EXE_5: usize = 9;
    pub const LAST_SECOND_ESCAPE: usize = 10;
    pub const UNHURT_WIN: usize = 11;
    pub const EXE_KILLS: [(u32, usize); 4] = [(3, 12), (10, 13), (50, 14), (100, 15)];
    pub const RESTORE_HEALTH_3: usize = 16;
    pub const HEAL_2: usize = 17;
    pub const EVERY_SURVIVOR: usize = 18;
    pub const EVERY_EXE: usize = 19;
    pub const ESCAPE_WITH_5: usize = 20;
    pub const RINGS_25: usize = 21;
    pub const FULL_SHOT_AT_EXE: usize = 22;
    pub const FULL_DEMON_SHOT: usize = 23;
    pub const UPPERCUT_2: usize = 24;
    pub const DEMON_UPPERCUT_2: usize = 25;
    pub const TRACK_EXE_4: usize = 26;
    pub const DEMON_TRACK_3: usize = 27;
    pub const AMY_NO_STUNS: usize = 28;
    pub const DEMON_AMY_HITS_5: usize = 29;
    pub const CREAM_HEALS_3: usize = 30;
    pub const DEMON_CREAM_KILLS_2: usize = 31;
    pub const SALLY_LAST_HIT_BLOCKS_2: usize = 32;
    pub const DEMON_SALLY_SHARDS_2: usize = 33;
    pub const KILL_AFTER_INVISIBILITY: usize = 34;
    pub const CHAOS_DASH_2: usize = 35;
    pub const SHOCKWAVE_2: usize = 36;
    pub const KILL_AFTER_TELEPORT: usize = 37;
    pub const PRICELESS_FREEDOM_NO_BLACK_RINGS: usize = 47;
    pub const LATE_SHARDS_6: usize = 48;
    pub const EVERY_MAP: usize = 49;
}

/// survWins + exeWins for WIN_10, WIN_50 and WIN_100.
const WINS: [(u32, usize); 3] = [(10, id::WIN_10), (50, id::WIN_50), (100, id::WIN_100)];
/// The original asked for 18 different maps when survivors won or time ran out, 20
/// when EXE killed everyone; 18, the number of the two, counts for all three here.
const EVERY_MAP_COUNT: usize = 18;
/// Survived on a map as a character (and against an EXE): the achievement for it.
const SURVIVED_ON: [(RoomId, Character, Option<ExeCharacter>, usize); 10] = [
    (RoomId::Hideandseek2, Character::Tails, None, 38),
    (RoomId::Angelisland, Character::Tails, None, 38),
    (RoomId::Youcantrun, Character::Knux, None, 39),
    (RoomId::Dotdotdot, Character::Eggman, None, 40),
    (RoomId::Notperfect, Character::Amy, None, 41),
    (RoomId::Kindandfair, Character::Cream, None, 42),
    (RoomId::Act9, Character::Sally, None, 43),
    (RoomId::Torturecave, Character::Tails, Some(ExeCharacter::Chaos), 44),
    (RoomId::Nastyparadise, Character::Cream, Some(ExeCharacter::Exetior), 45),
    (RoomId::Limpcity, Character::Sally, Some(ExeCharacter::Exeller), 46),
];

/// The windows the original timed with the alarms of obj_achivements: how long after
/// something another thing still counts together with it.
const REVIVAL_WINDOW_SECONDS: f64 = 1.0;
const DEMONIZED_WINDOW_SECONDS: f64 = 5.0;
const TELEPORT_WINDOW_SECONDS: f64 = 1.0;
const SHOCKWAVE_WINDOW_SECONDS: f64 = 1.0;
const CHAOS_DASH_WINDOW_SECONDS: f64 = 1.0;
/// alarm[6] = 80 steps.
const INVISIBILITY_WINDOW_SECONDS: f64 = 80.0 / 60.0;
const UPPERCUT_WINDOW_SECONDS: f64 = 1.0;
/// The last seconds of a round a survivor with one hit left may escape in, and the
/// last minute of shards for LATE_SHARDS_6.
const LAST_SECONDS: u32 = 5;
const LAST_MINUTE_SECONDS: u32 = 60;
/// CLIENT_MERCOIN_BONUS numbers (core/contact.rs, shield_shards.rs).
const BONUS_STUNNED_EXE: u8 = 1;
const BONUS_AMY_HIT: u8 = 3;
const BONUS_DEMON_KILL: u8 = 4;
const BONUS_SHIELD_SHARDS: u8 = 5;
const BONUS_SHOCKWAVE: u8 = 7;
const BONUS_DEMON_UPPERCUT: u8 = 8;
const BONUS_UPPERCUT: u8 = 9;
const BONUS_CHAOS_SECOND_ATTACK: u8 = 10;
const BONUS_AMY_STUN: u8 = 11;
/// A survivor's shot at EXE: its stun of 6 to 10 seconds is a full charge; up to 10
/// it counts for STUN_EXE_5 too.
const FULL_SHOT_STUN: std::ops::RangeInclusive<i32> = 6..=10;
const COUNTED_SHOT_STUN: i32 = 10;
/// obj_achivements Draw_64: the box slides 6 px a step out to 194 and stays 4 seconds.
const BOX_SLIDE: f64 = 6.0;
const BOX_OUT_X: f64 = 194.0;
const BOX_TOP: f64 = 4.0;
const BOX_SHOWN_SECONDS: f64 = 4.0;

/// What the player earned over all rounds (3.bin of the original).
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Record {
    /// The numbers of the earned achievements.
    unlocked: Vec<usize>,
    survivor_wins: u32,
    exe_wins: u32,
    exe_kills: u32,
    /// The survivors survived as and the EXE won as, by name.
    survived_as: Vec<String>,
    won_as_exe: Vec<String>,
    /// The maps won on (room file names).
    maps: Vec<String>,
}

/// The counters a round keeps (obj_achivements Other_4 sets them back), and the
/// windows in ticks, each clearing its counter when it runs out.
#[derive(Default)]
struct Round {
    revivals: u32,
    revival_window: i32,
    demonized_window: i32,
    stunned_exe: u32,
    was_hurt: bool,
    health_restored: u32,
    heals: u32,
    last_healed: Option<usize>,
    others_escaped: u32,
    demon_kills: u32,
    exe_tracked: u32,
    survivors_tracked: u32,
    amy_stuns: u32,
    demon_amy_hits: u32,
    last_hit_blocks: u32,
    shard_hits: u32,
    black_rings: u32,
    late_shards: u32,
    teleport_window: i32,
    shockwave_hits: u32,
    shockwave_window: i32,
    chaos_dashes: u32,
    chaos_dash_window: i32,
    invisibility_window: i32,
    uppercuts: u32,
    uppercut_window: i32,
}

/// What the achievements need to know about this player and the round when something happens.
pub struct Situation {
    pub room: RoomId,
    pub character: Character,
    pub exe_character: ExeCharacter,
    pub demonized: bool,
    pub hp: i32,
    /// The round clock.
    pub seconds_left: u32,
    /// Tails.demonized_damage at its strongest: a full charged demon shot.
    pub full_demon_shot: i32,
}

pub struct Achievements {
    /// Where the record is kept (ACHIEVEMENTS_FILE).
    path: PathBuf,
    record: Record,
    round: Round,
    /// Whether this round counts: on a server, and not Fart Zone.
    counting: bool,
    box_x: f64,
    box_ticks: i32,
    /// Earned since the game last asked, for snd_achivement (take_new_unlock).
    new_unlock: bool,
}

impl Achievements {
    /// The saved record, or a new one when there is none (or it is broken, which is said).
    pub fn load() -> Achievements {
        Achievements::load_from(Path::new(ACHIEVEMENTS_FILE))
    }

    fn load_from(path: &Path) -> Achievements {
        let record = match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).unwrap_or_else(|error| {
                eprintln!("{} is broken, starting a new one: {error}", path.display());
                Record::default()
            }),
            Err(_) => Record::default(),
        };
        Achievements { path: path.to_path_buf(), record, round: Round::default(), counting: false, box_x: 0.0, box_ticks: 0, new_unlock: false }
    }

    fn save(&self) {
        let written = self
            .path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|_| std::fs::write(&self.path, toml::to_string(&self.record).expect("the record always serializes")));
        if let Err(error) = written {
            eprintln!("failed to save {}: {error}", self.path.display());
        }
    }

    /// Whether an achievement was earned since the last call: the game plays snd_achivement.
    pub fn take_new_unlock(&mut self) -> bool {
        std::mem::take(&mut self.new_unlock)
    }

    pub fn is_unlocked(&self, index: usize) -> bool {
        self.record.unlocked.contains(&index)
    }

    /// achivements[index] = true; show(); save(): once, in a round that counts.
    fn unlock(&mut self, index: usize) {
        if !self.counting || self.is_unlocked(index) {
            return;
        }
        self.record.unlocked.push(index);
        self.record.unlocked.sort_unstable();
        self.save();
        self.new_unlock = true;
        self.box_ticks = ticks(BOX_SHOWN_SECONDS);
    }

    fn unlock_when(&mut self, reached: bool, index: usize) {
        if reached {
            self.unlock(index);
        }
    }

    /// obj_achivements Other_4: a new level starts the round's counters over.
    pub fn start_round(&mut self, room: RoomId, online: bool) {
        self.round = Round::default();
        self.counting = online && room != RoomId::Fartzone;
    }

    /// The alarms of obj_achivements: each window running out clears what it kept.
    pub fn tick(&mut self) {
        let round = &mut self.round;
        let run_out = |window: &mut i32| {
            *window -= 1;
            *window == 0
        };
        run_out(&mut round.revival_window);
        run_out(&mut round.demonized_window);
        run_out(&mut round.teleport_window);
        run_out(&mut round.invisibility_window);
        if run_out(&mut round.shockwave_window) {
            round.shockwave_hits = 0;
        }
        if run_out(&mut round.chaos_dash_window) {
            round.chaos_dashes = 0;
        }
        if run_out(&mut round.uppercut_window) {
            round.uppercuts = 0;
        }
        // obj_achivements Draw_64: the box slides in, stays, and slides back.
        self.box_ticks -= 1;
        if self.box_ticks > 0 {
            self.box_x = (self.box_x + BOX_SLIDE * crate::core::config::step()).min(BOX_OUT_X);
        } else {
            self.box_x = (self.box_x - BOX_SLIDE * crate::core::config::step()).max(0.0);
        }
    }

    /// Over whatever screen is on, while the box is out.
    pub fn draw(&self, canvas: &mut Canvas) {
        if self.box_x > 0.0 {
            canvas.draw_sprite(sprite::SPR_ACHIVEMENTBOX, 0.0, self.box_x, BOX_TOP);
        }
    }

    /// The round's events. `own` says whether a player index is this player; `about_own`
    /// whether an event with no index (a shield's block) happened to this player.
    pub fn on_event(&mut self, event: &SimEvent, own: impl Fn(usize) -> bool, about_own: bool, is_exe: impl Fn(usize) -> bool, now: &Situation) {
        match *event {
            SimEvent::KilledByExe { killer, .. } if own(killer) => {
                self.record.exe_kills += 1;
                self.save();
                for (kills, index) in id::EXE_KILLS {
                    self.unlock_when(self.record.exe_kills >= kills, index);
                }
                self.unlock_when(self.round.teleport_window > 0, id::KILL_AFTER_TELEPORT);
                self.unlock_when(self.round.invisibility_window > 0, id::KILL_AFTER_INVISIBILITY);
            }
            SimEvent::MercoinBonus { player, bonus } if own(player) => self.on_bonus(bonus, now),
            SimEvent::ShotHit { shooter, damage, on_exe } if own(shooter) => {
                if on_exe {
                    self.unlock_when(FULL_SHOT_STUN.contains(&damage), id::FULL_SHOT_AT_EXE);
                    if damage <= COUNTED_SHOT_STUN {
                        self.stunned_exe();
                    }
                } else {
                    self.unlock_when(damage >= now.full_demon_shot, id::FULL_DEMON_SHOT);
                }
            }
            SimEvent::TrackerCaught { eggman, victim } if own(eggman) && now.character == Character::Eggman => {
                if is_exe(victim) {
                    self.round.exe_tracked += 1;
                } else {
                    self.round.survivors_tracked += 1;
                }
                self.unlock_when(self.round.exe_tracked >= 4, id::TRACK_EXE_4);
                self.unlock_when(self.round.survivors_tracked >= 3, id::DEMON_TRACK_3);
            }
            SimEvent::TeammateHealed { healer, target } if own(healer) && self.round.last_healed != Some(target) => {
                self.round.heals += 1;
                self.round.last_healed = Some(target);
                self.unlock_when(self.round.heals >= 2, id::HEAL_2);
                self.unlock_when(self.round.heals >= 3 && now.character == Character::Cream, id::CREAM_HEALS_3);
            }
            SimEvent::BlackRingTaken { player } if own(player) => self.round.black_rings += 1,
            SimEvent::ShardFound { player } if own(player) && now.seconds_left < LAST_MINUTE_SECONDS => self.round.late_shards += 1,
            SimEvent::ShieldBlocked { on_last_hit: true } if about_own && now.character == Character::Sally => {
                self.round.last_hit_blocks += 1;
                self.unlock_when(self.round.last_hit_blocks >= 2, id::SALLY_LAST_HIT_BLOCKS_2);
            }
            _ => {}
        }
    }

    /// This player's own step, as its client predicts it: the windows it opens.
    pub fn on_own_event(&mut self, event: &SimEvent) {
        match *event {
            SimEvent::TeleportToClone { .. } => self.round.teleport_window = ticks(TELEPORT_WINDOW_SECONDS),
            SimEvent::ExeVanished { .. } | SimEvent::ExeAppeared { .. } => self.round.invisibility_window = ticks(INVISIBILITY_WINDOW_SECONDS),
            SimEvent::SpawnStompWaves { .. } => self.round.shockwave_window = ticks(SHOCKWAVE_WINDOW_SECONDS),
            SimEvent::Sound { sound: sound::SND_CHAOS_DASH, .. } => self.round.chaos_dash_window = ticks(CHAOS_DASH_WINDOW_SECONDS),
            SimEvent::RingsHealed => {
                self.round.health_restored += 1;
                self.unlock_when(self.round.health_restored >= 3, id::RESTORE_HEALTH_3);
            }
            _ => {}
        }
    }

    /// CLIENT_MERCOIN_BONUS for this player.
    fn on_bonus(&mut self, bonus: u8, now: &Situation) {
        let round = &mut self.round;
        match bonus {
            BONUS_STUNNED_EXE => self.stunned_exe(),
            BONUS_AMY_HIT => {
                round.demon_amy_hits += 1;
                let reached = round.demon_amy_hits >= 5;
                self.unlock_when(reached, id::DEMON_AMY_HITS_5);
            }
            BONUS_DEMON_KILL => {
                round.demon_kills += 1;
                let (right_away, cream) = (round.demonized_window > 0, round.demon_kills >= 2 && now.character == Character::Cream);
                self.unlock_when(right_away, id::DEMON_KILL_RIGHT_AWAY);
                self.unlock_when(cream, id::DEMON_CREAM_KILLS_2);
                self.unlock(id::DEMON_KILL);
            }
            BONUS_SHIELD_SHARDS => {
                round.shard_hits += 1;
                let reached = round.shard_hits >= 2;
                self.unlock_when(reached, id::DEMON_SALLY_SHARDS_2);
            }
            BONUS_SHOCKWAVE => {
                round.shockwave_hits += 1;
                let reached = round.shockwave_hits >= 2;
                self.unlock_when(reached, id::SHOCKWAVE_2);
            }
            BONUS_DEMON_UPPERCUT | BONUS_UPPERCUT => {
                round.uppercut_window = ticks(UPPERCUT_WINDOW_SECONDS);
                round.uppercuts += 1;
                let index = if bonus == BONUS_DEMON_UPPERCUT { id::DEMON_UPPERCUT_2 } else { id::UPPERCUT_2 };
                let reached = round.uppercuts >= 2;
                self.unlock_when(reached, index);
            }
            BONUS_CHAOS_SECOND_ATTACK => {
                round.chaos_dashes += 1;
                let reached = round.chaos_dashes >= 2;
                self.unlock_when(reached, id::CHAOS_DASH_2);
            }
            BONUS_AMY_STUN => round.amy_stuns += 1,
            _ => {}
        }
    }

    fn stunned_exe(&mut self) {
        self.round.stunned_exe += 1;
        self.unlock_when(self.round.stunned_exe >= 5, id::STUN_EXE_5);
    }

    /// SERVER_REVIVAL_RINGSUB: a teammate this player revived is up again.
    pub fn revived_teammate(&mut self) {
        self.round.revivals += 1;
        self.unlock_when(self.round.revival_window > 0, id::REVIVE_2_AT_ONCE);
        self.round.revival_window = ticks(REVIVAL_WINDOW_SECONDS);
        for (count, index) in [(1, id::REVIVE_1), (2, id::REVIVE_2), (3, id::REVIVE_3)] {
            self.unlock_when(self.round.revivals >= count, index);
        }
    }

    /// This player came back as a demon (SERVER_PLAYER_DEMONIZED in the original).
    pub fn demonized(&mut self) {
        self.round.demonized_window = ticks(DEMONIZED_WINDOW_SECONDS);
    }

    /// This player lost health, from whatever it was (wasHurt).
    pub fn hurt(&mut self) {
        self.round.was_hurt = true;
    }

    /// scr_player_instakill: shards found by someone who dies do not count for them.
    pub fn died(&mut self) {
        self.round.late_shards = 0;
    }

    /// scr_move_basic: this player's rings.
    pub fn rings(&mut self, rings: i32) {
        self.unlock_when(rings >= 25, id::RINGS_25);
    }

    /// SERVER_GAME_PLAYER_ESCAPED: someone else got out.
    pub fn other_escaped(&mut self) {
        self.round.others_escaped += 1;
    }

    /// SERVER_PLAYER_ESCAPED: this player got out, maybe with one hit left in the last seconds.
    pub fn escaped(&mut self, now: &Situation) {
        self.unlock_when(now.hp <= 1 && now.seconds_left <= LAST_SECONDS, id::LAST_SECOND_ESCAPE);
    }

    /// SERVER_GAME_EXE_WINS, SERVER_GAME_TIME_OVER and SERVER_GAME_SURVIVOR_WIN, with the
    /// server's word on whether the round counts; `exe_of_round`: the EXE survived against.
    pub fn round_ended(&mut self, ending: RoundEnding, counts: bool, now: &Situation, exe_of_round: Option<ExeCharacter>) {
        if !counts {
            return;
        }
        let map = now.room.file_name().to_string();
        let is_exe = now.character == Character::Exe;
        match ending {
            RoundEnding::ExeWon | RoundEnding::TimeOver if is_exe => {
                self.record.exe_wins += 1;
                self.won_on(map);
                let name = exe_name(now.exe_character).to_string();
                if !self.record.won_as_exe.contains(&name) {
                    self.record.won_as_exe.push(name);
                }
                self.save();
                let every_exe = [ExeCharacter::Original, ExeCharacter::Chaos, ExeCharacter::Exetior, ExeCharacter::Exeller].iter().all(|&exe| self.record.won_as_exe.iter().any(|won| won == exe_name(exe)));
                self.unlock_when(every_exe, id::EVERY_EXE);
            }
            RoundEnding::SurvivorsEscaped if !is_exe && !now.demonized && now.hp > 0 => {
                self.record.survivor_wins += 1;
                self.won_on(map);
                let name = survivor_name(now.character).to_string();
                if !self.record.survived_as.contains(&name) {
                    self.record.survived_as.push(name);
                }
                self.save();
                let round = &self.round;
                let unlocks = [
                    (now.room == RoomId::Pricelessfreedom && round.black_rings == 0, id::PRICELESS_FREEDOM_NO_BLACK_RINGS),
                    (round.late_shards >= 6, id::LATE_SHARDS_6),
                    (now.character == Character::Amy && round.amy_stuns == 0, id::AMY_NO_STUNS),
                    (!round.was_hurt, id::UNHURT_WIN),
                    (round.others_escaped >= 5, id::ESCAPE_WITH_5),
                ];
                for (reached, index) in unlocks {
                    self.unlock_when(reached, index);
                }
                for (room, character, against, index) in SURVIVED_ON {
                    let against_matches = against.is_none() || against == exe_of_round;
                    self.unlock_when(now.room == room && now.character == character && against_matches, index);
                }
                let every_survivor = [Character::Tails, Character::Knux, Character::Eggman, Character::Amy, Character::Cream, Character::Sally]
                    .iter()
                    .all(|&survivor| self.record.survived_as.iter().any(|survived| survived == survivor_name(survivor)));
                self.unlock_when(every_survivor, id::EVERY_SURVIVOR);
            }
            _ => return,
        }
        let wins = self.record.survivor_wins + self.record.exe_wins;
        for (needed, index) in WINS {
            self.unlock_when(wins >= needed, index);
        }
        self.unlock_when(self.record.maps.len() >= EVERY_MAP_COUNT, id::EVERY_MAP);
    }

    fn won_on(&mut self, map: String) {
        if !self.record.maps.contains(&map) {
            self.record.maps.push(map);
        }
    }
}

fn survivor_name(character: Character) -> &'static str {
    match character {
        Character::Tails => "tails",
        Character::Knux => "knuckles",
        Character::Eggman => "eggman",
        Character::Amy => "amy",
        Character::Cream => "cream",
        Character::Sally => "sally",
        Character::Exe => "exe",
    }
}

fn exe_name(exe: ExeCharacter) -> &'static str {
    match exe {
        ExeCharacter::Original => "exe",
        ExeCharacter::Chaos => "chaos",
        ExeCharacter::Exetior => "exetior",
        ExeCharacter::Exeller => "exeller",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A record in a folder of its own, so the player's is never touched.
    fn fresh(name: &str) -> Achievements {
        let path = std::env::temp_dir().join(format!("achievements_{}_{name}.toml", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut achievements = Achievements::load_from(&path);
        achievements.start_round(RoomId::Greenhill, true);
        achievements
    }

    fn as_character(character: Character) -> Situation {
        Situation { room: RoomId::Greenhill, character, exe_character: ExeCharacter::Original, demonized: false, hp: 3, seconds_left: 100, full_demon_shot: 3 }
    }

    #[test]
    fn a_round_that_does_not_count_earns_nothing() {
        let mut achievements = fresh("offline");
        achievements.start_round(RoomId::Greenhill, false);
        achievements.rings(30);
        assert!(!achievements.is_unlocked(id::RINGS_25));
        achievements.start_round(RoomId::Fartzone, true);
        achievements.rings(30);
        assert!(!achievements.is_unlocked(id::RINGS_25), "Fart Zone counts for nothing");
        achievements.start_round(RoomId::Greenhill, true);
        achievements.rings(30);
        assert!(achievements.is_unlocked(id::RINGS_25) && achievements.take_new_unlock());
        assert!(!achievements.take_new_unlock(), "the sound plays once");
    }

    #[test]
    fn exe_kills_count_over_rounds_and_are_saved() {
        let mut achievements = fresh("kills");
        let exe = as_character(Character::Exe);
        for _ in 0..3 {
            achievements.on_event(&SimEvent::KilledByExe { victim: 1, killer: 0 }, |index| index == 0, false, |_| false, &exe);
        }
        assert!(achievements.is_unlocked(12));
        let again = Achievements::load_from(&achievements.path);
        assert_eq!((again.record.exe_kills, again.is_unlocked(12)), (3, true), "kept in the file");
        let _ = std::fs::remove_file(&achievements.path);
    }

    #[test]
    fn windows_close_and_clear_their_counters() {
        let mut achievements = fresh("windows");
        let knuckles = as_character(Character::Knux);
        let bonus = SimEvent::MercoinBonus { player: 0, bonus: BONUS_UPPERCUT };
        achievements.on_event(&bonus, |index| index == 0, false, |_| false, &knuckles);
        for _ in 0..ticks(UPPERCUT_WINDOW_SECONDS) {
            achievements.tick();
        }
        achievements.on_event(&bonus, |index| index == 0, false, |_| false, &knuckles);
        assert!(!achievements.is_unlocked(id::UPPERCUT_2), "a second apart is not at once");
        achievements.on_event(&bonus, |index| index == 0, false, |_| false, &knuckles);
        assert!(achievements.is_unlocked(id::UPPERCUT_2), "two within the second");
        let _ = std::fs::remove_file(&achievements.path);
    }

    #[test]
    fn a_survivors_win_checks_the_round() {
        let mut achievements = fresh("win");
        let mut tails = as_character(Character::Tails);
        tails.room = RoomId::Hideandseek2;
        achievements.round_ended(RoundEnding::SurvivorsEscaped, true, &tails, Some(ExeCharacter::Original));
        assert!(achievements.is_unlocked(38), "survived as Tails on Hide and Seek");
        assert!(achievements.is_unlocked(id::UNHURT_WIN), "never hurt");
        assert!(!achievements.is_unlocked(id::PRICELESS_FREEDOM_NO_BLACK_RINGS), "another map");
        assert_eq!(achievements.record.survivor_wins, 1);
        let _ = std::fs::remove_file(&achievements.path);
    }
}
