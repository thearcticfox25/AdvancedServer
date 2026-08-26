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
    pub hosts_name: String,
    pub lower_bracket: String,
    pub server_location: String,
    pub message_of_the_day: String,
    pub lobby_timeout_timer: u8,
    pub lobby_start_timer: u8,
    /// Strips non-op players of all vote and lobby-start rights. The map and
    /// other lobby decisions are controlled exclusively by operators. (это
    /// самое бесполезное, что я делал...)
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
    /// Synchronises Sudden Death's clock with each player's own death
    /// countdown -- Sudden Death is, de facto, also a player-death timer (it
    /// kills everyone still down), so de jure it should never disagree with
    /// death_timer_sec about when that happens. Caps a dead player's
    /// displayed/effective death_timer_sec to the time actually remaining
    /// until sudden death, and forces it to keep counting down through
    /// exe-camp freeze once sudden death is closer than the respawn timer --
    /// without this, the shown countdown can promise a respawn later than
    /// sudden death will actually kill everyone still down, or freeze
    /// indefinitely near the exe with sudden death still bearing down
    /// regardless. Correctly inverts for banana.disable_timer, where time_sec
    /// counts up instead of down.
    pub sync_sudden_death_timers: bool,
    pub ending_timer: u8,
    pub waiting_timeout: u8,
    /// Elimination-tournament progression, off at 0. Runs a real Lobby entry
    /// between rounds -- clients need that to leave Results and reset their
    /// per-round state -- but with spectator promotion suppressed for as long
    /// as the tournament still has another round left, so anyone waiting in
    /// the spectator room stays there instead of joining a running tournament.
    /// Spectators also aren't shown the Results screen for a round that isn't
    /// the tournament's last, since they won't be following it into Lobby.
    /// Once elimination would leave too few players to continue (below
    /// lobby_misc.min_players_required), that round is treated as the
    /// tournament's last and spectators are let in same as always. After
    /// viewing results, the worst-ranked survivor(s) (by the same ranking used
    /// for the results screen) are kicked before the next round starts. If
    /// every survivor escaped -- no bad performance to rank -- the exe is
    /// eliminated instead.
    ///
    /// 1: kick the single worst player.
    /// 2: kick half the round's participants if that count is even, else fall
    ///    back to mode 1.
    /// 3: same as mode 2 when even; when odd, kick two thirds if the count is
    ///    divisible by 3, else fall back to mode 1.
    pub tournament_mode: u8,
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
    /// Percentage of non-exe in-game players force-demonized the instant the round
    /// starts, before anyone can be wounded and independent of sudden death. 0
    /// disables it. Rounds to the nearest player count.
    pub ambush_force_demonization_percentage_on_start: u8,
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
            hosts_name: "The Arctic Fox".to_string(),
            lower_bracket: "------------------------".to_string(),
            server_location: "Saint Petersburg".to_string(),
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
            sync_sudden_death_timers: true,
            ending_timer: 5,
            waiting_timeout: 15,
            tournament_mode: 0,
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
        Self { overhell: false, ambush_force_demonization_percentage_on_start: 0 }
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

/// (TOML section path, field name) -> comment text to emit above that field when
/// writing a fresh default Config.toml, mirroring the doc comment on the matching
/// struct/field above. `field: None` places the comment above the `[section]`
/// header itself instead of a specific key. Keep in sync by hand when adding or
/// changing a doc comment above -- a stale or missing entry here just means the
/// generated file has no comment for that field, not a broken config.
const CONFIG_TOML_COMMENTS: &[(&str, Option<&str>, &str)] = &[
    // server_config.networking
    ("server_config.networking", Some("port"),
        "Changes the port the server listens on. Note: the game cannot handle a\nport specified after \":\" in the address -- if you want a non-standard\nport, distribute a custom build with the port hardcoded."),
    ("server_config.networking", Some("server_count"),
        "Number of sub-servers (lobbies). If a lobby is full, players are\nredirected to another one; if none are available, they receive a\n\"Server Full\" error."),
    ("server_config.networking.vinny", None,
        "Per-tick flood cap, raw ENet events/packets. Lives under `networking`\nsince it applies before any game-state processing."),
    ("server_config.networking.vinny", Some("max_events_per_tick"),
        "Maximum raw ENet events (connect/receive/disconnect) drained per tick,\nacross all peers combined."),
    ("server_config.networking.vinny", Some("max_packets_per_peer_per_tick"),
        "Maximum packets processed from a single peer per tick; that peer's\nremaining packets are dropped for the rest of the tick."),

    // server_config.pairing
    ("server_config.pairing", Some("maximum_players_per_lobby"),
        "Maximum players allowed in a single lobby."),
    ("server_config.pairing", Some("ip_validation"),
        "Prevents players without op (admin) from connecting with two clients\nsharing the same IP address."),
    ("server_config.pairing", Some("ping_limit"),
        "The server periodically calculates each player's rolling average ping.\nIf it exceeds this value (ms), the player is kicked."),
    ("server_config.pairing", Some("player_maximum_errors"),
        "Number of network errors after which the server kicks the player."),
    ("server_config.pairing", Some("rate_limit_window"),
        "If a player reconnects too quickly, their connection is throttled for\nthis many seconds."),
    ("server_config.pairing", Some("kick_timeout_window"),
        "Default kick duration (seconds). Also used as the timeout applied by\n.vk (votekick)."),
    ("server_config.pairing.versioning", Some("target_version"),
        "Client version required to connect. Known compatible versions: v1101,\nv110-leaked-build."),
    ("server_config.pairing.versioning", Some("disable_version_validating"),
        "If true, version checking is bypassed and all clients are allowed to\nconnect."),

    // server_config.logging
    ("server_config.logging", Some("log_level"),
        "0: startup banner + basic init report + terminal command replies only\n   (those always show -- an admin needs to see their own command run).\n1: level 0 + ERROR logs.\n2: level 1 + WARN logs.\n3 (default, matches the C version's default verbosity): level 2 + INFO logs.\n4: level 3 + DEBUG logs."),
    ("server_config.logging", Some("log_to_file"),
        "Saves log output to a file on disk."),

    // states.lobby_misc
    ("states.lobby_misc", Some("upper_bracket"),
        "Decorative line framing the top of the welcome message shown when a\nplayer connects -- an \"opening bracket,\" in the style old games used to\nframe their connection banners."),
    ("states.lobby_misc", Some("server_location"),
        "Display-only label shown in the welcome window as \"%server_location%,\nmax ping: %max_acceptable_ping%ms\", helping players decide if the server\nsuits them by region and latency. Has no effect on server logic."),
    ("states.lobby_misc", Some("hosts_name"),
        "The name displayed for the host."),
    ("states.lobby_misc", Some("lower_bracket"),
        "Decorative line framing the bottom of the welcome message -- the\nmatching \"closing bracket\" to `upper_bracket`."),
    ("states.lobby_misc", Some("message_of_the_day"),
        "Custom message shown after the welcome splash."),
    ("states.lobby_misc", Some("lobby_timeout_timer"),
        "Time (seconds) a non-op player can remain in the lobby with an unready\nstatus without taking any action before being auto-kicked for inactivity."),
    ("states.lobby_misc", Some("lobby_start_timer"),
        "Countdown duration (seconds) once the required percentage of players\nis ready."),
    ("states.lobby_misc", Some("authoritarian_mode"),
        "Strips non-op players of all vote and lobby-start rights. The map and\nother lobby decisions are controlled exclusively by operators. (это самое\nбесполезное, что я делал...)"),
    ("states.lobby_misc", Some("lobby_ready_required_percentage"),
        "Percentage of ready players required to trigger the game-start countdown."),
    ("states.lobby_misc", Some("kick_unready_before_starting"),
        "If true, kicks unready players when the countdown expires. Only\nrelevant when `lobby_ready_required_percentage` is below 100."),
    ("states.lobby_misc", Some("anonymous_mode"),
        "Replaces all player usernames with \"anonymous\". Does not apply to\nmoderators."),
    ("states.lobby_misc", Some("min_players_required"),
        "Minimum number of players that must be in the lobby before the\nready-percentage check can trigger the game-start countdown."),

    ("states.lobby_misc.moderation", Some("ban_ip"),
        "Enables IP-based banning."),
    ("states.lobby_misc.moderation", Some("ban_udid"),
        "Enables UDID-based banning."),
    ("states.lobby_misc.moderation", Some("ban_nickname"),
        "Enables nickname-based banning."),
    ("states.lobby_misc.moderation", Some("op_default_level"),
        "Default operator level granted by the ?op command. The in-lobby .op\ncommand always assigns this default level."),
    ("states.lobby_misc.moderation", Some("enforce_whitelist"),
        "Inverts the banned/allowed player logic. Players are checked against a\nseparate whitelist table and must be present in it to connect, rather\nthan absent from the ban list."),
    ("states.lobby_misc.moderation", Some("shy_mode"),
        "Does not notify a player when their admin status is revoked -- they\nwill discover it after attempting a command or on their next reconnect.\nGranting op still sends a notification."),
    ("states.lobby_misc.moderation", Some("aggressive_username_ban"),
        "Leftover from early testing. When enabled, a nickname ban also chains\nto nicknames indirectly associated with the banned player through prior\nname reuse. Left disabled by default: since nicknames aren't unique and\nany player can take another player's name, this could turn moderation\ninto a joke -- or into a \"terminator\" that bans unrelated players in a\nchain reaction from a single flagged nickname. May also no longer\nfunction as intended after later changes to the ban system; treat as\nunsupported/experimental."),

    ("states.lobby_misc.votekick", Some("cooldown"),
        "Duration of the votekick vote (seconds)."),
    ("states.lobby_misc.votekick", Some("autoban_leavers"),
        "If true, a player who disconnects while a votekick is active against\nthem is treated as a deserter and the kick is counted as successful,\nrather than cancelling the vote. Still only applies the normal timed\nkick (kick_timeout_window), not a permanent ban."),
    ("states.lobby_misc.votekick", Some("vote_result_delay"),
        "Delay (seconds) between vote completion and execution of the result,\ngiving players time to read the outcome."),
    ("states.lobby_misc.votekick", Some("use_legacy_voting"),
        "Switches votekick voting to the mechanic used in the game's original\nv1.0.0 client."),

    ("states.lobby_misc.chat_rate_limit", Some("enable"),
        "Enables chat rate limiting -- an anti-spam / chat throttling mechanism.\nUnverified by the developer; treat its behaviour as unconfirmed until\nyou've checked it in practice."),
    ("states.lobby_misc.chat_rate_limit", Some("burst"),
        "Number of messages a player can send in a quick burst before\nthrottling kicks in."),
    ("states.lobby_misc.chat_rate_limit", Some("messages_per_second"),
        "Sustained message rate (messages/sec) a player is limited to once\ntheir burst allowance is used up."),
    ("states.lobby_misc.chat_rate_limit", Some("exempt_op_level"),
        "Operators at or above this level are exempt from the chat rate limit."),

    // states.map_selection
    ("states.map_selection", Some("enabled"),
        "If false, a random map from the enabled pool is chosen instantly and\nthe server skips to Character Selection, bypassing Map Vote."),
    ("states.map_selection", Some("timer"),
        "Duration of the map voting phase (seconds)."),
    ("states.map_selection", Some("map_list"),
        "Map pool. Entries correspond to maps in their in-game index order:\n1 Hide 'n Seek 2, 2 Ravine Mist, 3 ..., 4 Desert Town, 5 You Can't Run,\n6 Limb City, 7 Not Perfect, 8 Kind & Fair, 9 Act 9, 10 Nasty Paradise,\n11 Priceless Freedom, 12 Volcano Valley, 13 Hill, 14 Majin Forest,\n15 Hide 'n Seek / Angel Island, 16 Torture Cave, 17 Dark Tower,\n18 Haunting Dream, 19 Mystic Wood (Codename: Weed Zone), 20 Echidna Ruins\n(Codename: Marijuana), 21 Practice Test Map (Codename: Fart Zone) --\ndisabled by default; enabling it with the original client causes a crash\nbecause the game tries to read map description data that does not exist\nin memory."),
    ("states.map_selection", Some("exclude_last_map"),
        "Removes the previously played map from the vote pool."),

    // states.character_selection
    ("states.character_selection", Some("enable"),
        "If false, each player is assigned a random character and the server\nskips to the Game stage, bypassing Character Selection."),
    ("states.character_selection", Some("charselect_mod_unlocked"),
        "Tricks the game into allowing multiple players to pick the same\ncharacter and disables the related server-side uniqueness checks."),
    ("states.character_selection", Some("allow_foreign_characters"),
        "Allows selecting characters beyond the original roster. Not intended\nfor the original client -- may cause it to crash or display\nmodded-client players as the Placeholder character."),
    ("states.character_selection", Some("charselect_timer"),
        "Time allotted for character selection (seconds)."),

    // states.gameplay
    ("states.gameplay", Some("respawn_time"),
        "Time (seconds) teammates have to revive a killed player."),
    ("states.gameplay", Some("sudden_death_timer"),
        "Time (seconds) after which players can no longer be revived. Note: the\ngame client always displays the Sudden Death warning at 2 minutes -- if\nyou change this value, inform players via motd or server banner."),
    ("states.gameplay", Some("ring_appearance_timer"),
        "Time (seconds) at which the exit ring silhouette appears on the map."),
    ("states.gameplay", Some("escape_time"),
        "Time (seconds) at which the actual exit ring appears. This is the\nwindow players have to jump in and win."),
    ("states.gameplay", Some("demonization_percentage"),
        "Percentage of players who become EXE pawns on death; beyond this\nthreshold, subsequent deaths are permanent."),
    ("states.gameplay", Some("exe_camp_penalty"),
        "If true, pauses the respawn timer while Sonic.EXE is camping a dying\nplayer's body."),
    ("states.gameplay", Some("hide_player_characters"),
        "Replaces all player characters with the Placeholder character."),
    ("states.gameplay", Some("enable_achievements"),
        "Sends achievement completion reports to clients. If you have changed\nany server settings, consider disabling this -- achievements should be\nearned as originally intended, and config changes may make some easier\nor harder to obtain."),
    ("states.gameplay", Some("enable_sounds"),
        "Sends sound event data from other players to the client. Originally\nan odd bug on the \"Ice Star\" Experimental Server, later kept as an\noptional fun mode."),
    ("states.gameplay", Some("gametimers_ceiling"),
        "Maximum round duration (seconds). The game adds extra time per\nadditional player -- this cap prevents excessively long matches with\nlarge player counts."),
    ("states.gameplay", Some("sync_sudden_death_timers"),
        "Synchronises Sudden Death's clock with each player's own death\ncountdown -- Sudden Death is, de facto, also a player-death timer (it\nkills everyone still down), so de jure it should never disagree with\ndeath_timer_sec about when that happens."),
    ("states.gameplay", Some("ending_timer"),
        "Duration (seconds) of the end-of-game cutscene before the Results\nstage begins."),
    ("states.gameplay", Some("waiting_timeout"),
        "Time (seconds) the server waits for clients to finish loading the game\nscene before kicking those who failed to prepare in time."),
    ("states.gameplay", Some("tournament_mode"),
        "Elimination-tournament progression, off at 0. Runs a real Lobby entry\nbetween rounds -- clients need that to leave Results and reset their\nper-round state -- but with spectator promotion suppressed for as long as\nthe tournament still has another round left, so anyone waiting in the\nspectator room stays there instead of joining a running tournament.\nSpectators also aren't shown the Results screen for a round that isn't\nthe tournament's last, since they won't be following it into Lobby. Once\nelimination would leave too few players to continue (below\nlobby_misc.min_players_required), that round is treated as the\ntournament's last and spectators are let in same as always. After\nviewing results, the worst-ranked survivor(s) (by the same ranking used\nfor the results screen) are kicked before the next round starts. If\nevery survivor escaped -- no bad performance to rank -- the exe is\neliminated instead.\n\n1: kick the single worst player.\n2: kick half the round's participants if that count is even, else fall\n   back to mode 1.\n3: same as mode 2 when even; when odd, kick two thirds if the count is\n   divisible by 3, else fall back to mode 1."),

    // states.gameplay.entities_misc
    ("states.gameplay.entities_misc.global.rings", Some("enabled"),
        "Enables ring spawning globally."),
    ("states.gameplay.entities_misc.global.rings", Some("red_ring_chance"),
        "Percentage chance for a spawned ring to be a red ring."),
    ("states.gameplay.entities_misc.global.spikes", Some("timer"),
        "Interval (seconds) between spike trap triggers."),

    ("states.gameplay.entities_misc.map_specific.ravine_mist.shards", Some("amount"),
        "Total number of shards that spawn."),
    ("states.gameplay.entities_misc.map_specific.ravine_mist.shards", Some("required_for_exit"),
        "Number of shards survivors must collect to open the exit."),
    ("states.gameplay.entities_misc.map_specific.ravine_mist.slugs", Some("enabled"),
        "Enables slug enemy spawning."),
    ("states.gameplay.entities_misc.map_specific.ravine_mist.slugs", Some("ring_chance"),
        "Percentage chance a slain slug drops a ring."),
    ("states.gameplay.entities_misc.map_specific.ravine_mist.slugs", Some("red_ring_chance"),
        "Percentage chance the dropped ring is a red ring."),

    ("states.gameplay.entities_misc.map_specific.you_cant_run.gas", Some("delay"),
        "Delay (seconds) before the gas hazard activates."),

    ("states.gameplay.entities_misc.map_specific.limb_city.eye", Some("recharge_strength"),
        "Amount the eye power meter recharges per tick."),
    ("states.gameplay.entities_misc.map_specific.limb_city.eye", Some("recharge_timer"),
        "Interval (seconds) between recharge ticks."),
    ("states.gameplay.entities_misc.map_specific.limb_city.eye", Some("use_cost"),
        "Power meter cost per use of the eye ability."),
    ("states.gameplay.entities_misc.map_specific.limb_city.chain", Some("delay"),
        "Delay (seconds) before the chain hazard activates."),
    ("states.gameplay.entities_misc.map_specific.limb_city.chain", Some("warning"),
        "Warning duration (seconds) before the chain strikes."),
    ("states.gameplay.entities_misc.map_specific.limb_city.chain", Some("shocking_time"),
        "Duration (seconds) the shock effect lasts."),

    ("states.gameplay.entities_misc.map_specific.not_perfect", Some("switch_timer"),
        "Interval (seconds) between platform state switches (normal phase)."),
    ("states.gameplay.entities_misc.map_specific.not_perfect", Some("switch_timer_chase"),
        "Interval (seconds) between switches during the chase phase."),
    ("states.gameplay.entities_misc.map_specific.not_perfect", Some("switch_warning_timer"),
        "Warning duration (seconds) before a switch (normal phase)."),
    ("states.gameplay.entities_misc.map_specific.not_perfect", Some("switch_warning_timer_chase"),
        "Warning duration (seconds) before a switch during the chase phase."),

    ("states.gameplay.entities_misc.map_specific.kind_and_fair.speedbox", Some("timer"),
        "Duration (seconds) the speed boost lasts."),
    ("states.gameplay.entities_misc.map_specific.kind_and_fair.speedbox", Some("timer_offset"),
        "Random offset (seconds) added to the timer."),

    ("states.gameplay.entities_misc.map_specific.act9.walls", Some("enabled"),
        "Enables the moving wall hazards."),

    ("states.gameplay.entities_misc.map_specific.nasty_paradise.snowballs", Some("enabled"),
        "Enables snowball projectile spawning."),
    ("states.gameplay.entities_misc.map_specific.nasty_paradise.ice", Some("regeneration_timer"),
        "Time (seconds) before broken ice tiles regenerate."),

    ("states.gameplay.entities_misc.map_specific.priceless_freedom.black_rings", Some("enabled"),
        "Enables black ring spawning on this map."),

    ("states.gameplay.entities_misc.map_specific.hills.thunder", Some("timer"),
        "Base interval (seconds) between lightning strikes."),
    ("states.gameplay.entities_misc.map_specific.hills.thunder", Some("timer_offset"),
        "Random offset (seconds) added to the interval."),

    ("states.gameplay.entities_misc.map_specific.torture_cave.acid", Some("delay"),
        "Delay (seconds) before the acid hazard activates."),

    ("states.gameplay.entities_misc.map_specific.dark_tower.stalactites", Some("timer"),
        "Base interval (seconds) between stalactite drops."),
    ("states.gameplay.entities_misc.map_specific.dark_tower.stalactites", Some("timer_offset"),
        "Random offset (seconds) added to the interval."),
    ("states.gameplay.entities_misc.map_specific.dark_tower.stalactites", Some("acceleration"),
        "Fall acceleration applied to each stalactite."),
    ("states.gameplay.entities_misc.map_specific.dark_tower.balls", Some("shift_per_tick"),
        "Lateral movement distance per tick for the rolling balls."),

    ("states.gameplay.entities_misc.map_specific.haunting_dream.doors", Some("toggle_delay"),
        "Interval (seconds) between door open/close toggles."),

    ("states.gameplay.entities_misc.map_specific.fart_zone.dummy", Some("velocity"),
        "Movement speed of the dummy character."),
    ("states.gameplay.entities_misc.map_specific.fart_zone.black_rings", Some("enabled"),
        "Enables black ring spawning on this map."),

    ("states.gameplay.entities_misc.character_specific.tails", Some("projectile_speed"),
        "Initial speed of Tails's projectile."),
    ("states.gameplay.entities_misc.character_specific.tails", Some("projectile_timeout_timer"),
        "Time (seconds) before a projectile despawns."),

    // states.gameplay.anticheat
    ("states.gameplay.anticheat", Some("palette_anticheat"),
        "Detects and rejects colour palettes not present in the original game."),
    ("states.gameplay.anticheat", Some("zone_anticheat"),
        "Uses exported map layouts to detect players clipping inside solid map\nobjects."),
    ("states.gameplay.anticheat", Some("distance_anticheat"),
        "Prevents single-tick teleports and blatant speed hacking."),
    ("states.gameplay.anticheat", Some("ability_anticheat"),
        "Verifies that character ability cooldowns are being respected."),
    ("states.gameplay.anticheat", Some("data_based_anticheat"),
        "Checks for logical impossibilities based on known game state -- e.g. a\nplayer cannot win by jumping into an exit ring that does not exist."),
    ("states.gameplay.anticheat", Some("physics_anticheat"),
        "Compares player physics against server-side expectations derived from\na reimplementation of the game's physics engine."),
    ("states.gameplay.anticheat", Some("physics_anticheat_log_only"),
        "If true, physics anticheat violations are logged as DEBUG messages\nwithout kicking the player."),
    ("states.gameplay.anticheat", Some("physics_correction"),
        "Teleports the player back to a valid position when a physics\ndiscrepancy is detected."),
    ("states.gameplay.anticheat", Some("physics_correction_log_only"),
        "If true, physics corrections are logged as DEBUG messages without\nbeing applied."),
    ("states.gameplay.anticheat", Some("zero_trust_anticheat"),
        "Exact purpose unknown; possibly a duplicate of another check."),
    ("states.gameplay.anticheat", Some("zero_trust_log_only"),
        "If true, zero-trust violations are logged as DEBUG messages without\ntaking action."),

    ("states.gameplay.anticheat.useless_anticheat", Some("enable"),
        "Verifies the client signature, as the official TD2DR servers did\nbefore their shutdown."),
    ("states.gameplay.anticheat.useless_anticheat.strict_mode", Some("c1"),
        "Client-signature check constants for the proprietary anticheat. If\nyou've decompiled the original (non-FOSS) server binary rather than this\nFOSS build, you can plug in the matching values from that server to\nreproduce the exact client check the official servers once performed.\nOf limited practical value -- cheaters have long since worked out how to\nbypass it -- but available if you want bit-for-bit parity with the\noriginal."),
    ("states.gameplay.anticheat.useless_anticheat.strict_mode", Some("c2"),
        "Second set of client-signature check constants, paired with `c1`."),

    // states.gameplay.banana / gmcycle
    ("states.gameplay.banana", Some("disable_timer"),
        "Reimplementation of the No Timer Mod Server by Banana 7800."),
    ("states.gameplay.gmcycle", Some("overhell"),
        "Reimplementation of the OverHell game mode from the Gamemode Cycle\nmod by IcedCoffee & ColdestTea."),
    ("states.gameplay.gmcycle", Some("ambush_force_demonization_percentage_on_start"),
        "Percentage of non-exe in-game players force-demonized the instant the round\nstarts, before anyone can be wounded and independent of sudden death. 0\ndisables it. Rounds to the nearest player count."),

    // states.results_misc
    ("states.results_misc", Some("enabled"),
        "Shows the post-game results screen."),
    ("states.results_misc", Some("timer"),
        "Duration of the results screen (seconds)."),
    ("states.results_misc", Some("pride"),
        "Displays camp tease messages on the results screen."),

    // miscellaneous.other
    ("miscellaneous.other", Some("ignore_inadequate_configuration"),
        "Forces the server to run even with settings that may cause crashes or\nunfinishable games."),
    ("miscellaneous.other", Some("instructor_enabled"),
        "Enables the instructor / tutorial system."),
];

fn comment_prefix(comment: &str) -> String {
    comment.lines()
        .map(|l| if l.is_empty() { "#\n".to_string() } else { format!("# {}\n", l) })
        .collect()
}

fn navigate_table<'a>(root: &'a mut toml_edit::Table, path: &str) -> Option<&'a mut toml_edit::Table> {
    let mut table = root;
    for seg in path.split('.') {
        table = table.get_mut(seg)?.as_table_mut()?;
    }
    Some(table)
}

/// Prepends `comment` to whatever prefix decor is already there (e.g. the blank
/// line toml_edit puts before a new `[section]`), so the existing spacing is kept
/// rather than replaced.
fn prepend_comment(decor: &mut toml_edit::Decor, comment: &str) {
    let existing = decor.prefix().and_then(|s| s.as_str()).unwrap_or("");
    decor.set_prefix(format!("{}{}", existing, comment_prefix(comment)));
}

/// Attaches a `#` comment, via toml_edit's own decor API, above the matching
/// `[section]` header or `key = value` line of a freshly-generated default
/// Config.toml, per CONFIG_TOML_COMMENTS.
fn annotate_default_toml(raw: &str) -> String {
    let Ok(mut doc) = raw.parse::<toml_edit::DocumentMut>() else { return raw.to_string(); };

    for (path, field, comment) in CONFIG_TOML_COMMENTS {
        let Some(table) = navigate_table(doc.as_table_mut(), path) else { continue };
        match field {
            None => { prepend_comment(table.decor_mut(), comment); }
            Some(key) => {
                if let Some(mut key_mut) = table.key_mut(*key) {
                    prepend_comment(key_mut.leaf_decor_mut(), comment);
                }
            }
        }
    }

    doc.to_string()
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
            let _ = std::fs::write(CONFIG_FILE, annotate_default_toml(&out));
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
        if gp.gmcycle.ambush_force_demonization_percentage_on_start > 100 {
            log::error!("gmcycle.ambush_force_demonization_percentage_on_start must be <= 100");
            return false;
        }
        if gp.tournament_mode > 3 {
            log::error!("tournament_mode must be 0-3");
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
