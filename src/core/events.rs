//! Things the simulation wants shown or heard. The simulation never plays a
//! sound or draws anything itself: the server forwards these to clients, the
//! client presents them (and ignores the ones produced while re-simulating).

use crate::core::resources::{SoundId, SpriteId};
use serde::{Deserialize, Serialize};

/// image_speed of net_quick_effect when the call gives none.
pub const EFFECT_SPEED: f64 = 0.5;

/// Whose code made an event in the original, which decides which clients
/// predicted it and which must be told about it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum EventSource {
    /// The player's own step (movement, abilities): its client predicts it.
    Player(usize),
    /// Another player's attack reaching this one: only the server sees it.
    DoneTo(usize),
    /// Shots, trackers, waves and clones.
    World,
}

impl SimEvent {
    /// The player a level object's event is about, whose client alone may be told.
    pub fn done_to(&self) -> Option<usize> {
        match *self {
            SimEvent::TailsDollNoticed { target } | SimEvent::TailsDollChases { target } => Some(target),
            SimEvent::TailsDollCaught { victim, .. } => Some(victim),
            _ => None,
        }
    }

    /// Events for the player they happened to alone, which nobody else sees or hears.
    pub fn is_private(&self) -> bool {
        matches!(
            self,
            SimEvent::LocalSound { .. }
                | SimEvent::CameraShake
                | SimEvent::CameraOnPlayer
                | SimEvent::TeleportFlash
                | SimEvent::SmallCameraShake
                | SimEvent::RedRingEnded
                | SimEvent::RedRingStarted
                | SimEvent::RingsHealed
                | SimEvent::TailsDollNoticed { .. }
                | SimEvent::DummyPushed { .. }
                | SimEvent::PotatoBoom
                | SimEvent::SlugHit { .. }
                | SimEvent::ControlsReversed { .. }
                | SimEvent::EyeRequest { .. }
                | SimEvent::DummyHit { everyone: false, .. }
                | SimEvent::TailsDollChases { .. }
                | SimEvent::BloodScreen { .. }
                | SimEvent::HidingWarning { .. }
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SimEvent {
    /// audio_play_sound + net_sound_emit of the original.
    Sound { sound: SoundId, x: f64, y: f64 },
    /// audio_play_sound without net_sound_emit: heard by this player only.
    LocalSound { sound: SoundId },
    /// audio_stop_sound.
    StopSound { sound: SoundId },
    /// obj_player_warning: this player has not left one screenful of the map for too
    /// long (`shown`), or has finally moved away from it.
    HidingWarning { shown: bool },
    /// SERVER_RING_COLLECTED: this player took a ring off the map. The server counts
    /// it for the round's statistics, and every client sparkles where it stood.
    RingTaken { x: f64, y: f64, red: bool },
    /// net_tails_spawn_projectile: the game creates the shot (see tails_projectile.rs).
    SpawnTailsProjectile { x: f64, y: f64, dir: f64, damage: i32, hurts_survivors: bool, charge: i32 },
    /// CLIENT_ETRACKER: Eggman placed a tracker mine at this point.
    SpawnEggTracker { x: f64, y: f64 },
    /// A tracker caught someone: the client shows the victim to Eggman (indices into Game::players).
    TrackerCaught { eggman: usize, victim: usize },
    /// CLIENT_CREAM_SPAWN_RINGS: rings (red rings when demonized) appear at this point.
    SpawnCreamRings { x: f64, y: f64, demonized: bool },
    /// EXE taunted with the emotion key of this number (1 to 3). The client picks the
    /// voice line (original: random, and skipped while another voice line plays).
    Taunt { exe: crate::core::player::ExeCharacter, emotion: u8, x: f64, y: f64 },
    /// EXE turned invisible. The client picks the sound variant (original: random, and
    /// skipped while a taunt plays), so the simulation stays deterministic.
    ExeVanished { x: f64, y: f64 },
    /// EXE became visible again; same sound rules as ExeVanished.
    ExeAppeared { x: f64, y: f64 },
    /// CLIENT_ERECTOR_BRING_SPAWN: Exetior placed a black ring. The client also plays a
    /// random Exetior voice line unless a taunt is playing.
    SpawnBlackRing { x: f64, y: f64 },
    /// CLIENT_ERECTOR_BALLS: Exetior's stomp landed here; the game spawns the shockwaves.
    SpawnStompWaves { x: f64, y: f64 },
    /// scr_exe_checkwin: a survivor died from an EXE's hit (indices into Game::players);
    /// the client plays a kill line of the killer from where the killer stands.
    KilledByExe { victim: usize, killer: usize },
    /// CLIENT_EXELLER_SPAWN_CLONE: Exeller asks for a clone here (the game enforces the limit).
    SpawnExellerClone { x: f64, y: f64, dir: f64 },
    /// CLIENT_EXELLER_TELEPORT_CLONE: Exeller asks to teleport to the clone in this slot.
    TeleportToClone { slot: crate::core::player::CloneSlot },
    /// CLIENT_KAFMONITOR_ACTIVATE: the player's attack or boost broke a speed monitor
    /// (the game hands it to the level).
    MonitorHit { nid: u8 },
    /// CLIENT_HDDOOR_TOGGLE: the player used a Haunting Dream crystal (the game hands it to the level).
    CrystalUsed,
    /// CLIENT_NAPICE_ACTIVATE: the player broke a Nasty Paradise ice block (the game hands it to the level).
    IceBlockHit { nid: u8 },
    /// CLIENT_VVVASE_BREAK: the player broke a Volcano Valley vase (the game hands it to the level).
    VaseHit { nid: u8 },
    /// SERVER_DTTAILSDOLL_STATE 2 and 3 on the target's client: the Tails Doll noticed
    /// them (snd_tailsball), and now chases them (snd_tailsball_chase).
    TailsDollNoticed { target: usize },
    TailsDollChases { target: usize },
    /// SERVER_DTTAILSDOLL_STATE 1: it caught them (a jumpscare for them, a scream for the others).
    TailsDollCaught { victim: usize, x: f64, y: f64 },
    /// SERVER_NPCONTROLLER_STATE 1: Not Perfect's stage switched and everyone moved by
    /// this much (a white flash and snd_npteleport).
    StageSwitched { dx: f64, dy: f64 },
    /// CLIENT_FART_PUSH: the training dummy was hit and slides this fast (the game
    /// hands it to the level).
    DummyPushed { speed: f64 },
    /// obj_fart_text over the dummy, with snd_dummy: seconds of stun (positive) or
    /// health (negative) the hit would have been worth.
    /// `everyone`: every client saw the hit (a shot or a wave), not just the hitter's.
    DummyHit { worth: i32, x: f64, y: f64, everyone: bool },
    /// obj_fart_ass: the statue's curse ran out on this player; their game ends.
    PotatoBoom,
    /// CLIENT_RMZSLIME_HIT: the player squashed a Ravine Mist slug (the game hands it to the level).
    SlugHit { id: u16 },
    /// SERVER_RMZSLIME_STATE 2: a slug was squashed here (its death, snd_slime).
    SlugSquashed { x: f64, y: f64, facing_right: bool },
    /// swapControls: a crystal here reversed the player's controls (the screen turns inside out around it).
    ControlsReversed { x: f64, y: f64 },
    /// CLIENT_LCEYE_REQUEST_ACTIVATE: the player asks to look through a Limp City eye at
    /// `target`, or to stop (the game hands it to the level).
    EyeRequest { eye: u8, target: u8, on: bool },
    /// CLIENT_PFLIT_ACTIVATE: the player touched a Priceless Freedom lift.
    LiftTouched { lift: crate::core::world::InstanceId },
    /// The camera jumps onto the player (a zipline's grab).
    CameraOnPlayer,
    /// obj_majong_controller fade: a quick white flash after a teleport.
    TeleportFlash,
    /// SERVER_MOVINGSPIKE_STATE 1 and 3: the moving spikes go up or down (snd_movingspike
    /// from the map's spike sound emitters).
    SpikesMove,
    /// SERVER_GHZTHUNDER_STATE 0: lightning flashes over Green Hill (obj_flash, snd_thunder).
    Lightning,
    /// net_quick_effect: a one-shot animation at a position.
    Effect { sprite: SpriteId, x: f64, y: f64, xscale: f64, image_speed: f64, yspd: f64 },
    /// scr_effect_fade_quick copy of a running character (speed trail).
    Trail { sprite: SpriteId, image_index: f64, x: f64, y: f64, xscale: f64 },
    /// obj_chaos_liquid: a piece Chaos sheds while running, which falls to the floor
    /// and breaks there.
    ChaosLiquid { x: f64, y: f64 },
    /// scr_camera_shake(25, 1, 0.2) after taking damage.
    CameraShake,
    /// scr_camera_shake(10, 0.2, 0.01): the light rumble of a falling stomp.
    SmallCameraShake,
    /// Red ring effect is over: the client stops mus_mindfuck and the red screen.
    RedRingEnded,
    /// A red ring was taken: the client plays mus_mindfuck and shows the red screen.
    RedRingStarted,
    /// CLIENT_RING_BROKE: EXE or a demon broke a ring; its pieces fly off with the breaker's speed.
    RingBroke { x: f64, y: f64, xspd: f64 },
    /// CLIENT_STATS_REPORT 0: rings healed the survivor.
    RingsHealed,
    /// CLIENT_PLAYER_HEAL_PART: sparkles rise over a teammate being healed.
    HealSparkles { x: f64, y: f64 },
    /// CLIENT_PLAYER_DEATH_STATE of the original: the game rules count the death.
    PlayerDied { revival_times: i32 },
    /// CLIENT_PLAYER_HURT and CLIENT_STATS_REPORT: an attack landed (indices into
    /// Game::players). A hit when `stun_seconds` is 0, a stun otherwise.
    PlayerHit { victim: usize, attacker: usize, damage: i32, stun_seconds: f64 },
    /// CLIENT_MERCOIN_BONUS: the attacker earned the bonus of this number.
    MercoinBonus { player: usize, bonus: u8 },
    /// obj_level.bloodFade = 1: the victim's screen flashes red.
    BloodScreen { victim: usize },
    /// CLIENT_SPRING_USE in Hide and Seek 2: the spring echo reveals the position to EXE.
    SpringEcho { x: f64, y: f64 },
    /// A Tails shot reached someone (obj_tails_projectile, CLIENT_MERCOIN_BONUS 2): its
    /// damage (a survivor's shot: EXE's stun in seconds; a demon's: hits) and whether it
    /// was EXE. For the shooter's achievements.
    ShotHit { shooter: usize, damage: i32, on_exe: bool },
    /// scr_survivor_heal: `healer` gave `target` some of their health (CLIENT_PLAYER_HEAL).
    TeammateHealed { healer: usize, target: usize },
    /// obj_blackring: this player took an Exetior's black ring.
    BlackRingTaken { player: usize },
    /// obj_ravintmist_shard: this player found a shard.
    ShardFound { player: usize },
    /// Sally's shield took a hit for its player; `on_last_hit`: they had one hit left.
    ShieldBlocked { on_last_hit: bool },
}
