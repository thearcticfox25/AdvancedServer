//! Every gameplay number, named, in editable files.
//!
//! The server reads `PersistentData/Core/<File>.toml` (one file per
//! topic) and sends the result to each client on join, so client prediction
//! always uses the server's numbers. A missing file or key means "the value of
//! the original game", and the server writes missing files so owners can see
//! and edit every value without rebuilding.
//!
//! Units are part of the names: `_seconds`, `_percent`, `_per_tick` (pixels per
//! game tick at 60 ticks per second). Exact parity with the original holds only
//! with the default values.

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::sync::atomic::{AtomicU32, Ordering};

/// Where the server keeps these files. They are neither the server's own settings nor the
/// client's: they describe the game itself, which both sides simulate.
pub const CONFIG_FOLDER: &str = "PersistentData/Core";

/// Ticks per second the original was built around. Every `_per_tick` number stays a step
/// of this size whatever rate the round runs at, so the config files keep the original's
/// numbers and `step()` below turns them into the step actually taken.
pub const ORIGINAL_TICKS_PER_SECOND: f64 = 60.0;

/// The rate rounds run at when the server owner does not set one. More ticks a second
/// means the server sees a shot or a hit closer to when it happened (the reason CS:GO
/// leagues ran 128 instead of 64), at the price of more work and more packets.
pub const DEFAULT_TICKS_PER_SECOND: u32 = 72;

/// The rate of this process, set once from the server's Config.toml (and on the client
/// from the rate the server sends), read everywhere a tick is turned into time.
static TICK_RATE: AtomicU32 = AtomicU32::new(DEFAULT_TICKS_PER_SECOND);

/// Sets the rate of every round this process simulates. Called before a round starts.
pub fn set_tick_rate(ticks_per_second: u32) {
    TICK_RATE.store(ticks_per_second.clamp(MIN_TICK_RATE, MAX_TICK_RATE), Ordering::Relaxed);
}

/// Below the original's rate the simulation would step so coarsely that the sensors miss
/// thin floors; above a few hundred a step is smaller than the rounding the sensors do.
pub const MIN_TICK_RATE: u32 = 60;
pub const MAX_TICK_RATE: u32 = 300;

#[cfg(not(test))]
pub fn ticks_per_second() -> f64 {
    TICK_RATE.load(Ordering::Relaxed) as f64
}

// Tests compare one rate against another, and they run at the same time in one process,
// so under test each thread carries its own rate; it is the original's until a test says
// otherwise, which keeps every parity check written in whole 60 Hz ticks honest.
#[cfg(test)]
thread_local! {
    static TEST_TICK_RATE: std::cell::Cell<f64> = const { std::cell::Cell::new(ORIGINAL_TICKS_PER_SECOND) };
}

#[cfg(test)]
pub fn ticks_per_second() -> f64 {
    TEST_TICK_RATE.with(|rate| rate.get())
}

#[cfg(test)]
pub fn set_tick_rate_of_this_thread(ticks_per_second: f64) {
    TEST_TICK_RATE.with(|rate| rate.set(ticks_per_second));
}

/// How much of an original 60 Hz step one tick is. Every number a step adds to a speed or
/// a position is multiplied by it, which is what keeps the game the same speed in seconds
/// at any rate: half the step, half the movement, twice as many steps.
pub fn step() -> f64 {
    ORIGINAL_TICKS_PER_SECOND / ticks_per_second()
}

/// The share of the way an easing (x += (target - x) * share, once a 60 Hz step) covers
/// in one step of the rate the round runs at, so it covers the same way in a second.
pub fn eased_share(share_at_60: f64) -> f64 {
    1.0 - (1.0 - share_at_60).powf(step())
}

/// A count of the original's 60 Hz steps, as the `_ticks` numbers of the config files
/// are written, in steps of the rate the round runs at: the same time at any rate.
pub fn original_ticks(steps_at_60: i32) -> i32 {
    ticks(f64::from(steps_at_60) / ORIGINAL_TICKS_PER_SECOND)
}

/// Seconds from a config file to whole ticks. Rounding keeps 0.4 s = 24 ticks exact at 60.
pub fn ticks(seconds: f64) -> i32 {
    (seconds * ticks_per_second()).round() as i32
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct GameplayConfig {
    pub physics: Physics,
    pub hazards: Hazards,
    pub hurt: HurtRules,
    pub hiding: HidingRules,
    pub contact: ContactRules,
    pub rings: RingRules,
    pub springs: Springs,
    pub tails: Tails,
    pub knuckles: Knuckles,
    pub eggman: Eggman,
    pub amy: Amy,
    pub cream: Cream,
    pub sally: Sally,
    /// Sonic.EXE, the original killer.
    pub exe: ExeOriginal,
    pub chaos: Chaos,
    pub exetior: Exetior,
    pub exeller: Exeller,
    /// The maps' own objects, a section per map.
    pub levels: LevelRules,
}

/// A max_hp of at least this many hits is one of the old 0..100 files, not a real setting.
const OLD_HEALTH_SCALE_FROM: i32 = 20;

/// The gameplay numbers of CONFIG_FOLDER, read once a program: the server's rounds
/// and the game's Singleplayer both play by them. Missing files are written with the
/// original values, so the folder shows what can be changed.
pub fn gameplay_config() -> Arc<GameplayConfig> {
    static CONFIG: OnceLock<Arc<GameplayConfig>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let folder = Path::new(CONFIG_FOLDER);
            let config = GameplayConfig::load(folder).unwrap_or_else(|error| {
                log::error!("{CONFIG_FOLDER} is broken, using the original values: {error:#}");
                GameplayConfig::default()
            });
            if let Err(error) = config.write_missing_files(folder) {
                log::warn!("could not write the missing files of {CONFIG_FOLDER}: {error:#}");
            }
            // Health used to be counted 0..100 with 20 for a hit; it is counted in hits
            // now. A Hurt.toml written before that reads as 100 hits, and its damage
            // numbers as 20 hits a hit, which kills every survivor on the first scratch.
            if config.hurt.max_hp > OLD_HEALTH_SCALE_FROM {
                log::warn!(
                    "{CONFIG_FOLDER}/Hurt.toml has max_hp = {}: that is the old health scale, which counted to 100. \
                     Health is counted in hits now ({} by default), so divide the health and damage numbers of these files by 20.",
                    config.hurt.max_hp,
                    GameplayConfig::default().hurt.max_hp,
                );
            }
            Arc::new(config)
        })
        .clone()
}

impl GameplayConfig {
    /// Reads every topic file in `folder`; missing files keep the original values.
    pub fn load(folder: &Path) -> Result<GameplayConfig> {
        Ok(GameplayConfig {
            physics: load_file(folder, "Physics")?,
            hazards: load_file(folder, "Hazards")?,
            hurt: load_file(folder, "Hurt")?,
            hiding: load_file(folder, "Hiding")?,
            contact: load_file(folder, "Contact")?,
            rings: load_file(folder, "Rings")?,
            springs: load_file(folder, "Springs")?,
            tails: load_file(folder, "Tails")?,
            knuckles: load_file(folder, "Knuckles")?,
            eggman: load_file(folder, "Eggman")?,
            amy: load_file(folder, "Amy")?,
            cream: load_file(folder, "Cream")?,
            sally: load_file(folder, "Sally")?,
            exe: load_file(folder, "SonicExe")?,
            chaos: load_file(folder, "Chaos")?,
            exetior: load_file(folder, "Exetior")?,
            exeller: load_file(folder, "Exeller")?,
            levels: load_file(folder, "Levels")?,
        })
    }

    /// Writes the topic files that do not exist yet, with the current values.
    pub fn write_missing_files(&self, folder: &Path) -> Result<()> {
        std::fs::create_dir_all(folder)?;
        save_if_missing(folder, "Physics", &self.physics)?;
        save_if_missing(folder, "Hazards", &self.hazards)?;
        save_if_missing(folder, "Hurt", &self.hurt)?;
        save_if_missing(folder, "Hiding", &self.hiding)?;
        save_if_missing(folder, "Contact", &self.contact)?;
        save_if_missing(folder, "Rings", &self.rings)?;
        save_if_missing(folder, "Springs", &self.springs)?;
        save_if_missing(folder, "Tails", &self.tails)?;
        save_if_missing(folder, "Knuckles", &self.knuckles)?;
        save_if_missing(folder, "Eggman", &self.eggman)?;
        save_if_missing(folder, "Amy", &self.amy)?;
        save_if_missing(folder, "Cream", &self.cream)?;
        save_if_missing(folder, "Sally", &self.sally)?;
        save_if_missing(folder, "SonicExe", &self.exe)?;
        save_if_missing(folder, "Chaos", &self.chaos)?;
        save_if_missing(folder, "Exetior", &self.exetior)?;
        save_if_missing(folder, "Exeller", &self.exeller)?;
        save_if_missing(folder, "Levels", &self.levels)?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct LevelRules {
    // In the order of the map numbers (maps/mod.rs MAP_LIST), which Levels.toml follows.
    // Hide and Seek 2 (0), Desert Town (3) and Majin Forest (13) have no rules of their own.
    pub ravine_mist: RavineMistRules,
    pub dotdotdot: DotDotDotRules,
    pub you_cant_run: YouCantRunRules,
    pub limp_city: LimpCityRules,
    pub not_perfect: NotPerfectRules,
    pub kind_and_fair: KindAndFairRules,
    pub act9: Act9Rules,
    pub nasty_paradise: NastyParadiseRules,
    pub priceless_freedom: PricelessFreedomRules,
    pub volcano_valley: VolcanoValleyRules,
    pub green_hill: GreenHillRules,
    pub angel_island: AngelIslandRules,
    pub torture_cave: TortureCaveRules,
    pub dark_tower: DarkTowerRules,
    pub haunting_dream: HauntingDreamRules,
    pub weed_zone: WeedZoneRules,
    pub echidna_ruins: EchidnaRuinsRules,
    pub fart_zone: FartZoneRules,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct DotDotDotRules {
    /// A ladder holds a player (who is not slowed) to this speed, and slows its walking
    /// animation this many times.
    pub ladder_max_speed: f64,
    pub ladder_walk_slowdown: f64,
}

impl Default for DotDotDotRules {
    fn default() -> DotDotDotRules {
        DotDotDotRules { ladder_max_speed: 7.0, ladder_walk_slowdown: 2.5 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct TortureCaveRules {
    /// Every `acid_seconds` the acid either stops or starts in one random group of clouds.
    pub acid_seconds: f64,
    /// A cloud burns once its frame reaches this; standing in it costs a ring, or health
    /// without rings, `acid_first_bite_seconds` in and then every `acid_bite_seconds`.
    pub acid_harmful_frame: f64,
    pub acid_first_bite_seconds: f64,
    pub acid_bite_seconds: f64,
    pub acid_damage: i32,
    pub acid_knockback_x: f64,
}

impl Default for TortureCaveRules {
    fn default() -> TortureCaveRules {
        TortureCaveRules {
            acid_seconds: 4.0,
            acid_harmful_frame: 18.0,
            acid_first_bite_seconds: 0.1,
            acid_bite_seconds: 0.6,
            acid_damage: 1,
            acid_knockback_x: 4.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct LimpCityRules {
    /// The electric chains rest this long, warn this long, then shock this long.
    pub chain_rest_seconds: f64,
    pub chain_warning_seconds: f64,
    pub chain_shock_seconds: f64,
    pub chain_damage: i32,
    pub chain_knockback_x: f64,
    /// Looking through an eye costs `eye_use_cost` of its charge a second; it
    /// recharges `eye_recharge` a second while unused, needs `eye_min_charge` to be
    /// used, and rests `eye_rest_seconds` when it runs dry.
    pub eye_use_cost: u8,
    pub eye_recharge: u8,
    pub eye_min_charge: u8,
    pub eye_rest_seconds: f64,
}

impl Default for LimpCityRules {
    fn default() -> LimpCityRules {
        LimpCityRules {
            chain_rest_seconds: 8.0,
            chain_warning_seconds: 2.0,
            chain_shock_seconds: 2.0,
            chain_damage: 1,
            chain_knockback_x: 4.0,
            eye_use_cost: 20,
            eye_recharge: 10,
            eye_min_charge: 20,
            eye_rest_seconds: 2.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct EchidnaRuinsRules {
    pub lava: LavaRules,
    /// The crystals light up or go out every `crystal_seconds` plus up to
    /// `crystal_extra_seconds` - 1 whole random seconds; a lit one reverses the controls
    /// of whoever touches it for `reversed_seconds`, once it has glowed up fully.
    pub crystal_seconds: f64,
    pub crystal_extra_seconds: u32,
    pub crystal_glow_per_tick: f64,
    pub reversed_seconds: f64,
    /// The judgers wait, get ready and fire in turn (see mj_judger).
    pub judger_wait_seconds: f64,
    pub judger_wait_extra_seconds: u32,
    pub judger_ready_seconds: f64,
    pub judger_fire_seconds: f64,
    pub judger_fire_extra_seconds: u32,
}

impl Default for EchidnaRuinsRules {
    fn default() -> EchidnaRuinsRules {
        EchidnaRuinsRules {
            lava: LavaRules { rest_seconds: 5.0, rest_extra_seconds: 4, first_rest_seconds: Some(5.0), rise: 512.0, ..LavaRules::default() },
            crystal_seconds: 10.0,
            crystal_extra_seconds: 5,
            crystal_glow_per_tick: 0.016,
            reversed_seconds: 5.0,
            judger_wait_seconds: 10.0,
            judger_wait_extra_seconds: 5,
            judger_ready_seconds: 2.0,
            judger_fire_seconds: 7.0,
            judger_fire_extra_seconds: 2,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct RavineMistRules {
    /// This many shards lie on random ones of `shard_spots`; the big ring opens only
    /// once `shards_needed` of them have been found.
    pub shard_amount: usize,
    pub shards_needed: usize,
    pub shard_spots: Vec<[f64; 2]>,
    /// A dying survivor drops the shards they carry, up to this far to either side;
    /// dropped shards fall with this gravity up to this speed.
    pub shard_drop_spread: i32,
    pub shard_gravity: f64,
    pub shard_max_fall_speed: f64,
    /// A slug crawls out of each spawner this often (the first up to a second later)
    /// while its last one is gone, and crawls this far to either side of it.
    pub slugs: bool,
    pub slug_spawners: Vec<[f64; 2]>,
    pub slug_every_seconds: f64,
    pub slug_walk: f64,
    pub slug_speed: f64,
    /// Percent chances that a slug carries a red ring, or a ring.
    pub slug_red_ring_chance: u32,
    pub slug_ring_chance: u32,
    /// Standing in a slug costs a ring, or health without rings, this often.
    pub slug_bite_every_ticks: i32,
    pub slug_damage: i32,
    pub slug_knockback_x: f64,
    /// Jumping on a slug squashes it and throws the survivor up this fast.
    pub slug_bounce_speed: f64,
}

impl Default for RavineMistRules {
    fn default() -> RavineMistRules {
        RavineMistRules {
            shard_amount: 7,
            shards_needed: 6,
            shard_spots: vec![
                [862.0, 248.0],
                [3078.0, 248.0],
                [292.0, 558.0],
                [2918.0, 558.0],
                [1100.0, 820.0],
                [980.0, 1188.0],
                [1870.0, 1252.0],
                [2180.0, 1508.0],
                [2920.0, 2216.0],
                [282.0, 2228.0],
                [1318.0, 1916.0],
                [3010.0, 1766.0],
            ],
            shard_drop_spread: 8,
            shard_gravity: 0.16,
            shard_max_fall_speed: 6.0,
            slugs: true,
            slug_spawners: vec![
                [1901.0, 392.0],
                [2193.0, 392.0],
                [2468.0, 392.0],
                [1188.0, 860.0],
                [2577.0, 1952.0],
                [2564.0, 2264.0],
                [2782.0, 2264.0],
                [1441.0, 2264.0],
                [884.0, 2264.0],
                [988.0, 2004.0],
                [915.0, 2004.0],
            ],
            slug_every_seconds: 15.0,
            slug_walk: 100.0,
            slug_speed: 1.0,
            slug_red_ring_chance: 10,
            slug_ring_chance: 40,
            slug_bite_every_ticks: 10,
            slug_damage: 1,
            slug_knockback_x: 4.0,
            slug_bounce_speed: -5.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct WeedZoneRules {
    /// The lanterns are dark for this long plus 0 to `lantern_dark_extra_seconds` - 1
    /// whole seconds, then one random lantern is lit for `lantern_lit_seconds` plus the same.
    pub lantern_dark_seconds: f64,
    pub lantern_dark_extra_seconds: u32,
    pub lantern_lit_seconds: f64,
    pub lantern_lit_extra_seconds: u32,
    /// A conveyor under the feet moves the player this far left a tick.
    pub conveyor_speed: f64,
}

impl Default for WeedZoneRules {
    fn default() -> WeedZoneRules {
        WeedZoneRules { lantern_dark_seconds: 7.0, lantern_dark_extra_seconds: 2, lantern_lit_seconds: 20.0, lantern_lit_extra_seconds: 2, conveyor_speed: 4.0 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Act9Rules {
    /// The walls close in as the round's time runs out: the ceiling comes this far down,
    /// each side wall this far in.
    pub walls_close: bool,
    pub ceiling_travel: f64,
    pub side_travel: f64,
    /// Whoever a wall crushes, or who is squeezed past one, comes back here.
    pub crushed_to: [f64; 2],
}

impl Default for Act9Rules {
    fn default() -> Act9Rules {
        Act9Rules { walls_close: true, ceiling_travel: 1025.0, side_travel: 1663.0, crushed_to: [1535.0, 943.0] }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct NotPerfectRules {
    /// The next stage's shadow shows this long after a switch, and the switch comes
    /// this long after that; the `_chase` times once the round has
    /// `chase_seconds_left` left, when a switch also comes at once.
    pub warning_after_seconds: f64,
    pub warning_after_seconds_chase: f64,
    pub switch_after_seconds: f64,
    pub switch_after_seconds_chase: f64,
    pub chase_seconds_left: f64,
    /// The four stages are copies this far apart: stage 1 right of stage 0, stage 2
    /// below it, stage 3 both.
    pub stage_offset: [f64; 2],
}

impl Default for NotPerfectRules {
    fn default() -> NotPerfectRules {
        NotPerfectRules {
            warning_after_seconds: 15.0,
            warning_after_seconds_chase: 2.0,
            switch_after_seconds: 5.0,
            switch_after_seconds_chase: 3.0,
            chase_seconds_left: 60.0,
            stage_offset: [2904.0, 1368.0],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct DarkTowerRules {
    /// The balls swing between -1 and 1 of their map distance by this much a tick.
    pub ball_shift_per_tick: f64,
    pub ball_damage: i32,
    pub ball_knockback_x: f64,
    /// A stalactite falls on a live player up to this far below it, within its width;
    /// it can once shown for `stalactite_armed_after_seconds`.
    pub stalactite_notice_below: f64,
    pub stalactite_notice_width: f64,
    pub stalactite_armed_after_seconds: f64,
    pub stalactite_acceleration: f64,
    pub stalactite_damage: i32,
    pub stalactite_knockback_x: f64,
    /// A broken stalactite is back after this plus up to `stalactite_back_extra_seconds`
    /// - 1 whole random seconds.
    pub stalactite_back_seconds: f64,
    pub stalactite_back_extra_seconds: u32,
    /// The Tails Doll waits at a random spot at least `doll_spot_away` from everyone,
    pub doll_spots: Vec<[f64; 2]>,
    pub doll_spot_away: f64,
    /// notices a survivor within `doll_notice`, waits `doll_ready_seconds` plus 0 or
    /// `doll_ready_extra_seconds`,
    pub doll_notice: f64,
    pub doll_ready_seconds: f64,
    pub doll_ready_extra_seconds: f64,
    /// chases, gaining speed on each axis unless that close on it,
    pub doll_acceleration: [f64; 2],
    pub doll_dead_zone: [f64; 2],
    pub doll_max_speed: f64,
    /// and catching one within `doll_catch` scares and slows them.
    pub doll_catch: f64,
    pub doll_slow_percent: f64,
    pub doll_slow_seconds: f64,
}

impl Default for DarkTowerRules {
    fn default() -> DarkTowerRules {
        DarkTowerRules {
            ball_shift_per_tick: 0.015,
            ball_damage: 1,
            ball_knockback_x: 4.0,
            stalactite_notice_below: 336.0,
            stalactite_notice_width: 80.0,
            stalactite_armed_after_seconds: 1.0,
            stalactite_acceleration: 0.164,
            stalactite_damage: 1,
            stalactite_knockback_x: 4.0,
            stalactite_back_seconds: 25.0,
            stalactite_back_extra_seconds: 5,
            doll_spots: vec![
                [177.0, 944.0],
                [1953.0, 544.0],
                [3279.0, 224.0],
                [4101.0, 544.0],
                [4060.0, 1264.0],
                [3805.0, 1824.0],
                [2562.0, 1584.0],
                [515.0, 1824.0],
                [2115.0, 1056.0],
                [984.0, 1184.0],
                [1498.0, 1504.0],
            ],
            doll_spot_away: 480.0,
            doll_notice: 130.0,
            doll_ready_seconds: 1.0,
            doll_ready_extra_seconds: 0.5,
            doll_acceleration: [0.512, 0.48],
            doll_dead_zone: [4.0, 5.0],
            doll_max_speed: 5.0,
            doll_catch: 12.0,
            doll_slow_percent: 40.0,
            doll_slow_seconds: 3.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct VolcanoValleyRules {
    pub lava: LavaRules,
    /// A vase holds 1 to this many rings, for whoever breaks it.
    pub vase_max_rings: u32,
}

impl Default for VolcanoValleyRules {
    fn default() -> VolcanoValleyRules {
        VolcanoValleyRules { lava: LavaRules::default(), vase_max_rings: 4 }
    }
}

/// Lava that rises and sinks (the servers' vv_lava and mj_lava).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct LavaRules {
    /// It bobs `bob` px (a radian every `bob_ticks_per_radian` ticks) for `rest_seconds`
    /// plus up to `rest_extra_seconds` - 1 whole random seconds (the first time
    /// `first_rest_seconds`, when given),
    pub rest_seconds: f64,
    pub rest_extra_seconds: u32,
    pub first_rest_seconds: Option<f64>,
    pub bob: f64,
    pub bob_ticks_per_radian: f64,
    /// sinks `sink` px at `sink_speed`, rises `rise` px above its place speeding up by
    /// `acceleration` to `max_speed`,
    pub sink: f64,
    pub sink_speed: f64,
    pub rise: f64,
    pub acceleration: f64,
    pub max_speed: f64,
    /// bobs up there for `top_seconds` plus up to `top_extra_seconds` - 1 whole random
    /// seconds, and comes back down as it rose.
    pub top_seconds: f64,
    pub top_extra_seconds: u32,
    pub damage: i32,
    pub knockback_x: f64,
    pub knockback_y: f64,
}

impl Default for LavaRules {
    fn default() -> LavaRules {
        LavaRules {
            rest_seconds: 20.0,
            rest_extra_seconds: 5,
            first_rest_seconds: None,
            bob: 6.0,
            bob_ticks_per_radian: 25.0,
            sink: 20.0,
            sink_speed: 0.15,
            rise: 130.0,
            acceleration: 0.08,
            max_speed: 5.0,
            top_seconds: 4.0,
            top_extra_seconds: 3,
            damage: 1,
            knockback_x: 4.0,
            knockback_y: -2.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct NastyParadiseRules {
    /// A broken ice block grows back after this long.
    pub ice_regeneration_seconds: f64,
    /// Landing on an ice block breaks it and throws the player up this fast.
    pub ice_bounce_speed: f64,
    pub snowballs_roll: bool,
    /// Every snowball that is not rolling starts down its path this often.
    pub snowball_every_seconds: f64,
    /// A starting snowball gains this much speed a tick; its frame moves by speed times
    /// `snowball_start_frames` and its progress along a leg by speed times
    /// `snowball_start_progress`, until the speed is over 1.
    pub snowball_start_acceleration: f64,
    pub snowball_start_frames: f64,
    pub snowball_start_progress: f64,
    /// Then each tick it goes this part of a leg and this many frames, unless the leg's
    /// first waypoint says otherwise (map properties roll_speed and roll_frames).
    pub snowball_roll_speed: f64,
    pub snowball_roll_frames: f64,
    /// Once on its rolling frames a snowball hurts whoever it rolls over and slows them.
    pub snowball_damage: i32,
    pub snowball_knockback_x: f64,
    pub snowball_slow_seconds: f64,
}

impl Default for NastyParadiseRules {
    fn default() -> NastyParadiseRules {
        NastyParadiseRules {
            ice_regeneration_seconds: 15.0,
            ice_bounce_speed: -5.0,
            snowballs_roll: true,
            snowball_every_seconds: 20.0,
            snowball_start_acceleration: 0.016,
            snowball_start_frames: 0.45,
            snowball_start_progress: 0.05,
            snowball_roll_speed: 0.05,
            snowball_roll_frames: 0.35,
            snowball_damage: 1,
            snowball_knockback_x: 4.0,
            snowball_slow_seconds: 3.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct FartZoneRules {
    /// Black rings on the map's first spawners when the round starts (0: none).
    pub black_rings: usize,
    /// The training dummy slides between these, slowing by `dummy_friction` a tick.
    pub dummy_left: f64,
    pub dummy_right: f64,
    pub dummy_friction: f64,
    /// After a hit the dummy ignores hits this many ticks (a stomp wave's: the second).
    pub dummy_rest_ticks: i32,
    pub dummy_wave_rest_ticks: i32,
    /// Holding the statue's curse lasts this long; it passes on by touch once this much is gone.
    pub potato_seconds: f64,
    pub potato_passes_after_seconds: f64,
}

impl Default for FartZoneRules {
    fn default() -> FartZoneRules {
        FartZoneRules {
            black_rings: 3,
            dummy_left: 1282.0,
            dummy_right: 2944.0,
            dummy_friction: 0.046875 * 4.0,
            dummy_rest_ticks: 20,
            dummy_wave_rest_ticks: 10,
            potato_seconds: 4.0,
            potato_passes_after_seconds: 0.5,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct PricelessFreedomRules {
    /// A lift gains this much speed a tick up to its top speed (pixels per tick).
    pub lift_acceleration: f64,
    pub lift_max_speed: f64,
    /// The rider hangs this far above the lift and leaves it with this vertical speed.
    pub lift_rider_above: f64,
    pub lift_leave_speed: f64,
    /// After a ride the lift fades out, and is back at its start after this long; it
    /// takes riders again once faded in.
    pub lift_rest_seconds: f64,
    pub lift_fade_per_step: f64,
    /// Black rings on the map's first spawners when the round starts (0: none).
    pub black_rings: usize,
}

impl Default for PricelessFreedomRules {
    fn default() -> PricelessFreedomRules {
        PricelessFreedomRules {
            lift_acceleration: 0.052,
            lift_max_speed: 7.0,
            lift_rider_above: 16.0,
            lift_leave_speed: -3.0,
            lift_rest_seconds: 1.5,
            lift_fade_per_step: 0.016,
            black_rings: 29,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct HauntingDreamRules {
    /// After a crystal opens or closes the doors, no crystal works for this long.
    pub door_toggle_rest_seconds: f64,
}

impl Default for HauntingDreamRules {
    fn default() -> HauntingDreamRules {
        HauntingDreamRules { door_toggle_rest_seconds: 10.0 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct KindAndFairRules {
    /// A broken speed monitor is back after this, plus up to
    /// `monitor_broken_offset_seconds` - 1 whole random seconds.
    pub monitor_broken_seconds: f64,
    pub monitor_broken_offset_seconds: u32,
}

impl Default for KindAndFairRules {
    fn default() -> KindAndFairRules {
        KindAndFairRules { monitor_broken_seconds: 25.0, monitor_broken_offset_seconds: 5 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct YouCantRunRules {
    /// The gas fills one of `gas_areas` random areas, then clears, each for this long
    /// (Config.toml's you_cant_run.gas.delay in the original round mode).
    pub gas_seconds: f64,
    pub gas_areas: u32,
    /// Breathing it this long brings the red screen for `gas_red_screen_seconds`;
    /// after that it eats a ring, or hurts without rings, every `gas_hurt_every_seconds`.
    pub gas_red_screen_after_seconds: f64,
    pub gas_red_screen_seconds: f64,
    pub gas_hurt_every_seconds: f64,
    pub gas_damage: i32,
    pub gas_knockback_x: f64,
}

impl Default for YouCantRunRules {
    fn default() -> YouCantRunRules {
        YouCantRunRules {
            gas_seconds: 6.0,
            gas_areas: 7,
            gas_red_screen_after_seconds: 1.0,
            gas_red_screen_seconds: 3.0,
            gas_hurt_every_seconds: 1.0,
            gas_damage: 1,
            gas_knockback_x: 4.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct AngelIslandRules {
    /// A ride's progress along the line per tick: starting value, gain per tick, top.
    pub zipline_start_speed: f64,
    pub zipline_acceleration: f64,
    pub zipline_max_speed: f64,
    /// Jump lets go once this much of the line is behind.
    pub zipline_release_after_progress: f64,
    /// At the end of the line the rider flies on at the ride's speed times this.
    pub zipline_launch_speed_per_progress: f64,
    /// The released handle falls away, and can be taken again this long after it is back.
    pub zipline_handle_gravity: f64,
    pub zipline_regrab_seconds: f64,
}

impl Default for AngelIslandRules {
    fn default() -> AngelIslandRules {
        AngelIslandRules {
            zipline_start_speed: 0.001,
            zipline_acceleration: 0.0001,
            zipline_max_speed: 0.016,
            zipline_release_after_progress: 0.1,
            zipline_launch_speed_per_progress: 150.0,
            zipline_handle_gravity: 0.32,
            zipline_regrab_seconds: 3.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct GreenHillRules {
    /// Time from one lightning cycle to the next, plus up to
    /// `thunder_cycle_offset_seconds` - 1 whole random seconds.
    pub thunder_cycle_seconds: f64,
    pub thunder_cycle_offset_seconds: u32,
    /// The lightning strikes this long before a cycle ends; the water shocks until then.
    pub shock_seconds: f64,
    pub shock_damage: i32,
    pub shock_knockback_x: f64,
    /// Standing in the water keeps a player slowed for this long.
    pub water_slow_seconds: f64,
}

impl Default for GreenHillRules {
    fn default() -> GreenHillRules {
        GreenHillRules {
            thunder_cycle_seconds: 15.0,
            thunder_cycle_offset_seconds: 5,
            shock_seconds: 2.0,
            shock_damage: 1,
            shock_knockback_x: 4.0,
            water_slow_seconds: 0.5,
        }
    }
}

fn load_file<T: DeserializeOwned + Default>(folder: &Path, name: &str) -> Result<T> {
    let path = folder.join(format!("{name}.toml"));
    if !path.exists() {
        return Ok(T::default());
    }
    let text = std::fs::read_to_string(&path)?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

fn save_if_missing<T: Serialize>(folder: &Path, name: &str, value: &T) -> Result<()> {
    let path = folder.join(format!("{name}.toml"));
    if !path.exists() {
        std::fs::write(&path, toml::to_string_pretty(value)?)?;
    }
    Ok(())
}

/// How a character accelerates, runs and jumps. No per-key defaults: a
/// partial `[movement]` table would otherwise mix in another character's values,
/// so it must be written in full (or left out to keep the original).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Movement {
    pub acceleration_per_tick: f64,
    pub max_speed_per_tick: f64,
    pub jump_force_per_tick: f64,
    /// Above this ground speed the run animation replaces walking.
    pub run_animation_speed_per_tick: f64,
}

/// Movement and terrain collision shared by every character (scr_move_basic, scr_collision_basic).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Physics {
    pub gravity_per_tick: f64,
    pub max_fall_speed_per_tick: f64,
    /// Pressing against the running direction on the ground.
    pub turn_around_deceleration_per_tick: f64,
    /// Pressing against the rolling direction.
    pub roll_brake_per_tick: f64,
    pub roll_friction_per_tick: f64,
    pub min_speed_to_roll: f64,
    /// How strongly slopes pull at a runner and at a roller.
    pub slope_factor_running: f64,
    pub slope_factor_rolling: f64,
    pub air_acceleration_multiplier: f64,
    /// Air drag applies while rising slower than this.
    pub air_drag_below_rise_speed: f64,
    pub air_drag_step: f64,
    pub air_drag_divisor: f64,
    /// Sliding friction while dead also uses the rolling slope factor.
    pub dead_slope_factor: f64,
    pub hurt_air_friction_per_tick: f64,
    pub hurt_gravity_per_tick: f64,
    /// Knockback ends once grounded slower than this.
    pub hurt_recover_speed: f64,
    pub boost_speed_per_tick: f64,
    pub boost_trail_ticks: i32,
    pub shocked_invincibility_seconds: f64,

    // Sensor layout around the player's position (x, y).
    pub feet_below_position: f64,
    pub bottom_sensor_spread: f64,
    pub side_sensor_spread: f64,
    pub top_sensor_spread: f64,
    pub sensor_start_above_position: f64,
    pub head_sensor_above_position: f64,
    pub angle_sensor_distance: f64,
    pub wall_scan_top: f64,
    /// In the air the wall scan reaches down to this minus the horizontal speed.
    pub air_wall_scan_bottom: f64,
    pub landing_scan_reach: f64,
    pub ground_scan_reach: f64,
    pub ground_scan_reach_on_angle_allower: f64,
    pub angle_scan_reach: f64,
    /// A jump-through platform catches only a player whose feet were at least this far above it.
    pub jump_through_min_distance: f64,
    /// Walls closer than this are "touching" even when standing still.
    pub wall_check_min_speed: f64,
    /// Balancing looks for a gap this far to each side of a bottom sensor, this deep.
    pub edge_search_half_width: f64,
    pub edge_search_depth: f64,
    /// Terrain within this radius is checked each tick.
    pub terrain_search_radius: f64,
    /// Falling this far below the room counts as falling into the abyss.
    pub abyss_margin: f64,
    /// Default slowdown of scr_player_slow.
    pub slowdown_percent: f64,
    /// Abilities stay on cooldown at least this long while hiding / under a red ring.
    pub hiding_ability_delay_seconds: f64,
    pub red_ring_ability_delay_ticks: i32,
    /// Animation speeds every character uses unless its own file says otherwise.
    pub animation: SharedAnimation,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct SharedAnimation {
    pub look_speed: f64,
    /// Running and jumping animate at speed / this, but never slower than run_min_speed.
    pub run_speed_divisor: f64,
    pub run_min_speed: f64,
    pub spin_speed_divisor: f64,
    pub demonized_dead_speed: f64,
}

impl Default for SharedAnimation {
    fn default() -> SharedAnimation {
        SharedAnimation { look_speed: 0.2, run_speed_divisor: 20.0, run_min_speed: 0.5, spin_speed_divisor: 5.0, demonized_dead_speed: 0.5 }
    }
}

impl Default for Physics {
    fn default() -> Physics {
        Physics {
            gravity_per_tick: 0.21875,
            max_fall_speed_per_tick: 12.0,
            turn_around_deceleration_per_tick: 0.5,
            roll_brake_per_tick: 0.125,
            roll_friction_per_tick: 0.0234375,
            min_speed_to_roll: 2.0,
            slope_factor_running: 0.125,
            slope_factor_rolling: 0.225,
            air_acceleration_multiplier: 2.0,
            air_drag_below_rise_speed: 4.0,
            air_drag_step: 0.125,
            air_drag_divisor: 256.0,
            dead_slope_factor: 0.225,
            hurt_air_friction_per_tick: 0.06,
            hurt_gravity_per_tick: 0.08,
            hurt_recover_speed: 1.0,
            boost_speed_per_tick: 12.0,
            boost_trail_ticks: 16,
            shocked_invincibility_seconds: 2.0,
            feet_below_position: 18.0,
            bottom_sensor_spread: 7.0,
            side_sensor_spread: 8.0,
            top_sensor_spread: 7.0,
            sensor_start_above_position: 16.0,
            head_sensor_above_position: 18.0,
            angle_sensor_distance: 8.0,
            wall_scan_top: -16.0,
            air_wall_scan_bottom: 15.0,
            landing_scan_reach: 35.0,
            ground_scan_reach: 48.0,
            ground_scan_reach_on_angle_allower: 38.0,
            angle_scan_reach: 66.0,
            jump_through_min_distance: 15.0,
            wall_check_min_speed: 1.0,
            edge_search_half_width: 8.0,
            edge_search_depth: 16.0,
            terrain_search_radius: 80.0,
            abyss_margin: 30.0,
            slowdown_percent: 40.0,
            hiding_ability_delay_seconds: 2.0,
            red_ring_ability_delay_ticks: 2,
            animation: SharedAnimation::default(),
        }
    }
}

/// A rectangle relative to the player position: left, top, right, bottom.
pub type Rect = [f64; 4];

/// scr_move_basic: a survivor who never leaves one screenful of the map is told off
/// for it and slowed down until they move somewhere else (obj_player_warning).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct HidingRules {
    /// Standing inside the same chunk this long brings the warning.
    pub warning_after_seconds: f64,
    /// How often the chunk is looked at.
    pub chunk_check_seconds: f64,
    /// A chunk is one screen, taken around the player.
    pub chunk_width: f64,
    pub chunk_height: f64,
    /// While the warning is up the player keeps being slowed by this much, for this long.
    pub slow_percent: f64,
    pub slow_seconds: f64,
    /// No warning in the last minute, when standing still costs enough by itself.
    pub last_minute_seconds: f64,
}

impl Default for HidingRules {
    fn default() -> HidingRules {
        HidingRules {
            warning_after_seconds: 20.0,
            chunk_check_seconds: 1.0,
            chunk_width: 480.0,
            chunk_height: 270.0,
            slow_percent: 40.0,
            slow_seconds: 5.0,
            last_minute_seconds: 60.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Hazards {
    pub spike_damage: i32,
    pub spike_knockback_x: f64,
    pub spike_knockback_y: f64,
    /// Area checked against obj_damage (top edge follows the vertical speed in the original).
    pub damage_zone_hitbox: Rect,
    pub spike_hitbox: Rect,
    /// Moving spikes hurt only while their frame is strictly between these.
    pub moving_spike_harmful_frames: [f64; 2],
    /// Moving spikes stay down, and then up, this long (states.gameplay.entities_misc.global.spikes.timer).
    pub moving_spike_wait_seconds: f64,
    pub abyss_damage: i32,
    /// Extra stun for EXE and demonized players who fall into the abyss.
    pub abyss_stun_seconds: f64,
    /// Taking a black ring costs this many rings, or with fewer this much health.
    pub black_ring_rings: i32,
    pub black_ring_damage: i32,
    /// A black ring can be taken once it has faded in.
    pub black_ring_fade_in_seconds: f64,
}

impl Default for Hazards {
    fn default() -> Hazards {
        Hazards {
            spike_damage: 1,
            spike_knockback_x: 4.0,
            spike_knockback_y: -6.0,
            damage_zone_hitbox: [-12.0, 0.0, 12.0, 20.0],
            spike_hitbox: [-6.0, 19.0, 6.0, 20.0],
            moving_spike_harmful_frames: [0.0, 5.0],
            moving_spike_wait_seconds: 2.0,
            abyss_damage: 1,
            abyss_stun_seconds: 0.5,
            black_ring_rings: 5,
            black_ring_damage: 1,
            black_ring_fade_in_seconds: 0.5,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct HurtRules {
    pub invincibility_seconds: f64,
    /// Shorter invincibility once the round timer is below `last_minute_seconds`.
    pub last_minute_invincibility_seconds: f64,
    pub last_minute_seconds: f64,
    pub default_knockback_x: f64,
    pub default_knockback_y: f64,
    /// Horizontal push when killed, times the facing direction.
    pub death_push_x: f64,
    /// deadTimer value set on death (the revival countdown starts from it).
    pub death_countdown_start: i32,
    /// Health and invincibility of a survivor teammates revived.
    pub revived_hp: i32,
    pub revived_invincibility_seconds: f64,
    /// Invincibility of a survivor coming back as a demon.
    pub demonized_invincibility_seconds: f64,
    /// Health a heal from a teammate gives, and the most a survivor can have.
    ///
    /// Health is counted in hits, not in the original's percent: a survivor has 5
    /// and an ordinary hit takes 1, which is the same five steps the health bar and
    /// the icons of the bottom row have always drawn (100, 80, 60, 40, 20, 0). Every
    /// damage number in these files is in the same hits.
    pub heal_hp: i32,
    pub max_hp: i32,
}

impl Default for HurtRules {
    fn default() -> HurtRules {
        HurtRules {
            invincibility_seconds: 3.0,
            last_minute_invincibility_seconds: 1.0,
            last_minute_seconds: 60.0,
            default_knockback_x: 4.0,
            default_knockback_y: -6.0,
            death_push_x: 2.0,
            death_countdown_start: 31,
            revived_hp: 2,
            revived_invincibility_seconds: 4.0,
            demonized_invincibility_seconds: 2.0,
            heal_hp: 1,
            max_hp: 5,
        }
    }
}

/// Players touching players (obj_player_puppet Step): EXE and demonized players
/// hitting survivors, survivors stunning them, and two attacks meeting.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct ContactRules {
    /// A hit by Sonic.EXE, Exetior or Exeller.
    pub exe_damage: i32,
    /// A hit by Chaos or a demonized survivor.
    pub damage: i32,
    /// A hit throws the survivor this fast in the attacker's facing direction, and up.
    pub hit_knockback_x: f64,
    pub hit_knockback_y: f64,
    /// A demonized Eggman's hit also slows the survivor down for this long.
    pub eggman_hit_slow_seconds: f64,
    /// A survivor's attack stuns EXE or a demonized player for this long; Eggman's shock is shorter.
    pub stun_seconds: f64,
    pub eggman_stun_seconds: f64,
    /// A stunned player is pushed away from the attacker this fast (not by Eggman's shock), and up.
    pub stun_push_x: f64,
    pub stun_push_y: f64,
    /// Two attacks meeting: the one hit bounces back this fast from a demonized
    /// attacker or from a survivor, and up.
    pub clash_push_from_demon_x: f64,
    pub clash_push_from_survivor_x: f64,
    pub clash_push_y: f64,
    /// Sally's shield taking a hit pushes her back this fast, and up.
    pub shield_break_push_x: f64,
    pub shield_break_push_y: f64,
    /// Invincibility after two attacks met or a shield broke.
    pub bounce_invincibility_ticks: i32,
}

impl Default for ContactRules {
    fn default() -> ContactRules {
        ContactRules {
            exe_damage: 2,
            damage: 1,
            hit_knockback_x: 3.0,
            hit_knockback_y: -3.0,
            eggman_hit_slow_seconds: 3.0,
            stun_seconds: 3.0,
            eggman_stun_seconds: 2.0,
            stun_push_x: 4.0,
            stun_push_y: -2.0,
            clash_push_from_demon_x: 3.0,
            clash_push_from_survivor_x: 2.0,
            clash_push_y: -2.0,
            shield_break_push_x: 2.0,
            shield_break_push_y: -2.0,
            bounce_invincibility_ticks: 60,
        }
    }
}

/// Rings the round rules put on the map (obj_ring, obj_redring, SERVER_RING_COLLECTED).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct RingRules {
    /// Rings appear on the map at all (states.gameplay.entities_misc.global.rings.enabled
    /// of the original server).
    pub enabled: bool,
    /// One ring appears every so many seconds, on a ring spawner that has none. The
    /// round sets the seconds from the map and how many are playing; this is what
    /// Singleplayer uses.
    pub spawn_every_seconds: f64,
    /// A new ring is red this often, in percent.
    pub red_chance_percent: i32,
    /// A ring can be taken once it has faded in: alpha grows by this every tick.
    pub fade_in_per_tick: f64,
    pub red_fade_in_per_tick: f64,
    /// A red ring blocks abilities for this long.
    pub red_ring_ticks: i32,
    /// Every ring taken under a red ring shortens it by this, more in the round's
    /// last minute, but it lasts at least `red_ring_min_ticks`.
    pub red_ring_shortening_ticks: f64,
    pub last_minute_red_ring_shortening_ticks: f64,
    pub red_ring_min_ticks: i32,
    /// This many rings heal a hurt survivor (by HurtRules.heal_hp) and are spent.
    pub rings_to_heal: i32,
    /// Healing a teammate (scr_survivor_heal) costs this many rings; the gauge fills by
    /// this much a tick while looking up at them, and sparkles every so many percent.
    pub teammate_heal_rings: i32,
    pub teammate_heal_per_tick: f64,
    pub teammate_heal_sparkle_every_percent: i64,
}

impl Default for RingRules {
    fn default() -> RingRules {
        RingRules {
            enabled: true,
            spawn_every_seconds: 5.0,
            red_chance_percent: 11,
            fade_in_per_tick: 1.0 / 30.0,
            red_fade_in_per_tick: 1.0 / 180.0,
            red_ring_ticks: 600,
            red_ring_shortening_ticks: 30.0,
            last_minute_red_ring_shortening_ticks: 120.0,
            red_ring_min_ticks: 60,
            rings_to_heal: 10,
            teammate_heal_rings: 10,
            teammate_heal_per_tick: 0.016,
            teammate_heal_sparkle_every_percent: 32,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Launch {
    pub x: f64,
    pub y: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Springs {
    pub spring_up: Launch,
    pub spring_left: Launch,
    pub spring_right: Launch,
    pub big_spring_up: Launch,
    pub big_spring_left: Launch,
    pub big_spring_right: Launch,
    pub yellow_spring_up: Launch,
    /// Haunting Dream springs throw higher while jump or up is held.
    pub dream_spring: f64,
    pub dream_spring_held: f64,
    pub frame_reset_seconds: f64,
    pub recharge_seconds: f64,
}

impl Default for Springs {
    fn default() -> Springs {
        Springs {
            spring_up: Launch { x: 0.0, y: -12.0 },
            spring_left: Launch { x: -12.0, y: -12.0 },
            spring_right: Launch { x: 12.0, y: -12.0 },
            big_spring_up: Launch { x: 0.0, y: -10.0 },
            big_spring_left: Launch { x: -10.0, y: -10.0 },
            big_spring_right: Launch { x: 10.0, y: -10.0 },
            yellow_spring_up: Launch { x: 0.0, y: -11.0 },
            dream_spring: -10.0,
            dream_spring_held: -12.0,
            frame_reset_seconds: 0.5,
            recharge_seconds: 4.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Tails {
    pub movement: Movement,
    pub attack_recharge_seconds: f64,
    pub demonized_attack_recharge_seconds: f64,
    /// Ground time needed before flying again.
    pub flight_recharge_seconds: f64,
    pub flight_duration_seconds: f64,
    pub flight_kick_per_tick: f64,
    pub flight_gravity_per_tick: f64,
    pub flight_max_fall_per_tick: f64,
    /// A flight sound plays every this many ticks.
    pub flight_sound_every_ticks: i32,
    pub max_charge_seconds: f64,
    pub medium_charge_seconds: f64,
    pub strong_charge_seconds: f64,
    /// Survivor shot: stun seconds = 1 + charge ticks / this.
    pub stun_charge_ticks_per_second: i32,
    pub demonized_damage: [i32; 3],
    pub shot_pose_seconds: [f64; 3],
    pub shot_recoil: [f64; 3],
    pub recoil_friction_multiplier: f64,
    pub projectile: TailsProjectile,
    pub animation: TailsAnimation,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct TailsProjectile {
    pub speed_per_tick: f64,
    pub lifetime_seconds: f64,
    pub push_x: f64,
    pub survivor_hit_push_y: f64,
    pub exe_hit_push_y: f64,
    pub image_speed: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct TailsAnimation {
    pub idle_loop_frame: f64,
    pub fly_speed: f64,
}

impl Default for Tails {
    fn default() -> Tails {
        Tails {
            movement: Movement {
                acceleration_per_tick: 0.046875,
                max_speed_per_tick: 11.0,
                jump_force_per_tick: 7.0,
                run_animation_speed_per_tick: 8.0,
            },
            attack_recharge_seconds: 23.0,
            demonized_attack_recharge_seconds: 12.0,
            flight_recharge_seconds: 7.0,
            flight_duration_seconds: 160.0 / 60.0,
            flight_kick_per_tick: -1.0,
            flight_gravity_per_tick: 0.035,
            flight_max_fall_per_tick: 2.0,
            flight_sound_every_ticks: 18,
            max_charge_seconds: 3.5,
            medium_charge_seconds: 1.4,
            strong_charge_seconds: 2.1,
            stun_charge_ticks_per_second: 42,
            demonized_damage: [1, 2, 3],
            shot_pose_seconds: [0.4, 0.5, 0.6],
            shot_recoil: [2.0, 4.0, 6.0],
            recoil_friction_multiplier: 3.0,
            projectile: TailsProjectile::default(),
            animation: TailsAnimation::default(),
        }
    }
}

impl Default for TailsProjectile {
    fn default() -> TailsProjectile {
        TailsProjectile {
            speed_per_tick: 14.0,
            lifetime_seconds: 5.0,
            push_x: 4.0,
            survivor_hit_push_y: 2.0,
            exe_hit_push_y: -2.0,
            image_speed: 1.2,
        }
    }
}

impl Default for TailsAnimation {
    fn default() -> TailsAnimation {
        TailsAnimation { idle_loop_frame: 70.0, fly_speed: 2.0 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Knuckles {
    pub movement: Movement,
    pub attack_recharge_seconds: f64,
    pub demonized_attack_recharge_seconds: f64,
    pub glide_recharge_seconds: f64,
    pub max_glide_seconds: f64,
    pub glide_speed_per_tick: f64,
    pub glide_fall_per_tick: f64,
    pub glide_turn_multiplier: f64,
    pub punch_dash_ground_per_tick: f64,
    pub punch_dash_air_per_tick: f64,
    pub animation: KnucklesAnimation,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct KnucklesAnimation {
    /// Idle and balancing loop over this many last frames.
    pub idle_loop_last_frames: f64,
    pub walk_base_speed: f64,
    pub look_speed: f64,
    /// The punch lasts as long as its animation, so these set its duration.
    pub ground_punch_speed: f64,
    pub air_punch_speed: f64,
}

impl Default for Knuckles {
    fn default() -> Knuckles {
        Knuckles {
            movement: Movement {
                acceleration_per_tick: 0.046835,
                max_speed_per_tick: 11.0,
                jump_force_per_tick: 6.5,
                run_animation_speed_per_tick: 8.0,
            },
            attack_recharge_seconds: 20.0,
            demonized_attack_recharge_seconds: 5.0,
            glide_recharge_seconds: 15.0,
            max_glide_seconds: 5.0,
            glide_speed_per_tick: 5.0,
            glide_fall_per_tick: 1.0,
            glide_turn_multiplier: 2.0,
            punch_dash_ground_per_tick: 0.5,
            punch_dash_air_per_tick: 2.0,
            animation: KnucklesAnimation::default(),
        }
    }
}

impl Default for KnucklesAnimation {
    fn default() -> KnucklesAnimation {
        KnucklesAnimation { idle_loop_last_frames: 4.0, walk_base_speed: 0.2, look_speed: 1.0, ground_punch_speed: 0.1, air_punch_speed: 1.5 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Eggman {
    pub movement: Movement,
    pub double_jump_recharge_seconds: f64,
    pub double_jump_speed_per_tick: f64,
    pub jetpack_pose_seconds: f64,
    pub shield_duration_seconds: f64,
    pub shield_recharge_seconds: f64,
    pub demonized_shield_recharge_seconds: f64,
    pub tracker_recharge_seconds: f64,
    /// The floor must reach this far in front of Eggman to place a tracker.
    pub tracker_floor_check_ahead: f64,
    pub tracker: Tracker,
    pub animation: EggmanAnimation,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Tracker {
    pub slow_seconds: f64,
    /// Everyone else is slowed by Physics.slowdown_percent (scr_player_slow).
    pub exe_slow_percent: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct EggmanAnimation {
    pub idle_loop_last_frames: f64,
    pub hurt_speed: f64,
    pub walk_base_speed: f64,
    pub look_speed: f64,
    /// A jetpack puff appears when the floored y is divisible by this.
    pub jetpack_puff_every_pixels: i64,
}

impl Default for Eggman {
    fn default() -> Eggman {
        Eggman {
            movement: Movement {
                acceleration_per_tick: 0.045835,
                max_speed_per_tick: 9.0,
                jump_force_per_tick: 6.2,
                run_animation_speed_per_tick: 6.0,
            },
            double_jump_recharge_seconds: 10.0,
            double_jump_speed_per_tick: -8.0,
            jetpack_pose_seconds: 0.5,
            shield_duration_seconds: 1.5,
            shield_recharge_seconds: 20.0,
            demonized_shield_recharge_seconds: 7.0,
            tracker_recharge_seconds: 30.0,
            tracker_floor_check_ahead: 12.0,
            tracker: Tracker::default(),
            animation: EggmanAnimation::default(),
        }
    }
}

impl Default for Tracker {
    fn default() -> Tracker {
        Tracker { slow_seconds: 3.0, exe_slow_percent: 30.0 }
    }
}

impl Default for EggmanAnimation {
    fn default() -> EggmanAnimation {
        EggmanAnimation { idle_loop_last_frames: 11.0, hurt_speed: 0.4, walk_base_speed: 0.2, look_speed: 1.0, jetpack_puff_every_pixels: 10 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Amy {
    pub movement: Movement,
    pub attack_recharge_seconds: f64,
    pub demonized_attack_recharge_seconds: f64,
    pub big_jump_recharge_seconds: f64,
    pub big_jump_force_per_tick: f64,
    /// Hearts shown by the hammer swing: how many, where the first one starts, and the gap.
    pub hearts: i32,
    pub heart_start_x: f64,
    pub heart_start_y: f64,
    pub heart_spacing: f64,
    pub animation: AmyAnimation,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct AmyAnimation {
    pub idle_loop_last_frames: f64,
    pub walk_base_speed: f64,
    pub look_speed: f64,
    /// The jump pose plays once at this speed.
    pub jump_speed: f64,
    /// The hammer attack and the big jump last as long as their animations.
    pub attack_speed: f64,
    pub big_jump_speed: f64,
}

impl Default for Amy {
    fn default() -> Amy {
        Amy {
            movement: Movement {
                acceleration_per_tick: 0.046875,
                max_speed_per_tick: 11.0,
                jump_force_per_tick: 7.0,
                run_animation_speed_per_tick: 8.0,
            },
            attack_recharge_seconds: 20.0,
            demonized_attack_recharge_seconds: 5.0,
            big_jump_recharge_seconds: 9.0,
            big_jump_force_per_tick: 10.0,
            hearts: 4,
            heart_start_x: 2.0,
            heart_start_y: -4.0,
            heart_spacing: 10.0,
            animation: AmyAnimation::default(),
        }
    }
}

impl Default for AmyAnimation {
    fn default() -> AmyAnimation {
        AmyAnimation { idle_loop_last_frames: 4.0, walk_base_speed: 0.2, look_speed: 1.0, jump_speed: 1.1, attack_speed: 0.3, big_jump_speed: 0.3 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Cream {
    pub movement: Movement,
    pub fly_recharge_seconds: f64,
    pub max_fly_seconds: f64,
    pub fly_fall_per_tick: f64,
    pub dash_recharge_seconds: f64,
    pub dash_seconds: f64,
    pub dash_max_speed_per_tick: f64,
    pub dash_acceleration_per_tick: f64,
    pub rings_recharge_seconds: f64,
    pub demonized_rings_recharge_seconds: f64,
    /// How long Cream stands still before the rings appear.
    pub rings_spawn_ticks: i32,
    /// Where the rings appear relative to Cream, and the free space they need.
    pub rings_spot_x: f64,
    pub rings_spot_y: f64,
    /// Extra x shift of the spot when Cream faces left.
    pub rings_spot_flip_shift: f64,
    pub rings_spot_radius: f64,
    /// Demonized Cream cannot place rings this close to a red ring (distance between
    /// the two bounding boxes, as distance_to_object measures it).
    pub demonized_red_ring_distance: f64,
    /// Cream's own slowdown, applied every tick while slowed (see cream.rs).
    pub slow_percent: f64,
    pub animation: CreamAnimation,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct CreamAnimation {
    pub idle_loop_last_frames: f64,
    pub walk_base_speed: f64,
    pub look_speed: f64,
    pub fly_speed: f64,
    pub spawn_speed: f64,
}

impl Default for Cream {
    fn default() -> Cream {
        Cream {
            movement: Movement {
                acceleration_per_tick: 0.046875,
                max_speed_per_tick: 11.0,
                jump_force_per_tick: 7.0,
                run_animation_speed_per_tick: 6.0,
            },
            fly_recharge_seconds: 15.0,
            max_fly_seconds: 2.0,
            fly_fall_per_tick: 1.0,
            dash_recharge_seconds: 30.0,
            dash_seconds: 0.5,
            dash_max_speed_per_tick: 13.0,
            dash_acceleration_per_tick: 0.8,
            rings_recharge_seconds: 40.0,
            demonized_rings_recharge_seconds: 30.0,
            rings_spawn_ticks: 60,
            rings_spot_x: -12.0,
            rings_spot_y: -16.0,
            rings_spot_flip_shift: 7.0,
            rings_spot_radius: 32.0,
            demonized_red_ring_distance: 130.0,
            slow_percent: 30.0,
            animation: CreamAnimation::default(),
        }
    }
}

impl Default for CreamAnimation {
    fn default() -> CreamAnimation {
        CreamAnimation { idle_loop_last_frames: 24.0, walk_base_speed: 0.2, look_speed: 1.0, fly_speed: 0.3, spawn_speed: 0.5 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Sally {
    pub movement: Movement,
    pub attack_recharge_seconds: f64,
    pub demonized_attack_recharge_seconds: f64,
    pub shield_seconds: f64,
    pub shield_recharge_seconds: f64,
    pub demonized_shield_recharge_seconds: f64,
    /// Invincibility after the shield absorbs a hit.
    pub shield_block_invincibility_seconds: f64,
    pub slide_min_speed_per_tick: f64,
    pub slide_boost_per_tick: f64,
    pub slide_friction_multiplier: f64,
    /// The slide stops this close to the left or right room edge.
    pub slide_room_edge_margin: f64,
    pub slide_dust_every_pixels: i64,
    pub slide_dust_offset_x: f64,
    pub slide_dust_offset_y: f64,
    pub animation: SallyAnimation,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct SallyAnimation {
    pub idle_loop_last_frames: f64,
    pub hurt_speed: f64,
    pub walk_base_speed: f64,
    pub look_speed: f64,
    pub jump_fall_speed: f64,
    /// The kick lasts as long as its animation.
    pub attack_speed: f64,
}

impl Default for Sally {
    fn default() -> Sally {
        Sally {
            movement: Movement {
                acceleration_per_tick: 0.046875,
                max_speed_per_tick: 11.0,
                jump_force_per_tick: 6.5,
                run_animation_speed_per_tick: 6.0,
            },
            attack_recharge_seconds: 15.0,
            demonized_attack_recharge_seconds: 4.0,
            shield_seconds: 5.0,
            shield_recharge_seconds: 30.0,
            demonized_shield_recharge_seconds: 15.0,
            shield_block_invincibility_seconds: 1.0,
            slide_min_speed_per_tick: 2.5,
            slide_boost_per_tick: 2.0,
            slide_friction_multiplier: 2.5,
            slide_room_edge_margin: 1.0,
            slide_dust_every_pixels: 5,
            slide_dust_offset_x: 8.0,
            slide_dust_offset_y: 20.0,
            animation: SallyAnimation::default(),
        }
    }
}

impl Default for SallyAnimation {
    fn default() -> SallyAnimation {
        SallyAnimation { idle_loop_last_frames: 14.0, hurt_speed: 0.4, walk_base_speed: 0.2, look_speed: 1.0, jump_fall_speed: 1.0, attack_speed: 1.0 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct ExeOriginal {
    pub movement: Movement,
    pub invisibility_seconds: f64,
    pub invisibility_recharge_seconds: f64,
    pub ground_attack_recharge_seconds: f64,
    pub air_attack_recharge_seconds: f64,
    /// Falling speed cap while dashing in the air.
    pub air_attack_fall_per_tick: f64,
    /// Part of the speed kept when the dash animation ends.
    pub dash_end_speed_keep: f64,
    pub dash_dust_every_pixels: i64,
    pub dash_dust_offset_x: f64,
    pub dash_dust_offset_y: f64,
    pub animation: KillerAnimation,
}

/// Animation speeds Sonic.EXE and Exetior share by design (their Draw_76 is the same).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct KillerAnimation {
    pub idle_loop_last_frames: f64,
    pub fall_speed: f64,
    /// The dashes last as long as their animations.
    pub attack_speed: f64,
    pub air_attack_speed: f64,
    pub shocked_speed: f64,
    /// Win and loss poses.
    pub end_pose_speed: f64,
    pub emotion_speed: f64,
}

impl Default for ExeOriginal {
    fn default() -> ExeOriginal {
        ExeOriginal {
            movement: Movement {
                acceleration_per_tick: 0.056875,
                max_speed_per_tick: 12.0,
                jump_force_per_tick: 7.0,
                run_animation_speed_per_tick: 8.0,
            },
            invisibility_seconds: 15.0,
            invisibility_recharge_seconds: 20.0,
            ground_attack_recharge_seconds: 1.0,
            air_attack_recharge_seconds: 2.0,
            air_attack_fall_per_tick: 2.5,
            dash_end_speed_keep: 0.7,
            dash_dust_every_pixels: 4,
            dash_dust_offset_x: 8.0,
            dash_dust_offset_y: 20.0,
            animation: KillerAnimation::default(),
        }
    }
}

impl Default for KillerAnimation {
    fn default() -> KillerAnimation {
        KillerAnimation {
            idle_loop_last_frames: 25.0,
            fall_speed: 0.1,
            attack_speed: 2.0,
            air_attack_speed: 0.8,
            shocked_speed: 0.5,
            end_pose_speed: 0.8,
            emotion_speed: 0.8,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Chaos {
    pub movement: Movement,
    pub ground_attack_recharge_seconds: f64,
    pub air_attack_recharge_seconds: f64,
    pub air_attack_fall_per_tick: f64,
    pub attack_end_speed_keep: f64,
    /// The aimed air dash: speed along each axis, how long it flies, and the speed left after it.
    pub air_dash_speed_x_per_tick: f64,
    pub air_dash_speed_y_per_tick: f64,
    pub air_dash_ticks: i32,
    pub air_dash_end_speed_per_tick: f64,
    /// A dash sticks to a surface only this soon after it started.
    pub dash_wall_stick_window_ticks: i32,
    pub dash_trail_ticks: i32,
    pub liquid_seconds: f64,
    pub liquid_recharge_seconds: f64,
    pub liquid_acceleration_multiplier: f64,
    pub liquid_max_speed_multiplier: f64,
    /// The liquid form cannot be cancelled during its first ticks.
    pub liquid_cancel_after_ticks: i32,
    pub liquid_air_trail_ticks: i32,
    pub stuck_ticks: i32,
    /// Dashing off a surface works only while the stuck timer is at least this.
    pub stuck_escape_window_ticks: i32,
    pub stuck_friction_multiplier: f64,
    pub stuck_slope_factor: f64,
    pub stuck_dash_ticks: i32,
    pub stuck_dash_ground_speed_per_tick: f64,
    /// Upward speed factor when dashing off a wall.
    pub wall_jump_speed_y: f64,
    pub dust_every_pixels: i64,
    pub dust_offset_x: f64,
    pub dust_offset_y: f64,
    pub animation: ChaosAnimation,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct ChaosAnimation {
    pub idle_loop_last_frames: f64,
    pub walk_multiplier: f64,
    pub liquid_walk_multiplier: f64,
    pub run_multiplier: f64,
    pub liquid_run_speed: f64,
    pub liquid_jump_speed: f64,
    /// The ground attack lasts as long as its animation.
    pub attack_speed: f64,
    pub air_attack_speed: f64,
    pub shocked_speed: f64,
    pub lost2_loop_last_frames: f64,
    /// Frame shown while stuck once dashing off is no longer possible.
    pub stuck_frame: f64,
}

impl Default for Chaos {
    fn default() -> Chaos {
        Chaos {
            movement: Movement {
                acceleration_per_tick: 0.056875,
                max_speed_per_tick: 12.0,
                jump_force_per_tick: 7.0,
                run_animation_speed_per_tick: 8.0,
            },
            ground_attack_recharge_seconds: 1.5,
            air_attack_recharge_seconds: 2.5,
            air_attack_fall_per_tick: 2.5,
            attack_end_speed_keep: 0.7,
            air_dash_speed_x_per_tick: 12.0,
            air_dash_speed_y_per_tick: 8.0,
            air_dash_ticks: 15,
            air_dash_end_speed_per_tick: 2.0,
            dash_wall_stick_window_ticks: 24,
            dash_trail_ticks: 20,
            liquid_seconds: 15.0,
            liquid_recharge_seconds: 20.0,
            liquid_acceleration_multiplier: 1.5,
            liquid_max_speed_multiplier: 1.2,
            liquid_cancel_after_ticks: 10,
            liquid_air_trail_ticks: 6,
            stuck_ticks: 30,
            stuck_escape_window_ticks: 15,
            stuck_friction_multiplier: 8.0,
            stuck_slope_factor: 0.225,
            stuck_dash_ticks: 8,
            stuck_dash_ground_speed_per_tick: 12.0,
            wall_jump_speed_y: -1.4,
            dust_every_pixels: 4,
            dust_offset_x: 8.0,
            dust_offset_y: 20.0,
            animation: ChaosAnimation::default(),
        }
    }
}

impl Default for ChaosAnimation {
    fn default() -> ChaosAnimation {
        ChaosAnimation {
            idle_loop_last_frames: 4.0,
            walk_multiplier: 1.8,
            liquid_walk_multiplier: 2.5,
            run_multiplier: 1.2,
            liquid_run_speed: 2.6,
            liquid_jump_speed: 1.3,
            attack_speed: 2.0,
            air_attack_speed: 0.8,
            shocked_speed: 0.5,
            lost2_loop_last_frames: 5.0,
            stuck_frame: 2.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Exetior {
    pub movement: Movement,
    pub ground_attack_recharge_seconds: f64,
    pub air_attack_recharge_seconds: f64,
    /// Held as the attack cooldown for the whole stomp.
    pub stomp_cooldown_seconds: f64,
    pub air_attack_fall_per_tick: f64,
    pub dash_end_speed_keep: f64,
    pub stomp_fall_speed_per_tick: f64,
    pub stomp_trail_ticks: i32,
    pub black_ring_recharge_seconds: f64,
    /// Where the black ring appears relative to Exetior.
    pub black_ring_offset_x: f64,
    pub black_ring_offset_y: f64,
    /// No black ring can be placed this near another.
    pub black_ring_spacing: f64,
    pub dust_every_pixels: i64,
    pub dust_offset_x: f64,
    pub dust_offset_y: f64,
    pub stomp_animation_speed: f64,
    /// The stomp landing lasts as long as its animation.
    pub stomp_land_animation_speed: f64,
    pub stomp_waves: StompWaves,
    pub animation: KillerAnimation,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct StompWaves {
    /// The first waves appear this far to each side of the landing spot, at this height below it.
    pub first_offset_x: f64,
    pub offset_y: f64,
    /// Each wave spawns the next one this much further out when its animation ends.
    pub step_x: f64,
    /// Waves beyond this generation vanish at once (the original allows 1).
    pub max_generation: i32,
    /// The original removes a wave when its sound (snd_exetior_shockwave, 1.32 s) ends.
    pub lifetime_seconds: f64,
    pub damage: i32,
    pub push_x: f64,
}

impl Default for Exetior {
    fn default() -> Exetior {
        Exetior {
            movement: Movement {
                acceleration_per_tick: 0.051875,
                max_speed_per_tick: 11.0,
                jump_force_per_tick: 7.0,
                run_animation_speed_per_tick: 8.0,
            },
            ground_attack_recharge_seconds: 1.0,
            air_attack_recharge_seconds: 3.0,
            stomp_cooldown_seconds: 4.0,
            air_attack_fall_per_tick: 2.5,
            dash_end_speed_keep: 0.7,
            stomp_fall_speed_per_tick: 12.0,
            stomp_trail_ticks: 6,
            black_ring_recharge_seconds: 42.0,
            black_ring_offset_x: -15.0,
            black_ring_offset_y: -15.0,
            black_ring_spacing: 100.0,
            dust_every_pixels: 4,
            dust_offset_x: 8.0,
            dust_offset_y: 20.0,
            stomp_animation_speed: 1.0,
            stomp_land_animation_speed: 1.0,
            stomp_waves: StompWaves::default(),
            animation: KillerAnimation { idle_loop_last_frames: 14.0, ..KillerAnimation::default() },
        }
    }
}

impl Default for StompWaves {
    fn default() -> StompWaves {
        StompWaves { first_offset_x: 25.0, offset_y: 19.0, step_x: 25.0, max_generation: 1, lifetime_seconds: 1.32, damage: 1, push_x: 3.0 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Exeller {
    pub movement: Movement,
    pub ground_attack_recharge_seconds: f64,
    pub air_attack_recharge_seconds: f64,
    pub air_attack_fall_per_tick: f64,
    pub dash_end_speed_keep: f64,
    pub dust_every_pixels: i64,
    pub dust_offset_x: f64,
    pub dust_offset_y: f64,
    pub max_clones: i32,
    /// Pause between placing clones.
    pub clone_place_cooldown_seconds: f64,
    /// Recharge after teleporting to a clone.
    pub clone_recharge_seconds: f64,
    /// A clone shows its owner the nearest survivor closer than this.
    pub clone_reveal_distance: f64,
    /// The clone looks for floor this far below itself to pick the standing pose.
    pub clone_floor_check_below: f64,
    /// On Not Perfect a clone rises while this rectangle (relative to it) touches the floor.
    pub clone_floor_push_rect: Rect,
    pub clone_effect_offset_y: f64,
    pub animation: KillerAnimation,
}

impl Default for Exeller {
    fn default() -> Exeller {
        Exeller {
            movement: Movement {
                acceleration_per_tick: 0.056875,
                max_speed_per_tick: 12.0,
                jump_force_per_tick: 7.0,
                run_animation_speed_per_tick: 8.0,
            },
            ground_attack_recharge_seconds: 1.0,
            air_attack_recharge_seconds: 2.0,
            air_attack_fall_per_tick: 2.5,
            dash_end_speed_keep: 0.7,
            dust_every_pixels: 4,
            dust_offset_x: 8.0,
            dust_offset_y: 20.0,
            max_clones: 2,
            clone_place_cooldown_seconds: 3.0,
            clone_recharge_seconds: 30.0,
            clone_reveal_distance: 240.0,
            clone_floor_check_below: 18.0,
            clone_floor_push_rect: [-7.0, -20.0, 7.0, 18.0],
            clone_effect_offset_y: 32.0,
            animation: KillerAnimation { idle_loop_last_frames: 48.0, fall_speed: 1.0, ..KillerAnimation::default() },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_files_parse_back_to_the_same_values() {
        let config = GameplayConfig::default();
        let text = toml::to_string_pretty(&config.eggman).unwrap();
        let parsed: Eggman = toml::from_str(&text).unwrap();
        assert_eq!(parsed, config.eggman);
    }

    #[test]
    fn a_partial_file_keeps_the_original_values_for_missing_keys() {
        let parsed: Knuckles = toml::from_str("glide_speed_per_tick = 7.0").unwrap();
        assert_eq!(parsed.glide_speed_per_tick, 7.0);
        assert_eq!(parsed.attack_recharge_seconds, 20.0);
    }

    #[test]
    fn a_partial_movement_table_is_an_error_not_a_silent_mix() {
        let result: Result<Knuckles, _> = toml::from_str("[movement]\njump_force_per_tick = 8.0");
        assert!(result.is_err());
    }

    #[test]
    fn seconds_become_whole_ticks() {
        assert_eq!(ticks(0.4), 24);
        assert_eq!(ticks(160.0 / 60.0), 160);
        assert_eq!(ticks(1.5), 90);
    }
}
