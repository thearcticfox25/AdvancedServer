use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn cfg() -> &'static Config {
    CONFIG.get().expect("Config not initialized")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server_config: ServerConfig,
    pub states: StatesConfig,
    pub miscellaneous: MiscellaneousConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub networking: Networking,
    pub pairing: Pairing,
    pub logging: Logging,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Networking {
    pub port: u16,
    pub server_count: u16,
    pub vinny: Vinny,
}

/// Per-tick flood cap, raw ENet events/packets. Lives under `networking`
/// since it applies before any game-state processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Vinny {
    pub max_events_per_tick: u32,
    pub max_packets_per_peer_per_tick: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Pairing {
    pub maximum_players_per_lobby: u8,
    pub ip_validation: bool,
    pub ping_limit: u16,
    pub player_maximum_errors: u16,
    pub rate_limit_window: u16,
    pub kick_timeout_window: u16,
    pub versioning: Versioning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Versioning {
    pub target_version: u16,
    pub disable_version_validating: bool,
}

/// 0: startup banner + basic init report + terminal command replies only
///    (those always show -- an admin needs to see their own command run).
/// 1: level 0 + ERROR logs.
/// 2: level 1 + WARN logs.
/// 3 (default, matches the C version's default verbosity): level 2 + INFO logs.
/// 4: level 3 + DEBUG logs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Logging {
    pub log_level: u8,
    pub log_to_file: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StatesConfig {
    pub lobby_misc: LobbyMisc,
    pub map_selection: MapSelection,
    pub character_selection: CharacterSelection,
    pub gameplay: Gameplay,
    pub results_misc: ResultsMisc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LobbyMisc {
    pub upper_bracket: String,
    pub server_location: String,
    pub hosts_name: String,
    pub lower_bracket: String,
    pub message_of_the_day: String,
    pub lobby_timeout_timer: u8,
    pub lobby_start_timer: u8,
    pub authoritarian_mode: bool,
    pub lobby_ready_required_percentage: u8,
    pub kick_unready_before_starting: bool,
    pub anonymous_mode: bool,
    pub min_players_required: u8,
    pub moderation: Moderation,
    pub votekick: Votekick,
    pub chat_rate_limit: ChatRateLimit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Moderation {
    pub ban_ip: bool,
    pub ban_udid: bool,
    pub ban_nickname: bool,
    pub op_default_level: u8,
    pub enforce_whitelist: bool,
    pub shy_mode: bool,
    pub aggressive_username_ban: bool,
}

/// Per-peer chat anti-flood (token bucket). A peer may send up to `burst`
/// messages back-to-back, then is limited to `messages_per_second` sustained.
/// Operators at or above `exempt_op_level` bypass the limit entirely.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ChatRateLimit {
    pub enable: bool,
    pub burst: f64,
    pub messages_per_second: f64,
    pub exempt_op_level: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Votekick {
    pub cooldown: u8,
    pub autoban_leavers: bool,
    pub vote_result_delay: u8,
    pub use_legacy_voting: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MapSelection {
    pub enabled: bool,
    pub timer: u8,
    pub map_list: Vec<bool>,
    pub exclude_last_map: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CharacterSelection {
    pub enable: bool,
    pub charselect_mod_unlocked: bool,
    pub allow_foreign_characters: bool,
    pub charselect_timer: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Gameplay {
    pub respawn_time: u8,
    pub sudden_death_timer: u8,
    pub ring_appearance_timer: u8,
    pub escape_time: u8,
    pub demonization_percentage: u8,
    pub exe_camp_penalty: bool,
    pub hide_player_characters: bool,
    pub enable_achievements: bool,
    pub enable_sounds: bool,
    pub gametimers_ceiling: u16,
    /// Caps a dead player's displayed/effective death_timer_sec to the time
    /// actually remaining until sudden death, and forces it to keep counting
    /// down through exe-camp freeze once sudden death is closer than the
    /// respawn timer -- without this, the shown countdown can promise a
    /// respawn later than sudden death will actually kill everyone still
    /// down, or freeze indefinitely near the exe with sudden death still
    /// bearing down regardless. Correctly inverts for banana.disable_timer,
    /// where time_sec counts up instead of down.
    pub match_respawn_and_game_timers: bool,
    pub ending_timer: u8,
    pub waiting_timeout: u8,
    pub entities_misc: EntitiesMisc,
    pub anticheat: Anticheat,
    pub banana: Banana,
    pub gmcycle: GMCycle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EntitiesMisc {
    pub global: GlobalEntities,
    pub map_specific: MapSpecific,
    pub character_specific: CharacterSpecific,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GlobalEntities {
    pub rings: RingsConfig,
    pub spikes: SpikesConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RingsConfig {
    pub enabled: bool,
    pub red_ring_chance: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SpikesConfig {
    pub timer: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MapSpecific {
    pub ravine_mist: RavineMistCfg,
    pub you_cant_run: YouCantRunCfg,
    #[serde(alias = "limp_city")]
    pub limb_city: LimbCityCfg,
    pub not_perfect: NotPerfectCfg,
    pub kind_and_fair: KindAndFairCfg,
    pub act9: Act9Cfg,
    pub nasty_paradise: NastyParadiseCfg,
    pub priceless_freedom: PricelessFreedomCfg,
    pub hills: HillsCfg,
    pub torture_cave: TortureCaveCfg,
    pub dark_tower: DarkTowerCfg,
    pub haunting_dream: HauntingDreamCfg,
    pub fart_zone: FartZoneCfg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RavineMistCfg {
    pub shards: ShardsCfg,
    pub slugs: SlugsCfg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ShardsCfg {
    pub amount: u8,
    pub required_for_exit: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SlugsCfg {
    pub enabled: bool,
    pub ring_chance: u8,
    pub red_ring_chance: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct YouCantRunCfg {
    pub gas: GasCfg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GasCfg {
    pub delay: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LimbCityCfg {
    pub eye: EyeCfg,
    pub chain: ChainCfg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EyeCfg {
    pub recharge_strength: u8,
    pub recharge_timer: u8,
    pub use_cost: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ChainCfg {
    pub delay: u16,
    pub warning: u8,
    pub shocking_time: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NotPerfectCfg {
    pub switch_timer: u8,
    pub switch_timer_chase: u8,
    pub switch_warning_timer: u8,
    pub switch_warning_timer_chase: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KindAndFairCfg {
    pub speedbox: SpeedboxCfg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SpeedboxCfg {
    pub timer: u8,
    pub timer_offset: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Act9Cfg {
    pub walls: WallsCfg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WallsCfg {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NastyParadiseCfg {
    pub snowballs: SnowballsCfg,
    pub ice: IceCfg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SnowballsCfg {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IceCfg {
    pub regeneration_timer: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PricelessFreedomCfg {
    pub black_rings: BlackRingsCfg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BlackRingsCfg {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HillsCfg {
    pub thunder: ThunderCfg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThunderCfg {
    pub timer: u8,
    pub timer_offset: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TortureCaveCfg {
    pub acid: AcidCfg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AcidCfg {
    pub delay: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DarkTowerCfg {
    pub stalactites: StalactitesCfg,
    pub balls: BallsCfg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StalactitesCfg {
    pub timer: u8,
    pub timer_offset: u8,
    pub acceleration: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BallsCfg {
    pub shift_per_tick: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HauntingDreamCfg {
    pub doors: DoorsCfg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DoorsCfg {
    pub toggle_delay: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FartZoneCfg {
    pub dummy: DummyCfg,
    pub black_rings: BlackRingsCfg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DummyCfg {
    pub velocity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CharacterSpecific {
    pub tails: TailsCfg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TailsCfg {
    pub projectile_speed: i8,
    pub projectile_timeout_timer: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Anticheat {
    pub palette_anticheat: bool,
    pub zone_anticheat: bool,
    pub distance_anticheat: bool,
    pub ability_anticheat: bool,
    pub data_based_anticheat: bool,
    pub physics_anticheat: bool,
    pub physics_anticheat_log_only: bool,
    pub physics_correction: bool,
    pub physics_correction_log_only: bool,
    pub useless_anticheat: UselessAnticheat,
    pub zero_trust_anticheat: bool,
    pub zero_trust_log_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UselessAnticheat {
    pub enable: bool,
    pub strict_mode: UselessStrictMode,
}

/// Fill c1/c2 with the real constants to enable strict PC-client verification.
/// Leave as zeros to use the FOSS fallback (checksum-zero detection only).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UselessStrictMode {
    pub c1: [u64; 3],
    pub c2: [u64; 3],
}

impl UselessStrictMode {
    pub fn is_active(&self) -> bool {
        self.c1.iter().all(|&v| v != 0) && self.c2.iter().all(|&v| v != 0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Banana {
    pub disable_timer: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GMCycle {
    pub overhell: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ResultsMisc {
    pub enabled: bool,
    pub timer: u8,
    pub pride: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MiscellaneousConfig {
    pub other: OtherConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OtherConfig {
    pub ignore_inadequate_configuration: bool,
    pub instructor_enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_config: ServerConfig::default(),
            states: StatesConfig::default(),
            miscellaneous: MiscellaneousConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            networking: Networking::default(),
            pairing: Pairing::default(),
            logging: Logging::default(),
        }
    }
}

impl Default for Vinny {
    fn default() -> Self {
        Self {
            max_events_per_tick: 256,
            max_packets_per_peer_per_tick: 16,
        }
    }
}

impl Default for Networking {
    fn default() -> Self {
        Self { port: 8606, server_count: 1, vinny: Vinny::default() }
    }
}

impl Default for Pairing {
    fn default() -> Self {
        Self {
            maximum_players_per_lobby: 7,
            ip_validation: true,
            ping_limit: 250,
            player_maximum_errors: 30000,
            rate_limit_window: 5,
            kick_timeout_window: 60,
            versioning: Versioning::default(),
        }
    }
}

impl Default for Versioning {
    fn default() -> Self {
        Self { target_version: 1101, disable_version_validating: false }
    }
}

impl Default for Logging {
    fn default() -> Self {
        Self { log_level: 3, log_to_file: false }
    }
}

impl Default for StatesConfig {
    fn default() -> Self {
        Self {
            lobby_misc: LobbyMisc::default(),
            map_selection: MapSelection::default(),
            character_selection: CharacterSelection::default(),
            gameplay: Gameplay::default(),
            results_misc: ResultsMisc::default(),
        }
    }
}

impl Default for LobbyMisc {
    fn default() -> Self {
        Self {
            upper_bracket: "-----\\advanced/server~-----".to_string(),
            server_location: "Saint Petersburg".to_string(),
            hosts_name: "The Arctic Fox".to_string(),
            lower_bracket: "------------------------".to_string(),
            message_of_the_day: "\\mods are disallowed on this server".to_string(),
            lobby_timeout_timer: 25,
            lobby_start_timer: 5,
            authoritarian_mode: false,
            lobby_ready_required_percentage: 100,
            kick_unready_before_starting: false,
            anonymous_mode: false,
            min_players_required: 2,
            moderation: Moderation::default(),
            votekick: Votekick::default(),
            chat_rate_limit: ChatRateLimit::default(),
        }
    }
}

impl Default for ChatRateLimit {
    fn default() -> Self {
        Self {
            enable: true,
            burst: 5.0,
            messages_per_second: 0.7,
            exempt_op_level: 2,
        }
    }
}

impl Default for Moderation {
    fn default() -> Self {
        Self {
            ban_ip: true,
            ban_udid: true,
            ban_nickname: false,
            op_default_level: 3,
            enforce_whitelist: false,
            shy_mode: true,
            aggressive_username_ban: false,
        }
    }
}

impl Default for Votekick {
    fn default() -> Self {
        Self { cooldown: 30, autoban_leavers: false, vote_result_delay: 3, use_legacy_voting: false }
    }
}

impl Default for MapSelection {
    fn default() -> Self {
        Self {
            enabled: true,
            timer: 30,
            map_list: vec![
                true, true, true, true, true, true, true, true, true, true,
                true, true, true, true, true, true, true, true, true, true,
                false,
            ],
            exclude_last_map: true,
        }
    }
}

impl Default for CharacterSelection {
    fn default() -> Self {
        Self {
            enable: true,
            charselect_mod_unlocked: false,
            allow_foreign_characters: false,
            charselect_timer: 30,
        }
    }
}

impl Default for Gameplay {
    fn default() -> Self {
        Self {
            respawn_time: 30,
            sudden_death_timer: 120,
            ring_appearance_timer: 60,
            escape_time: 50,
            demonization_percentage: 50,
            exe_camp_penalty: true,
            hide_player_characters: false,
            enable_achievements: true,
            enable_sounds: true,
            gametimers_ceiling: 570,
            match_respawn_and_game_timers: true,
            ending_timer: 5,
            waiting_timeout: 15,
            entities_misc: EntitiesMisc::default(),
            anticheat: Anticheat::default(),
            banana: Banana::default(),
            gmcycle: GMCycle::default(),
        }
    }
}

impl Default for EntitiesMisc {
    fn default() -> Self {
        Self {
            global: GlobalEntities::default(),
            map_specific: MapSpecific::default(),
            character_specific: CharacterSpecific::default(),
        }
    }
}

impl Default for GlobalEntities {
    fn default() -> Self {
        Self { rings: RingsConfig::default(), spikes: SpikesConfig::default() }
    }
}

impl Default for RingsConfig {
    fn default() -> Self {
        Self { enabled: true, red_ring_chance: 11 }
    }
}

impl Default for SpikesConfig {
    fn default() -> Self {
        Self { timer: 2 }
    }
}

impl Default for MapSpecific {
    fn default() -> Self {
        Self {
            ravine_mist: RavineMistCfg::default(),
            you_cant_run: YouCantRunCfg::default(),
            limb_city: LimbCityCfg::default(),
            not_perfect: NotPerfectCfg::default(),
            kind_and_fair: KindAndFairCfg::default(),
            act9: Act9Cfg::default(),
            nasty_paradise: NastyParadiseCfg::default(),
            priceless_freedom: PricelessFreedomCfg::default(),
            hills: HillsCfg::default(),
            torture_cave: TortureCaveCfg::default(),
            dark_tower: DarkTowerCfg::default(),
            haunting_dream: HauntingDreamCfg::default(),
            fart_zone: FartZoneCfg::default(),
        }
    }
}

impl Default for RavineMistCfg {
    fn default() -> Self {
        Self { shards: ShardsCfg::default(), slugs: SlugsCfg::default() }
    }
}

impl Default for ShardsCfg {
    fn default() -> Self {
        Self { amount: 7, required_for_exit: 6 }
    }
}

impl Default for SlugsCfg {
    fn default() -> Self {
        Self { enabled: true, ring_chance: 40, red_ring_chance: 10 }
    }
}

impl Default for YouCantRunCfg {
    fn default() -> Self {
        Self { gas: GasCfg { delay: 6 } }
    }
}

impl Default for GasCfg {
    fn default() -> Self {
        Self { delay: 6 }
    }
}

impl Default for LimbCityCfg {
    fn default() -> Self {
        Self { eye: EyeCfg::default(), chain: ChainCfg::default() }
    }
}

impl Default for EyeCfg {
    fn default() -> Self {
        Self { recharge_strength: 10, recharge_timer: 2, use_cost: 20 }
    }
}

impl Default for ChainCfg {
    fn default() -> Self {
        Self { delay: 8, warning: 2, shocking_time: 2 }
    }
}

impl Default for NotPerfectCfg {
    fn default() -> Self {
        Self {
            switch_timer: 5,
            switch_timer_chase: 3,
            switch_warning_timer: 15,
            switch_warning_timer_chase: 2,
        }
    }
}

impl Default for KindAndFairCfg {
    fn default() -> Self {
        Self { speedbox: SpeedboxCfg { timer: 25, timer_offset: 5 } }
    }
}

impl Default for SpeedboxCfg {
    fn default() -> Self {
        Self { timer: 25, timer_offset: 5 }
    }
}

impl Default for Act9Cfg {
    fn default() -> Self {
        Self { walls: WallsCfg { enabled: true } }
    }
}

impl Default for WallsCfg {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl Default for NastyParadiseCfg {
    fn default() -> Self {
        Self { snowballs: SnowballsCfg { enabled: true }, ice: IceCfg { regeneration_timer: 15 } }
    }
}

impl Default for SnowballsCfg {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl Default for IceCfg {
    fn default() -> Self {
        Self { regeneration_timer: 15 }
    }
}

impl Default for PricelessFreedomCfg {
    fn default() -> Self {
        Self { black_rings: BlackRingsCfg { enabled: true } }
    }
}

impl Default for BlackRingsCfg {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl Default for HillsCfg {
    fn default() -> Self {
        Self { thunder: ThunderCfg { timer: 15, timer_offset: 5 } }
    }
}

impl Default for ThunderCfg {
    fn default() -> Self {
        Self { timer: 15, timer_offset: 5 }
    }
}

impl Default for TortureCaveCfg {
    fn default() -> Self {
        Self { acid: AcidCfg { delay: 4 } }
    }
}

impl Default for AcidCfg {
    fn default() -> Self {
        Self { delay: 4 }
    }
}

impl Default for DarkTowerCfg {
    fn default() -> Self {
        Self {
            stalactites: StalactitesCfg { timer: 25, timer_offset: 5, acceleration: 0.164 },
            balls: BallsCfg { shift_per_tick: 0.015 },
        }
    }
}

impl Default for StalactitesCfg {
    fn default() -> Self {
        Self { timer: 25, timer_offset: 5, acceleration: 0.164 }
    }
}

impl Default for BallsCfg {
    fn default() -> Self {
        Self { shift_per_tick: 0.015 }
    }
}

impl Default for HauntingDreamCfg {
    fn default() -> Self {
        Self { doors: DoorsCfg { toggle_delay: 10 } }
    }
}

impl Default for DoorsCfg {
    fn default() -> Self {
        Self { toggle_delay: 10 }
    }
}

impl Default for FartZoneCfg {
    fn default() -> Self {
        Self {
            dummy: DummyCfg { velocity: 0.046875 },
            black_rings: BlackRingsCfg { enabled: true },
        }
    }
}

impl Default for DummyCfg {
    fn default() -> Self {
        Self { velocity: 0.046875 }
    }
}

impl Default for CharacterSpecific {
    fn default() -> Self {
        Self { tails: TailsCfg::default() }
    }
}

impl Default for TailsCfg {
    fn default() -> Self {
        Self { projectile_speed: 14, projectile_timeout_timer: 5 }
    }
}

impl Default for Anticheat {
    fn default() -> Self {
        Self {
            palette_anticheat: false,
            zone_anticheat: true,
            distance_anticheat: true,
            ability_anticheat: true,
            data_based_anticheat: true,
            physics_anticheat: false,
            physics_anticheat_log_only: true,
            physics_correction: false,
            physics_correction_log_only: true,
            useless_anticheat: UselessAnticheat::default(),
            zero_trust_anticheat: false,
            zero_trust_log_only: true,
        }
    }
}

impl Default for UselessAnticheat {
    fn default() -> Self {
        Self { enable: false, strict_mode: UselessStrictMode::default() }
    }
}

impl Default for UselessStrictMode {
    fn default() -> Self {
        Self { c1: [0, 0, 0], c2: [0, 0, 0] }
    }
}

impl Default for Banana {
    fn default() -> Self {
        Self { disable_timer: false }
    }
}

impl Default for GMCycle {
    fn default() -> Self {
        Self { overhell: false }
    }
}

impl Default for ResultsMisc {
    fn default() -> Self {
        Self { enabled: true, timer: 15, pride: true }
    }
}

impl Default for MiscellaneousConfig {
    fn default() -> Self {
        Self { other: OtherConfig { ignore_inadequate_configuration: false, instructor_enabled: true } }
    }
}

impl Default for OtherConfig {
    fn default() -> Self {
        Self { ignore_inadequate_configuration: false, instructor_enabled: true }
    }
}

pub fn load_config() -> anyhow::Result<Config> {
    const CONFIG_FILE: &str = "Config.toml";
    if let Ok(data) = std::fs::read_to_string(CONFIG_FILE) {
        let mut cfg: Config = toml::from_str(&data)?;

        cfg.states.map_selection.map_list.resize(21, false);
        log::debug!("{} loaded.", CONFIG_FILE);
        Ok(cfg)
    } else {
        log::warn!("Config.toml not found, using defaults.");
        let cfg = Config::default();

        if let Ok(out) = toml::to_string_pretty(&cfg) {
            let _ = std::fs::write(CONFIG_FILE, out);
        }
        Ok(cfg)
    }
}

impl Config {
    pub fn verify(&self) -> bool {
        let gp = &self.states.gameplay;
        let rm = &gp.entities_misc.map_specific.ravine_mist;
        if !gp.banana.disable_timer && !gp.gmcycle.overhell && gp.ring_appearance_timer < gp.escape_time {
            log::error!("ring_appearance_timer must be >= escape_time when timer is enabled");
            return false;
        }
        if rm.shards.amount > 12 {
            log::error!("ravine_mist.shards.amount must be <= 12");
            return false;
        }
        if rm.shards.amount < rm.shards.required_for_exit {
            log::error!("shards.amount must be >= required_for_exit");
            return false;
        }
        if rm.slugs.ring_chance + rm.slugs.red_ring_chance > 100 {
            log::error!("slug ring chances sum must be <= 100");
            return false;
        }
        if self.states.lobby_misc.lobby_ready_required_percentage > 100 {
            log::error!("lobby_ready_required_percentage must be <= 100");
            return false;
        }
        let crl = &self.states.lobby_misc.chat_rate_limit;
        if crl.enable && (crl.burst < 1.0 || crl.messages_per_second <= 0.0) {
            log::error!("chat_rate_limit: burst must be >= 1.0 and messages_per_second > 0.0 when enabled");
            return false;
        }
        if self.server_config.networking.server_count == 0 {
            log::error!("networking.server_count must be >= 1");
            return false;
        }
        if self.server_config.pairing.ping_limit < 3 {
            log::error!("pairing.ping_limit must be >= 3");
            return false;
        }
        true
    }
}
