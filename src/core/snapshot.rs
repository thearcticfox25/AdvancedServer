//! What an authoritative server tells a client every tick: the whole state of the
//! client's own player, which the client predicts from, and what it shows of the
//! others (obj_player_puppet).

use crate::core::resources::SpriteId;
use crate::core::config::GameplayConfig;
use crate::core::events::{EventSource, SimEvent};
use crate::core::player::{Character, ExeCharacter, Player};
use serde::{Deserialize, Serialize};

/// SERVER_ROUND_CONFIG: what a client needs to simulate the round like the server.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RoundSetup {
    /// Player ids in the order of the simulation's players, which events refer to.
    pub players: Vec<u16>,
    pub config: GameplayConfig,
    /// The rate the server simulates at, so the client predicts in the same steps.
    pub tick_rate: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// The server tick the state is from.
    pub tick: u32,
    /// The newest input tick of this client the state includes.
    pub acked_input: Option<u32>,
    /// None while this client has no player in the round (spectating, escaped).
    pub own: Option<Player>,
    pub others: Vec<OtherPlayer>,
    /// The tick's events, without the ones this client predicted itself.
    pub events: Vec<(EventSource, SimEvent)>,
    /// What the players' abilities left in the level.
    pub entities: Vec<EntityView>,
    pub level: crate::core::level::LevelView,
}

/// An ability's object as clients draw it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum EntityView {
    /// obj_tails_projectile
    TailsShot { x: f32, y: f32, sprite_index: SpriteId, image_index: f32, dir: f32 },
    /// obj_eggtrack
    EggTracker { x: f32, y: f32, sprite_index: SpriteId, image_index: f32 },
    /// obj_exetior_stompballs, while visible
    StompWave { x: f32, y: f32, image_index: f32, dir: f32 },
    /// obj_exeller_clone: whose clone (player id) and in which of its two slots.
    ExellerClone { owner: u16, slot: u8, x: f32, y: f32, sprite_index: SpriteId, image_xscale: f32 },
    /// obj_blackring
    BlackRing { id: u16, x: f32, y: f32, alpha: f32 },
    /// obj_ring and obj_redring: the rings the level put out, as they stand now.
    Ring { x: f32, y: f32, red: bool, alpha: f32 },
}

/// Every ability object of `game`; `player_ids` gives the id of each simulated player.
pub fn entity_views(game: &crate::core::game::Game, player_ids: &[u16]) -> Vec<EntityView> {
    let shots = game.projectiles.iter().map(|shot| EntityView::TailsShot {
        x: shot.x as f32,
        y: shot.y as f32,
        sprite_index: shot.sprite_index,
        image_index: shot.image_index as f32,
        dir: shot.dir as f32,
    });
    let trackers = game.trackers.iter().map(|tracker| EntityView::EggTracker {
        x: tracker.x as f32,
        y: tracker.y as f32,
        sprite_index: tracker.sprite_index,
        image_index: tracker.image_index as f32,
    });
    let waves = game.stomp_waves.iter().filter(|wave| wave.visible).map(|wave| EntityView::StompWave {
        x: wave.x as f32,
        y: wave.y as f32,
        image_index: wave.image_index as f32,
        dir: wave.dir as f32,
    });
    let clones = game.exeller_clones.iter().filter_map(|clone| {
        let owner = *player_ids.get(clone.owner)?;
        let slot = game.players.get(clone.owner)?.clones.iter().position(|&id| id == clone.id).unwrap_or(0) as u8;
        Some(EntityView::ExellerClone { owner, slot, x: clone.x as f32, y: clone.y as f32, sprite_index: clone.sprite_index, image_xscale: clone.image_xscale as f32 })
    });
    let black_rings = game.world.black_rings.iter().map(|ring| EntityView::BlackRing { id: ring.id, x: ring.x as f32, y: ring.y as f32, alpha: ring.alpha as f32 });
    let rings = game.world.rings.iter().map(|ring| EntityView::Ring { x: ring.x as f32, y: ring.y as f32, red: ring.red, alpha: ring.alpha as f32 });
    shots.chain(trackers).chain(waves).chain(clones).chain(black_rings).chain(rings).collect()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct OtherPlayer {
    pub id: u16,
    /// Where they are and what they are doing, while this client may see them.
    /// None for someone the server left out of this client's view, which it does
    /// for players but never for spectators (states.gameplay.limit_player_view).
    pub view: Option<PlayerView>,
    /// What the health row at the bottom of the screen says about them wherever they
    /// stand. Every client of the original knew this much about everyone in the round,
    /// so it is not hidden by distance; where they are is.
    pub health: HealthView,
}

/// A player as the health row and the player list draw them (level/hud.rs).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct HealthView {
    pub character: Character,
    /// Which killer, when `character` is EXE (the player list draws them apart).
    pub exe_character: ExeCharacter,
    pub hp: i32,
    /// 1: down once, 2: demonized.
    pub revival_times: i32,
    pub red_ring: bool,
    pub is_dead: bool,
}

impl HealthView {
    pub fn of(player: &Player) -> HealthView {
        HealthView {
            character: player.character,
            exe_character: player.exe_character,
            hp: player.hp,
            revival_times: player.revival_times,
            red_ring: player.red_ring_timer > 0,
            is_dead: player.is_dead,
        }
    }

    pub fn is_killers_side(&self) -> bool {
        crate::core::player::on_killers_side(self.character, self.revival_times)
    }
}

/// What someone else's player looks like: enough to draw it, nothing to predict it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerView {
    pub character: Character,
    pub exe_character: ExeCharacter,
    pub x: f32,
    pub y: f32,
    pub sprite_index: SpriteId,
    pub image_index: f32,
    pub image_xscale: f32,
    /// Degrees, 0 while spinning.
    pub angle: f32,
    pub state: u16,
    pub hp: i32,
    pub rings: i32,
    pub revival_times: i32,
    pub attack_charge: i32,
    pub dead_timer: i32,
    /// Fart Zone: ticks left of the statue's curse (CLIENT_PLAYER_POTATER).
    pub potato_ticks: u16,
    pub flags: u16,
}

const HURT: u16 = 1 << 0;
const HIDING: u16 = 1 << 1;
const SPINNING: u16 = 1 << 2;
const ATTACKING: u16 = 1 << 3;
const SHIELDED: u16 = 1 << 4;
const RED_RING: u16 = 1 << 5;
const DEAD: u16 = 1 << 6;
const GROUNDED: u16 = 1 << 7;
const EMOTION: u16 = 1 << 8;
const INVISIBLE: u16 = 1 << 9;
/// The player is in their pause menu (CLIENT_ROUND_PAUSE).
const AFK: u16 = 1 << 10;
/// The server has heard nothing from the player's client for a while (isInactive).
const INACTIVE: u16 = 1 << 11;

impl PlayerView {
    pub fn of(player: &Player) -> PlayerView {
        let flag = |on: bool, bit: u16| if on { bit } else { 0 };
        PlayerView {
            character: player.character,
            exe_character: player.exe_character,
            x: player.x as f32,
            y: player.y as f32,
            sprite_index: player.sprite_index,
            image_index: player.image_index as f32,
            image_xscale: player.image_xscale as f32,
            angle: if player.is_spinning { 0.0 } else { player.angle.to_degrees() as f32 },
            state: player.state as u16,
            hp: player.hp,
            rings: player.rings,
            revival_times: player.revival_times,
            attack_charge: player.attack_charge,
            dead_timer: player.dead_timer,
            potato_ticks: player.potato_ticks.max(0) as u16,
            // As CLIENT_PLAYER_DATA told others: "hurt" while invincible, the attack only in its hitting frames.
            flags: flag(player.hurttime > 0, HURT)
                | flag(player.is_hiding, HIDING)
                | flag(player.is_spinning, SPINNING)
                | flag(crate::core::contact::seen_attacking(player), ATTACKING)
                | flag(player.shield_timer > 0, SHIELDED)
                | flag(player.red_ring_timer > 0, RED_RING)
                | flag(player.is_dead, DEAD)
                | flag(player.is_grounded, GROUNDED)
                | flag(player.emotion, EMOTION)
                | flag(!crate::core::contact::seen_visible(player), INVISIBLE)
                | flag(player.paused, AFK)
                | flag(player.inactive, INACTIVE),
        }
    }

    /// An invisible Sonic.EXE is not drawn for others (obj_player_puppet visible).
    pub fn is_visible(&self) -> bool {
        self.flags & INVISIBLE == 0
    }

    /// A player with this look, for code that draws players.
    pub fn to_player(&self, cfg: &GameplayConfig) -> Player {
        let (x, y) = (self.x as f64, self.y as f64);
        let mut player = if self.character == Character::Exe {
            Player::new_exe(self.exe_character, x, y, cfg)
        } else {
            Player::new(self.character, x, y, cfg)
        };
        let has = |bit: u16| self.flags & bit != 0;
        player.sprite_index = self.sprite_index;
        player.image_index = self.image_index as f64;
        player.image_xscale = self.image_xscale as f64;
        player.angle = (self.angle as f64).to_radians();
        player.state = self.state as usize;
        player.hp = self.hp;
        player.rings = self.rings;
        player.revival_times = self.revival_times;
        player.attack_charge = self.attack_charge;
        player.dead_timer = self.dead_timer;
        player.potato_ticks = i32::from(self.potato_ticks);
        player.is_hurt = has(HURT);
        player.is_hiding = has(HIDING);
        player.is_spinning = has(SPINNING);
        player.is_attacking = has(ATTACKING);
        player.shield_timer = i32::from(has(SHIELDED));
        player.red_ring_timer = i32::from(has(RED_RING));
        player.is_dead = has(DEAD);
        player.is_grounded = has(GROUNDED);
        player.emotion = has(EMOTION);
        player.paused = has(AFK);
        player.inactive = has(INACTIVE);
        player
    }
}

impl Snapshot {
    pub fn encode(&self) -> Vec<u8> {
        postcard::to_allocvec(self).expect("snapshots always serialize")
    }

    pub fn decode(bytes: &[u8]) -> Option<Snapshot> {
        postcard::from_bytes(bytes).ok()
    }
}

impl RoundSetup {
    pub fn encode(&self) -> Vec<u8> {
        postcard::to_allocvec(self).expect("round setups always serialize")
    }

    pub fn decode(bytes: &[u8]) -> Option<RoundSetup> {
        postcard::from_bytes(bytes).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_and_config_survive_the_wire() {
        let cfg = GameplayConfig::default();
        let mut own = Player::new_exe(ExeCharacter::Chaos, 120.5, 300.25, &cfg);
        own.xspd = -3.75;
        own.clones = [4, -1];
        let other = Player::new(Character::Sally, 10.0, 20.0, &cfg);
        let events = vec![(EventSource::DoneTo(1), SimEvent::PlayerHit { victim: 1, attacker: 0, damage: 40, stun_seconds: 0.0 })];
        let entities = vec![EntityView::EggTracker { x: 1.0, y: 2.0, sprite_index: crate::core::resources::names::sprite::SPR_EGGTRACK, image_index: 0.5 }];
        let level = crate::core::level::LevelView { water_shocks: Some(true), spike_frame: Some(2), gassed_area: Some(4), broken_monitors: Some(0b101), doors: Some((true, false, vec![3, 96])), lifts: vec![crate::core::level::LiftView { raised: 12.5, alpha: 1.0, carrying: true }], broken_ice: Some(1 << 9), snowballs: vec![None, Some(crate::core::level::SnowballView { leg: 3, progress: 0.5, frame: 20 })], lava: vec![crate::core::level::LavaView { below: -130.0, state: 3 }], broken_vases: Some(0b11), dark_tower: Some(crate::core::level::DarkTowerView { stalactites: vec![None], doll: (177.0, 944.0, 2) }), not_perfect: Some((3, true, false)), act9_walls: Some(0.25), lit_lantern: Some(6), ball_swing: Some(-0.5), dummy: Some((1616.0, true)), ravine_mist: Some(crate::core::level::RavineMistView { found: 3, shards: vec![crate::core::level::ShardView { id: 4, x: 1.0, y: 2.0, falling: true }], slugs: vec![crate::core::level::SlugView { id: 9, x: 3.0, y: 4.0, ring: crate::core::level::SlugRing::RedRing, facing_right: false, age: 70 }] }), echidna_ruins: Some((true, 0.5, 2)), limp_city: Some(crate::core::level::LimpCityView { chains: 2, eyes: vec![crate::core::level::EyeView { used: true, target: 3, charge: 60 }] }), torture_cave: Some((Some(3), vec![None, Some(40)])) };
        let snapshot = Snapshot { tick: 77, acked_input: Some(70), own: Some(own), others: vec![OtherPlayer { id: 3, view: Some(PlayerView::of(&other)), health: HealthView::of(&other) }], events, entities, level };

        let bytes = snapshot.encode();
        assert_eq!(Snapshot::decode(&bytes), Some(snapshot.clone()));
        assert!(bytes.len() < 1200, "a snapshot of two players fits one datagram: {} bytes", bytes.len());
        let setup = RoundSetup { players: vec![3, 1], config: cfg.clone(), tick_rate: 72 };
        assert_eq!(RoundSetup::decode(&setup.encode()), Some(setup));

        let shown = snapshot.others[0].view.as_ref().expect("the other player is in view").to_player(&cfg);
        assert_eq!((shown.character, shown.x, shown.sprite_index), (Character::Sally, 10.0, other.sprite_index));
    }

    /// CLIENT_ROUND_PAUSE: the others are told that a player is in their pause menu,
    /// which is all the round does with it.
    #[test]
    fn a_paused_player_is_shown_as_away() {
        let cfg = GameplayConfig::default();
        let mut player = Player::new(Character::Tails, 0.0, 0.0, &cfg);
        assert!(!PlayerView::of(&player).to_player(&cfg).paused);
        player.paused = true;
        assert!(PlayerView::of(&player).to_player(&cfg).paused);
    }
}
