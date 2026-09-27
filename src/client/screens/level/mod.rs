//! A level during a round: the room's layers, the simulation's instances and
//! players, the camera (obj_camera) and the level's music (room creation code).
//!
//! In a round on a server this client predicts its own player from its buttons
//! and corrects the prediction with every snapshot of the server's simulation;
//! the other players are shown as the server last saw them. Without a
//! connection (a TEMP debug start) the level simulates this player alone.

mod act9;
mod camera;
mod drawing;
mod effects;
mod dark_tower;
mod echidna_ruins;
mod fart_zone;
mod desert_town;
mod dotdotdot;
mod green_hill;
mod hiding_warning;
mod limp_city;
mod nasty_paradise;
mod not_perfect;
mod pause;
mod pets;
mod hud;
mod indicators;
mod rings;
mod ravine_mist;
mod room_objects;
mod torture_cave;
mod voices;
mod volcano_valley;
mod weed_zone;
mod you_cant_run;

use super::chat::Chat;
use super::fades::BlackFadeIn;
use super::{count_down_alarm, go_to_error, ALARM_OFF};
use crate::client::canvas::Canvas;
use crate::client::levels::LEVELS;
use crate::client::net::{self, Notice, PlayerSkin};
use crate::client::palette::{self, Colours};
use crate::client::room::{LayerContent, Room};
use crate::client::Context;
use anyhow::{bail, Result};
use macroquad::input::KeyCode;
use camera::{Camera, CameraMode, CameraView};
use drawing::{draw_instance, draw_player, Skin, TAIL_IMAGE_SPEED};
use effects::{effect_blend, Effect, HealSparkle, HEAL_SPARKLE_DEPTH, TRAIL_BEHIND};
use hud::{Hud, HudPlayer};
use rings::{draw_red_screen, draw_ring, next_ring_frame, ring_pieces, RingPart, RED_SCREEN_DEPTH, RING_DEPTH};
use macroquad::rand;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use crate::core::resources::names::{sound, sprite};
use crate::core::config::{GameplayConfig, ticks_per_second};
use crate::core::events::{EventSource, SimEvent, EFFECT_SPEED};
use crate::core::game::{Game, RoundEnding};
use crate::core::objects::ids::ObjectId;
use crate::core::palettes::{PALETTE_DEMON, PALETTE_EXE};
use crate::core::player::{Buttons, Character, ExeCharacter, Player};
use crate::core::rooms::ids::RoomId;
use crate::core::snapshot::{entity_views, EntityView, HealthView, OtherPlayer};
use crate::core::world::{create_depth, InstanceId, ObjectVars, World, C_WHITE};
use crate::client::canvas::{C_DKGRAY, VIEW_HEIGHT, VIEW_WIDTH};
use crate::client::text::{draw_text, text_width};
use crate::core::config::step;

/// obj_spawnpoint and obj_exespawn create the player this far above themselves.
const SPAWN_ABOVE: f64 = 18.0;
/// instance_create_depth(x, y, 0, global.player)
const OWN_PLAYER_DEPTH: i32 = 0;
/// obj_player_puppet: depth = obj_spawnpoint.depth + 2.
const OTHERS_BEHIND_SPAWN_POINTS: i32 = 2;
/// scr_camera_shake arguments of SimEvent::CameraShake and SmallCameraShake.
const HIT_SHAKE: (i32, f64, f64) = (25, 1.0, 0.2);
const SMALL_SHAKE: (i32, f64, f64) = (10, 0.2, 0.01);
/// Buttons of this many latest ticks go in every CLIENT_ROUND_INPUT.
const INPUTS_PER_PACKET: usize = 8;
/// Unconfirmed buttons kept for correcting the prediction; a server that falls
/// further behind than this is past predicting anyway.
const MAX_UNCONFIRMED_INPUTS: usize = 120;
/// Someone else's sounds fade out at this distance from the middle of the view.
const HEARING_DISTANCE: f64 = 700.0;
/// udp_timeout = 60 * 5: this long without a snapshot and the server is taken for gone
/// (global.errorCode 4); the sign shows from udp_timeout < 60 * 3, 2 seconds in.
const SERVER_SILENT_SECONDS: f64 = 5.0;
const BAD_CONNECTION_AFTER_SECONDS: f64 = 2.0;
const SERVER_SILENT_ERROR: u32 = 4;
/// scr_audio_play_3d on Majin Forest: sounds this near either end play at the other too.
const MAJIN_FOREST_SOUND_EDGE: f64 = 512.0;
/// Where some emitters measure their gain from in the view (see play_sound_near_ear).
const LEFT_EAR: (f64, f64) = (213.0, 120.0);
/// obj_soundemitter: SOUNDEMT_MOVINGSPIKE, and its sounds come from this far above it.
const SOUND_EMITTER_MOVING_SPIKES: i64 = 0;
const SOUND_EMITTER_ABOVE: f64 = 16.0;
/// SERVER_KAFMONITOR_STATE 2: the explosion stands this far right of and below the monitor.
const MONITOR_EXPLOSION_OFFSET: f64 = 15.0;
/// obj_majong_controller: its white flash loses this much alpha a frame.
const TELEPORT_FLASH_FADE: f64 = 0.32;
/// Others are drawn this many server ticks behind the newest snapshot; the drawn tick
/// jumps to that delay when it drifts further than the slack from it.
const OTHERS_DELAY_TICKS: f64 = 4.0;
const OTHERS_CLOCK_SLACK_TICKS: f64 = 3.0;
const OTHERS_HISTORY_SNAPSHOTS: usize = 16;
/// obj_revival_puppet: depth -144, this far above the downed teammate.
const REVIVAL_SIGN_DEPTH: i32 = -144;
const REVIVAL_SIGN_ABOVE: f64 = 40.0;
/// A survivor needs this many rings to revive (scr_survivor_revive).
const REVIVAL_RINGS: i32 = 3;
/// obj_heal_progress stands this far above the teammate.
const HEAL_GAUGE_ABOVE: f64 = 45.0;
/// Depths of the abilities' objects: obj_eggtrack (the player's - 2),
/// obj_exetior_stompballs (-1) and obj_exeller_clone (8).
const EGG_TRACKER_DEPTH: i32 = -2;
const STOMP_WAVE_DEPTH: i32 = -1;
const EXELLER_CLONE_DEPTH: i32 = 8;
/// obj_blackring: depth -202; its hum is heard within 400 px.
const BLACK_RING_DEPTH: i32 = -202;
const BLACK_RING_HUM_REACH: f64 = 400.0;
/// obj_exeller_clone Draw GUI: EXE sees an arrow bobbing this far above a clone,
/// survivors see the Exeller's name.
const CLONE_ARROW_ABOVE: f64 = 35.0;
const CLONE_NAME_ABOVE: f64 = 43.0;
/// obj_bigring: depth = the player's depth + 5, fading in by 0.01 a step.
const BIG_RING_BEHIND: i32 = 5;
/// obj_marjiuna_static: depth -500.
const STATIC_DEPTH: i32 = -500;
/// SERVER_GAME_SPAWN_RING on Act 9: its chase track fades in over a second.
const ACT9_CHASE_FADE_SECONDS: f64 = 1.0;
const BIG_RING_FADE_PER_STEP: f64 = 0.01;
/// obj_bigring_teleport: depth -100, image_speed = 2.
const BIG_RING_TELEPORT_DEPTH: i32 = -100;
const BIG_RING_TELEPORT_SPEED: f64 = 2.0;
/// obj_netclient End Step: CLIENT_PING once a second once the round runs.
const PING_EVERY_SECONDS: f64 = 1.0;
/// After the round's end: the screen fades to black after 3 seconds, and 4 seconds
/// later the client returns to the lobby unless the server showed the results.
const ENDING_FADE_AFTER_SECONDS: f64 = 3.0;
const ENDING_LOBBY_AFTER_SECONDS: f64 = 4.0;
/// obj_ending: black up to half, the card shrinking from 1.5 while fading in.
const ENDING_FADE_PER_STEP: f64 = 0.01;
/// obj_chaos_liquid: the speeds it drops at and how far off Chaos it comes.
const LIQUID_SPEEDS: [f64; 6] = [4.0, 4.5, 5.0, 5.5, 6.0, 6.5];
const LIQUID_JITTER: i32 = 8;

const ENDING_MAX_FADE: f64 = 0.5;
const ENDING_FRAME_MILLISECONDS: f64 = 100.0;
/// obj_suddendeath: the words hold for four seconds over a half black screen, coming
/// up slowly and leaving twice as fast.
const SUDDEN_DEATH_HOLD_SECONDS: f64 = 4.0;
const SUDDEN_DEATH_FADE_IN: f64 = 0.01;
const SUDDEN_DEATH_FADE_OUT: f64 = 0.02;
/// obj_flash: a white screen losing this much alpha a step.
const FLASH_FADE_PER_STEP: f64 = 0.016;

pub struct Level {
    room: Room,
    /// This player (once known) and the level's instances.
    game: Game,
    /// The simulation's instances standing on each room layer, in creation order.
    layer_instances: Vec<Vec<InstanceId>>,
    /// Instances drawn at a depth their Create event set, or made with the room by the
    /// simulation rather than placed on a layer.
    depth_instances: Vec<(i32, InstanceId)>,
    camera: Option<Camera>,
    /// image_index of Tails' tail (obj_tails_tail, obj_puppet_tail).
    tail_frame: f64,
    others_depth: i32,
    /// This player's place among the round's players, which events refer to.
    own_index: Option<usize>,
    /// nid: this player's id, which the server's objects name their owners by.
    own_id: Option<u16>,
    /// The skin table last sent with CLIENT_PLAYER_PALETTE.
    sent_skin_table: Option<usize>,
    hud: Hud,
    /// obj_ending: the card of the round's end and its fade.
    ending: Option<(crate::core::resources::SpriteId, f64)>,
    /// obj_suddendeath while it is on screen: how dark it is and the ticks it still holds.
    sudden_death: Option<(f64, i32)>,
    /// obj_player_warning while the simulation says this player is hiding.
    hiding_warning: Option<hiding_warning::HidingWarning>,
    /// obj_level alarm[1] and alarm[2].
    ending_fade_alarm: i32,
    ending_lobby_alarm: i32,
    black_fade: Option<BlackFadeIn>,
    effects: Vec<Effect>,
    heal_sparkles: Vec<HealSparkle>,
    /// scr_survivor_heal as this player's client ran it: the gauge over each teammate,
    /// and the gauges shown this tick (where, how full).
    heal_progress: BTreeMap<u16, f64>,
    heal_gauges: Vec<(f64, f64, f64)>,
    ring_parts: Vec<RingPart>,
    /// global.ringFrame
    ring_frame: f64,
    /// obj_redring_screen while this player's red ring lasts: its image_index.
    red_screen: Option<f64>,
    /// obj_bigring: position, ready, image_alpha.
    big_ring: Option<(f64, f64, bool, f64)>,
    /// obj_flash: image_alpha of the lightning's white screen.
    flash: f64,
    /// obj_majong_controller fade.
    teleport_flash: f64,
    green_hill: Option<green_hill::GreenHill>,
    desert_town: Option<desert_town::DesertTown>,
    you_cant_run: Option<you_cant_run::YouCantRun>,
    nasty_paradise: Option<nasty_paradise::NastyParadise>,
    volcano_valley: Option<volcano_valley::VolcanoValley>,
    dark_tower: Option<dark_tower::DarkTower>,
    not_perfect: Option<not_perfect::NotPerfect>,
    act9_bodies: Option<act9::Act9Bodies>,
    weed_zone: Option<weed_zone::WeedZone>,
    fart_zone: Option<fart_zone::FartZone>,
    ravine_mist: Option<ravine_mist::RavineMist>,
    echidna_ruins: Option<echidna_ruins::EchidnaRuins>,
    limp_city: Option<limp_city::LimpCity>,
    torture_cave: Option<torture_cave::TortureCave>,
    dotdotdot: Option<dotdotdot::DotDotDot>,
    indicators: indicators::Indicators,
    /// obj_spawnpoint: the pet that follows this player.
    pet: Option<pets::Follower>,
    /// obj_netclient Other_4: the pets that follow everyone else, by their id.
    other_pets: BTreeMap<u16, pets::Follower>,
    /// This player's pet colours went to the others (CLIENT_PET_PALETTE).
    sent_pet_skin: bool,
    /// This player's health and revivals when last looked at, for the achievements.
    own_last_seen: Option<(i32, i32)>,
    /// The speed monitors broken when last looked at, for their sound and explosion.
    broken_monitors: Vec<InstanceId>,
    /// Haunting Dream's doors open and crystals usable when last looked at, for their sounds.
    doors_seen: (bool, bool),
    /// Which Priceless Freedom lifts carried someone when last looked at.
    lifts_carrying: Vec<bool>,
    /// Watching a round this client plays no part in (states::spectate): the camera
    /// follows the round's players or flies over the map, and the chat is at hand
    /// instead of being a page of the pause menu.
    spectating: bool,
    /// Esc: the options, the chat and the way out, without leaving the level.
    pause: pause::Pause,
    /// The chat of the round, which the original has no room for.
    chat: Chat,
    /// None without a connection.
    online: Option<Online>,
    room_id: RoomId,
    /// global.levels index of the room.
    level_number: usize,
}

struct Online {
    /// This client's tick, counted from the level's start: the tick of the next buttons.
    next_tick: u32,
    /// Buttons by tick, oldest first, kept until the server confirms them.
    inputs: VecDeque<(u32, Buttons)>,
    /// Everyone else this client may see, as drawn this tick: a little in the past,
    /// between two snapshots. Someone the server left out of this client's view
    /// (states.gameplay.limit_player_view) is not in here at all.
    others: Vec<(u16, Player)>,
    /// What the health row shows about everyone else, which distance never hides.
    health: Vec<(u16, HealthView)>,
    /// The latest snapshots' views of everyone else, oldest first, by server tick.
    others_history: VecDeque<(u32, Vec<OtherPlayer>)>,
    /// The server tick drawn now; it follows the newest snapshot at a fixed delay.
    render_tick: f64,
    /// The abilities' objects as the newest snapshot shows them.
    entities: Vec<EntityView>,
    /// obj_netclient.udp_timeout: ticks since the last snapshot, while the round runs.
    silent_ticks: i32,
}

impl Online {
    /// Plan 6.3: others cannot be predicted, so they are shown a few ticks late,
    /// moving smoothly between the two snapshots around the drawn tick.
    fn interpolate_others(&mut self, config: &GameplayConfig) {
        let Some(&(newest, _)) = self.others_history.back() else { return };
        let target = newest as f64 - OTHERS_DELAY_TICKS;
        self.render_tick += 1.0;
        if (self.render_tick - target).abs() > OTHERS_CLOCK_SLACK_TICKS {
            self.render_tick = target;
        }
        let before = self.others_history.iter().rev().find(|(tick, _)| *tick as f64 <= self.render_tick);
        let after = self.others_history.iter().find(|(tick, _)| *tick as f64 > self.render_tick);
        let (base, next, blend) = match (before, after) {
            (Some(before), Some(after)) => (before, Some(after), (self.render_tick - before.0 as f64) / (after.0 - before.0) as f64),
            (Some(only), None) | (None, Some(only)) => (only, None, 0.0),
            (None, None) => return,
        };
        self.others = base
            .1
            .iter()
            .filter_map(|other| Some((other, other.view.as_ref()?)))
            .filter(|(_, view)| view.is_visible())
            .map(|(other, view)| {
                let mut player = view.to_player(config);
                let later = next.and_then(|(_, views)| views.iter().find(|later| later.id == other.id)).and_then(|later| later.view.as_ref());
                if let Some(later) = later {
                    player.x += (later.x as f64 - player.x) * blend;
                    player.y += (later.y as f64 - player.y) * blend;
                }
                (other.id, player)
            })
            .collect();
    }
}

impl Level {
    pub fn open(context: &mut Context, room_id: RoomId) -> Result<Level> {
        let Some(level_number) = LEVELS.iter().position(|level| level.room == room_id) else {
            bail!("room {room_id:?} is not a level");
        };
        let level = &LEVELS[level_number];
        let sprites = context.canvas.sprites.clone();
        let mut room = Room::load(&context.maps_folder, room_id, &sprites)?;
        let online = context.net.is_connected;
        let (config, own_index) = match (&context.net.round_setup, online) {
            (Some(setup), true) => {
                let own_index = setup.players.iter().position(|&id| Some(id) == context.net.id);
                (Arc::new(setup.config.clone()), own_index)
            }
            (None, true) => bail!("the server started the round without sending its gameplay numbers"),
            (_, false) => {
                // Singleplayer has no server to name a rate, so it runs at the default one.
                crate::core::config::set_tick_rate(crate::core::config::DEFAULT_TICKS_PER_SECOND);
                (crate::core::config::gameplay_config(), Some(0))
            }
        };
        let mut world = World::empty(sprites, config);
        world.load_room(&context.maps_folder, room_id)?;
        let (layer_instances, depth_instances) = instances_by_layer(&room, &world)?;
        let spawn_depth = room.placed(ObjectId::Spawnpoint).next().map_or(0, |(layer, _)| room.layers[layer].depth);
        if let Some(setup) = context.net.round_setup.as_ref().filter(|setup| online && setup.players.len() > 1) {
            let played: Vec<i32> = setup.players.iter().filter_map(|id| context.net.players.get(id)).map(|player| player.character).chain([context.net.character]).collect();
            room_objects::show_corpses(&mut world, &played);
        }
        let mut game = Game::new(world);
        let max_hp = game.world.config.hurt.max_hp;

        // Watching the round rather than playing it: this client has no player in the
        // round's roster, so the camera starts out following its players (camera.rs).
        let spectating = online && context.net.is_spectating;
        let mut camera = spectating.then(Camera::watching);
        if !online {
            let spawn = sandbox_spawn(context, &game.world)?;
            game.world.round_started = true;
            // Without a server the level's own objects run here.
            let rings = crate::core::level::map_black_rings(room_id, &game.world.config);
            game.world.place_map_black_rings(rings);
            game.level = Some(crate::core::level::Level::new(&game.world, rand::rand() as u64));
            camera = Some(match sandbox_player(context, &game.world.config, spawn) {
                Some(player) => {
                    let camera = Camera::new(&player);
                    game.players.push(player);
                    camera
                }
                // The free camera picked on the page plays nobody, so the level has no
                // player at all and the camera flies on its own.
                None => Camera::free(spawn.0, spawn.1),
            });
        }
        let pet = game.players.first().and_then(|player| pets::Follower::new(context.unlockables.pet, room_id, player));
        context.canvas.inverse_circle = None;
        if room_id == RoomId::Act9 {
            context.audio.play_music_with_quiet_track(level.music, level.chase_music);
        } else if room_id == RoomId::Dotdotdot {
            // obj_dotdotdot_i Create: its second track waits for the last part of the level.
            context.audio.play_music_with_quiet_track(level.music, sound::MUS_DOTDOTDOT2);
        } else {
            context.audio.play_music(level.music);
        }
        let nasty_paradise = (room_id == RoomId::Nastyparadise).then(|| nasty_paradise::NastyParadise::open(&game.world));
        let volcano_valley = matches!(room_id, RoomId::Volcanovalley | RoomId::Marijuna).then(|| volcano_valley::VolcanoValley::open(&game.world));
        let dark_tower = (room_id == RoomId::Dartower).then(|| dark_tower::DarkTower::open(&game.world));
        let not_perfect = (room_id == RoomId::Notperfect).then(|| not_perfect::NotPerfect::open(&mut room));
        let act9_bodies = (room_id == RoomId::Act9).then(|| act9::Act9Bodies::open(&mut game.world));
        let is_exe = own_character(context).0 == Character::Exe;
        let fart_zone = (room_id == RoomId::Fartzone).then(fart_zone::FartZone::default);
        let ravine_mist = (room_id == RoomId::Ravinemist).then(ravine_mist::RavineMist::default);
        let echidna_ruins = (room_id == RoomId::Marijuna).then(|| echidna_ruins::EchidnaRuins::open(&game.world));
        let limp_city = (room_id == RoomId::Limpcity).then(|| limp_city::LimpCity::open(&game.world));
        let dotdotdot = (room_id == RoomId::Dotdotdot).then(dotdotdot::DotDotDot::default);
        let torture_cave = (room_id == RoomId::Torturecave).then(|| torture_cave::TortureCave::open(&mut context.audio));
        let weed_zone = (room_id == RoomId::Weedzone).then(|| weed_zone::WeedZone::open(&mut game.world, is_exe));
        context.achievements.start_round(room_id, online);
        Ok(Level {
            room,
            game,
            layer_instances,
            depth_instances,
            camera,
            tail_frame: 0.0,
            others_depth: spawn_depth + OTHERS_BEHIND_SPAWN_POINTS,
            own_index,
            own_id: if online { context.net.id } else { Some(0) },
            spectating,
            sent_skin_table: None,
            hud: Hud::new(room_id, level_number, !online, spectating, max_hp),
            ending: None,
            sudden_death: None,
            hiding_warning: None,
            ending_fade_alarm: ALARM_OFF,
            ending_lobby_alarm: ALARM_OFF,
            black_fade: None,
            effects: Vec::new(),
            heal_sparkles: Vec::new(),
            heal_progress: BTreeMap::new(),
            heal_gauges: Vec::new(),
            ring_parts: Vec::new(),
            ring_frame: 0.0,
            red_screen: None,
            big_ring: None,
            flash: 0.0,
            teleport_flash: 0.0,
            green_hill: (room_id == RoomId::Greenhill).then(|| green_hill::GreenHill::open(&mut context.audio)),
            desert_town: (room_id == RoomId::Deserttown).then(desert_town::DesertTown::default),
            you_cant_run: (room_id == RoomId::Youcantrun).then(you_cant_run::YouCantRun::default),
            nasty_paradise,
            volcano_valley,
            dark_tower,
            not_perfect,
            act9_bodies,
            weed_zone,
            fart_zone,
            ravine_mist,
            echidna_ruins,
            limp_city,
            torture_cave,
            dotdotdot,
            indicators: indicators::Indicators::default(),
            pet,
            other_pets: BTreeMap::new(),
            sent_pet_skin: false,
            own_last_seen: None,
            broken_monitors: Vec::new(),
            doors_seen: (false, true),
            lifts_carrying: Vec::new(),
            online: online.then(|| Online {
                next_tick: 0,
                inputs: VecDeque::new(),
                others: Vec::new(),
                health: Vec::new(),
                others_history: VecDeque::new(),
                render_tick: 0.0,
                entities: Vec::new(),
                silent_ticks: 0,
            }),
            pause: pause::Pause::default(),
            chat: Chat::default(),
            room_id,
            level_number,
        })
    }

    pub fn tick(&mut self, context: &mut Context) -> Option<RoomId> {
        let current_time_ms = (macroquad::time::get_time() * 1000.0).floor();
        let next_room = self.step(context);
        if next_room.is_some() {
            // The level is going: its looping sounds (the emitters of black rings, lava and
            // the like) go with it, as their instances freed them, and the pause effect ends.
            self.pause.end(context);
            context.audio.stop_all();
        }
        self.draw(context, current_time_ms);
        next_room
    }

    fn step(&mut self, context: &mut Context) -> Option<RoomId> {
        let online = self.online.is_some();
        if self.pause.step(context, &mut self.chat, online) == pause::Action::Leave {
            return Some(if online { net::reset(context) } else { RoomId::Menu });
        }
        // Alone the level has no server to keep it running, so a pause stops it.
        if self.pause.open && !online {
            return None;
        }
        self.room.step(&context.canvas.sprites);
        if count_down_alarm(&mut self.ending_fade_alarm) {
            self.black_fade = Some(BlackFadeIn::new());
            self.ending_lobby_alarm = crate::core::config::ticks(ENDING_LOBBY_AFTER_SECONDS);
        }
        if count_down_alarm(&mut self.ending_lobby_alarm) {
            return Some(net::leave_round(context));
        }
        if let Some(fade) = self.black_fade.as_mut() {
            fade.step();
        }
        // obj_controls Begin Step
        if context.input.pressed(context.options.keys.hide_gui.0) && !self.pause.open {
            self.hud.shown = !self.hud.shown;
        }
        // The pause menu has its own keys while it is open, the chat among them.
        if self.spectating && !self.pause.open {
            self.step_spectator(context);
        }
        // A paused player holds nothing down: they stand in the round, visible and hittable.
        // A spectator holds nothing down while typing, so the camera does not fly away.
        let buttons = if self.pause.open || self.chat.is_open { Buttons::default() } else { held_buttons(context) };
        if self.online.is_some() {
            for notice in net::update(context) {
                match notice {
                    Notice::GoTo(room) => return Some(room),
                    Notice::ShowError(code) => return Some(go_to_error(context, code)),
                    Notice::RoundTime(ticks_left) => self.hud.clock.sync(&mut context.audio, ticks_left),
                    Notice::SuddenDeath => {
                        self.sudden_death = Some((0.0, crate::core::config::ticks(SUDDEN_DEATH_HOLD_SECONDS)));
                        context.audio.play(sound::SND_SUDDENDEATH, false);
                    }
                    Notice::RoundEnded(ending, counts) => {
                        let now = self.achievement_situation(context);
                        let exe_of_round = self.online.as_ref().and_then(|online| online.others.iter().find(|(_, other)| other.character == Character::Exe).map(|(_, exe)| exe.exe_character));
                        context.achievements.round_ended(ending, counts, &now, exe_of_round);
                        self.round_ended(context, ending);
                    }
                    Notice::RevivedTeammate => context.achievements.revived_teammate(),
                    Notice::BigRing { ready, spawn } => self.big_ring_changed(context, ready, spawn),
                    Notice::Escaped => {
                        let now = self.achievement_situation(context);
                        context.achievements.escaped(&now);
                        self.game.players.clear();
                        if let Some(camera) = self.camera.as_mut().filter(|camera| camera.mode == CameraMode::Follow) {
                            camera.mode = CameraMode::Fade;
                        }
                        if self.red_screen.take().is_some() {
                            context.audio.stop(sound::MUS_MINDFUCK);
                        }
                    }
                    Notice::ChatMessage { sender, text } => self.chat.add_message(context, &sender, &text),
                    Notice::OtherEscaped(_) => {
                        context.achievements.other_escaped();
                        context.audio.play(sound::SND_TELEPORT, false);
                        if let Some((x, y, _, _)) = self.big_ring {
                            let mut teleport = Effect::quick(sprite::SPR_RING_TELEPORT, x, y, 1.0, BIG_RING_TELEPORT_SPEED, C_WHITE);
                            teleport.depth = BIG_RING_TELEPORT_DEPTH;
                            self.effects.push(teleport);
                        }
                    }
                    _ => {}
                }
            }
            self.game.world.round_started = context.net.is_server_ready;
            if self.count_silence(context) {
                return Some(super::go_to_error(context, SERVER_SILENT_ERROR));
            }
            self.apply_snapshot(context);
            for (source, event) in std::mem::take(&mut context.net.round_events) {
                self.on_server_event(context, source, &event);
            }
            self.send_skin_when_changed(context);
            self.send_pet_skin_once(context);
            self.watch_own_for_achievements(context);
            if let Some(online) = self.online.as_mut() {
                online.interpolate_others(&self.game.world.config);
            }
            self.show_heal_gauges();
            self.send_buttons(context, buttons);
        }
        // Online the server runs the level, so a client with no player of its own (a
        // spectator) has nothing to predict. Without a server the level runs here,
        // with or without a player: the free camera plays nobody, and the level's
        // objects must go on all the same.
        if !self.game.players.is_empty() || self.online.is_none() {
            let events = self.game.tick(&[buttons]);
            let now = self.achievement_situation(context);
            for event in &events {
                self.on_event(context, event, true, None);
                // This player's own step, predicted here: the local game has them alone, at 0.
                context.achievements.on_own_event(event);
                context.achievements.on_event(event, |index| index == 0, true, |_| false, &now);
            }
        }
        room_objects::step(&mut self.game.world, self.game.players.first());
        self.stop_fly_sound_when_nobody_flies(context);
        let sprites = context.canvas.sprites.clone();
        let world = &self.game.world;
        self.effects.retain_mut(|effect| effect.step(&sprites, world));
        let now_ms = (macroquad::time::get_time() * 1000.0).floor();
        self.heal_sparkles.retain_mut(|sparkle| sparkle.step(now_ms));
        self.ring_frame = next_ring_frame(self.ring_frame);
        let world = &self.game.world;
        self.ring_parts.retain_mut(|part| part.step(world));
        if let Some((_, _, _, alpha)) = self.big_ring.as_mut() {
            if *alpha < 1.0 {
                *alpha += BIG_RING_FADE_PER_STEP * step();
            }
        }
        if let Some(frame) = self.red_screen.as_mut() {
            *frame += sprites.get(sprite::SPR_REDRING_FORE).fps as f64 / ticks_per_second();
        }
        self.flash = (self.flash - FLASH_FADE_PER_STEP * step()).max(0.0);
        if let (Some(green_hill), Some(camera)) = (self.green_hill.as_mut(), &self.camera) {
            green_hill.step((camera.x, camera.y));
        }
        if let (Some(desert_town), Some(camera)) = (self.desert_town.as_mut(), &self.camera) {
            desert_town.step(&mut self.room, &self.game.world, self.game.players.first(), (camera.x, camera.y));
        }
        let now_ms = macroquad::time::get_time() * 1000.0;
        if let (Some(pet), Some(player)) = (self.pet.as_mut(), self.game.players.first()) {
            pet.step(&context.canvas, player, now_ms);
        }
        if let Some(online) = &self.online {
            for (id, player) in &online.others {
                let chosen = context.net.players.get(id).map_or(crate::client::unlockables::NO_PET, |other| other.pet);
                if !self.other_pets.contains_key(id) {
                    if let Some(pet) = pets::Follower::new(chosen, self.room_id, player) {
                        self.other_pets.insert(*id, pet);
                    }
                }
                if let Some(pet) = self.other_pets.get_mut(id) {
                    pet.step(&context.canvas, player, now_ms);
                }
            }
        }
        self.indicators.step();
        self.show_breaking_monitors(context);
        self.hum_black_rings(context);
        self.hear_doors(context);
        self.hear_lifts(context);
        if let (Some(nasty_paradise), Some(camera)) = (self.nasty_paradise.as_mut(), self.camera.as_mut()) {
            let heard = nasty_paradise.step(&mut self.game.world, self.game.players.first(), (camera.x, camera.y));
            if let Some(magnitude) = heard.shake {
                camera.shake(HIT_SHAKE.0, magnitude, HIT_SHAKE.2);
            }
            for (sound, x, y) in heard.sounds {
                play_sound_near_ear(context, camera, sound, x, y);
            }
        }
        if let Some(bodies) = &self.act9_bodies {
            let others = self.online.iter().flat_map(|online| online.others.iter().map(|(_, player)| player));
            bodies.step(&mut self.game.world, self.game.players.iter().chain(others), macroquad::time::get_time() * 1000.0);
        }
        if let (Some(dotdotdot), Some(camera)) = (self.dotdotdot.as_mut(), &self.camera) {
            dotdotdot.step(&mut self.room, &self.game.world, &mut context.audio, (camera.x, camera.y));
        }
        if let (Some(torture_cave), Some(camera)) = (self.torture_cave.as_mut(), &self.camera) {
            for (sound, x, y) in torture_cave.step(&self.game.world) {
                play_sound(context, camera, sound, x, y, false, None);
            }
        }
        if let (Some(limp_city), Some(camera)) = (self.limp_city.as_mut(), self.camera.as_mut()) {
            let heard = limp_city.step(&mut self.game.world, macroquad::time::get_time() * 1000.0);
            for (sound, x, y) in heard {
                play_sound(context, camera, sound, x, y, false, None);
            }
            // SERVER_LCEYE_STATE: the camera looks through the eye this player uses.
            match self.game.players.first().and_then(|own| crate::core::level::watched_eye(&self.game.world, own)) {
                Some((x, y)) if matches!(camera.mode, CameraMode::Follow | CameraMode::Watch) => {
                    camera.mode = CameraMode::Watch;
                    (camera.x, camera.y) = (x.floor() - VIEW_WIDTH / 2.0, y.floor() - VIEW_HEIGHT / 2.0);
                }
                None if camera.mode == CameraMode::Watch => camera.mode = CameraMode::Follow,
                _ => {}
            }
        }
        if let (Some(echidna_ruins), Some(camera)) = (self.echidna_ruins.as_mut(), &self.camera) {
            let heard = echidna_ruins.step(&mut context.canvas, &mut self.game.world, self.game.players.first(), (camera.x, camera.y), self.big_ring.is_some());
            for (sound, x, y) in heard {
                play_sound_near_ear(context, camera, sound, x, y);
            }
        }
        if let Some(sound) = self.ravine_mist.as_mut().and_then(|ravine_mist| ravine_mist.step(&self.game.world)) {
            context.audio.play(sound, false);
        }
        if let Some(fart_zone) = self.fart_zone.as_mut() {
            // game_end()
            if fart_zone.step(context.canvas.sprites.get(sprite::SPR_BOOM).fps as f64) {
                context.quit = true;
            }
        }
        if let (Some(weed_zone), Some(camera)) = (self.weed_zone.as_mut(), &self.camera) {
            let now_ms = macroquad::time::get_time() * 1000.0;
            let (heard, lantern) = weed_zone.step(&mut self.room, &mut self.game.world, self.game.players.first(), (camera.x, camera.y), self.big_ring.is_some(), now_ms);
            for sound in heard {
                context.audio.play(sound, false);
            }
            if let Some((x, y)) = lantern {
                self.indicators.lantern_lit(x, y);
            }
        }
        if let Some(not_perfect) = self.not_perfect.as_mut() {
            let big_ring = self.big_ring.map(|(x, y, _, _)| (x, y));
            not_perfect.step(&mut self.room, &mut self.game.world, big_ring, macroquad::time::get_time() * 1000.0);
        }
        if let (Some(dark_tower), Some(camera)) = (self.dark_tower.as_mut(), self.camera.as_mut()) {
            let heard = dark_tower.step(&mut self.game.world, self.game.players.first());
            if let Some(magnitude) = heard.shake {
                camera.shake(HIT_SHAKE.0, magnitude, HIT_SHAKE.2);
            }
            for (sound, x, y) in heard.sounds {
                play_sound(context, camera, sound, x, y, false, None);
            }
        }
        if let (Some(volcano_valley), Some(camera)) = (self.volcano_valley.as_mut(), &self.camera) {
            let heard = volcano_valley.step(&self.game.world);
            let listener = (camera.x + VIEW_WIDTH / 2.0, camera.y + VIEW_HEIGHT / 2.0);
            for (nid, x, y) in heard.hums {
                let gain = heard_gain((camera.x + LEFT_EAR.0, camera.y + LEFT_EAR.1), x, y);
                context.audio.set_emitter(("lava", nid), sound::SND_LAVA, gain, pan_towards(listener, x, y));
            }
            for (sound, x, y) in heard.sounds {
                play_sound_near_ear(context, camera, sound, x, y);
            }
        }
        if let (Some(you_cant_run), Some(camera)) = (self.you_cant_run.as_mut(), &self.camera) {
            for (x, y) in you_cant_run.step(&self.game.world) {
                play_sound(context, camera, sound::SND_SMOKE, x, y, false, None);
            }
        }
        self.tail_frame += TAIL_IMAGE_SPEED * context.canvas.sprites.get(sprite::SPR_TAILS_TAIL1).fps as f64 / ticks_per_second();
        if let Some(camera) = self.camera.as_mut() {
            let own = self.game.players.first();
            // SERVER_REVIVAL_REVIVED and SERVER_GAME_DEATHTIMER_END put the camera back on this player.
            if !matches!(camera.mode, CameraMode::Follow | CameraMode::Watch) && !camera.locked && own.is_some_and(|own| own.hp > 0 && !own.is_dead) {
                camera.mode = CameraMode::Follow;
            }
            let others = self.online.as_ref().map_or(&[][..], |online| &online.others[..]);
            let view = CameraView {
                own,
                others,
                exe_ids: &context.net.exe_ids,
                any_key_pressed: context.input.any_key_pressed() && !self.chat.is_open,
                buttons,
                room_width: self.game.world.room_width,
                room_height: self.game.world.room_height,
            };
            camera.step(&view, &context.canvas.sprites);
            if context.netlog && self.online.as_ref().is_some_and(|online| online.next_tick % 60 == 0) {
                eprintln!("camera {:?} at ({:.0}, {:.0}), own {:?} room {:?} {}x{}", camera.mode, camera.x, camera.y, self.game.players.first().map(|own| (own.x.round(), own.y.round(), own.sprite_index)), self.game.world.room, self.game.world.room_width, self.game.world.room_height);
            }
        }
        None
    }

    /// The server's state of this player replaces the prediction, and the buttons
    /// the server has not applied yet are played again on top of it.
    /// obj_netclient Step_0: a server that sends no snapshot for SERVER_SILENT_SECONDS is
    /// gone (true), and the HUD warns once it has been silent for BAD_CONNECTION_AFTER_SECONDS.
    /// Before the round starts and once it ends the server sends none, so that is not counted.
    fn count_silence(&mut self, context: &Context) -> bool {
        let Some(online) = self.online.as_mut() else { return false };
        let snapshot_waiting = context.net.round_snapshot.is_some();
        if snapshot_waiting || !context.net.is_server_ready || context.net.game_ends {
            online.silent_ticks = 0;
        } else {
            online.silent_ticks += 1;
        }
        self.hud.bad_connection = online.silent_ticks > crate::core::config::ticks(BAD_CONNECTION_AFTER_SECONDS);
        online.silent_ticks >= crate::core::config::ticks(SERVER_SILENT_SECONDS)
    }

    fn apply_snapshot(&mut self, context: &mut Context) {
        let Some(snapshot) = context.net.round_snapshot.take() else { return };
        let Some(online) = self.online.as_mut() else { return };
        online.health = snapshot.others.iter().map(|other| (other.id, other.health)).collect();
        online.others_history.push_back((snapshot.tick, snapshot.others));
        while online.others_history.len() > OTHERS_HISTORY_SNAPSHOTS {
            online.others_history.pop_front();
        }
        // The rings Exetior may not place another near, for the prediction.
        self.game.world.black_rings = snapshot
            .entities
            .iter()
            .filter_map(|entity| match *entity {
                EntityView::BlackRing { id, x, y, alpha } => Some(crate::core::black_ring::BlackRing { id, x: x as f64, y: y as f64, alpha: alpha as f64, owner: None }),
                _ => None,
            })
            .collect();
        // The rings the server put out: the game only draws them and lets its own
        // player walk into them, it never puts out rings of its own in a round.
        self.game.world.rings = snapshot
            .entities
            .iter()
            .filter_map(|entity| match *entity {
                EntityView::Ring { x, y, red, alpha } => {
                    let mut ring = crate::core::rings::MapRing::new(x as f64, y as f64, red);
                    ring.alpha = alpha as f64;
                    Some(ring)
                }
                _ => None,
            })
            .collect();
        online.entities = snapshot.entities;
        snapshot.level.apply(&mut self.game.world);
        let Some(mut own) = snapshot.own else {
            self.game.players.clear();
            return;
        };
        if let Some(acked) = snapshot.acked_input {
            while online.inputs.front().is_some_and(|&(tick, _)| tick <= acked) {
                online.inputs.pop_front();
            }
        }
        // Replayed ticks already made their sounds when they were predicted.
        let mut replayed_events = Vec::new();
        for &(_, buttons) in &online.inputs {
            own.tick(&self.game.world, buttons, &mut replayed_events);
        }
        if self.camera.is_none() {
            self.camera = Some(Camera::new(&own));
        }
        // --netlog: how far the server moved the player this client had predicted.
        if context.netlog {
            if let Some(predicted) = self.game.players.first() {
                let (dx, dy) = (own.x - predicted.x, own.y - predicted.y);
                if dx.abs() + dy.abs() > 0.001 {
                    eprintln!("tick {} correction ({dx:.3}, {dy:.3}) unconfirmed {} acked {:?}", online.next_tick, online.inputs.len(), snapshot.acked_input);
                }
            }
        }
        self.game.players = vec![own];
    }

    fn send_buttons(&mut self, context: &mut Context, buttons: Buttons) {
        // A spectator plays nobody, so it has no buttons to report.
        if self.spectating {
            return;
        }
        let Some(online) = self.online.as_mut() else { return };
        let tick = online.next_tick;
        online.next_tick += 1;
        if context.net.is_server_ready && tick % crate::core::config::ticks(PING_EVERY_SECONDS) as u32 == 0 {
            context.net.send_unreliable(&crate::packet::Packet::new(crate::packet::PacketType::CLIENT_PING));
        }
        online.inputs.push_back((tick, buttons));
        while online.inputs.len() > MAX_UNCONFIRMED_INPUTS {
            online.inputs.pop_front();
        }
        let first_sent = online.inputs.len().saturating_sub(INPUTS_PER_PACKET);
        let recent: Vec<Buttons> = online.inputs.iter().skip(first_sent).map(|&(_, buttons)| buttons).collect();
        context.net.send_round_input(tick, &recent);
    }

    /// A spectator plays nobody, so the keys are the camera's: any key follows the next
    /// player, the second ability key turns the free camera on and off, and the chat is
    /// opened where it stands, as it is in the lobby, instead of in the pause menu.
    fn step_spectator(&mut self, context: &mut Context) {
        if context.input.pressed(KeyCode::Enter) {
            self.chat.toggle(context);
        }
        if !self.chat.is_open && context.input.pressed(KeyCode::T) {
            self.chat.open(&mut context.input);
        }
        if self.chat.is_open {
            self.chat.type_message(&mut context.input, pause::CHAT_MAX_LENGTH);
            return;
        }
        if context.input.pressed(context.options.keys.special2.0) {
            if let Some(camera) = self.camera.as_mut() {
                camera.toggle_free();
            }
        }
    }

    /// SERVER_HDDOOR_STATE: every door sounds as it starts to move, and every crystal chimes
    /// as it goes to rest.
    fn hear_doors(&mut self, context: &mut Context) {
        let world = &self.game.world;
        let Some(crystal) = world.ids_of(ObjectId::HdCrystal).next() else { return };
        let ObjectVars::Crystal { lit: open, usable } = world.instances[crystal].vars else { return };
        let Some(camera) = &self.camera else { return };
        if open != self.doors_seen.0 {
            for door in world.ids_of(ObjectId::HdDoor).chain(world.ids_of(ObjectId::HdDoor2)) {
                play_sound_near_ear(context, camera, sound::SND_DOOR, world.instances[door].x, world.instances[door].y);
            }
        }
        if self.doors_seen.1 && !usable {
            for crystal in world.ids_of(ObjectId::HdCrystal) {
                play_sound_near_ear(context, camera, sound::SND_MESSAGE, world.instances[crystal].x, world.instances[crystal].y);
            }
        }
        self.doors_seen = (open, usable);
    }

    /// obj_blackring Step: every black ring hums, loudest near the view.
    fn hum_black_rings(&mut self, context: &mut Context) {
        let rings: Vec<(u16, f64, f64)> = self
            .entity_views()
            .iter()
            .filter_map(|entity| match *entity {
                EntityView::BlackRing { id, x, y, .. } => Some((id, x as f64, y as f64)),
                _ => None,
            })
            .collect();
        if let Some(camera) = &self.camera {
            let listener = (camera.x + VIEW_WIDTH / 2.0, camera.y + VIEW_HEIGHT / 2.0);
            for &(id, x, y) in &rings {
                let gain = heard_gain_within((camera.x + LEFT_EAR.0, camera.y + LEFT_EAR.1), x, y, BLACK_RING_HUM_REACH);
                context.audio.set_emitter(("black ring", id as u32), sound::SND_BLACKRING_BALL, gain, pan_towards(listener, x, y));
            }
        }
        context.audio.keep_emitters("black ring", |number| rings.iter().any(|&(id, _, _)| id as u32 == number));
    }

    /// SERVER_PFLIFT_STATE 2: when a lift lets its rider off, every lift sounds.
    fn hear_lifts(&mut self, context: &mut Context) {
        let world = &self.game.world;
        let lifts: Vec<InstanceId> = world.ids_of(ObjectId::PfLift).collect();
        let carrying: Vec<bool> = lifts.iter().map(|&lift| matches!(world.instances[lift].vars, ObjectVars::Lift { carrying: true, .. })).collect();
        let arrived = carrying.iter().zip(&self.lifts_carrying).any(|(&now, &before)| before && !now);
        if let (true, Some(camera)) = (arrived, &self.camera) {
            for &lift in &lifts {
                play_sound_near_ear(context, camera, sound::SND_LIFT, world.instances[lift].x, world.instances[lift].y);
            }
        }
        self.lifts_carrying = carrying;
    }

    /// SERVER_KAFMONITOR_STATE 2: a monitor that just broke bangs and explodes.
    fn show_breaking_monitors(&mut self, context: &mut Context) {
        let world = &self.game.world;
        let broken: Vec<InstanceId> = world
            .ids_of(ObjectId::KafSpeedbox)
            .filter(|&monitor| matches!(world.instances[monitor].vars, ObjectVars::Monitor { broken: true, .. }))
            .collect();
        for &monitor in broken.iter().filter(|monitor| !self.broken_monitors.contains(monitor)) {
            let (x, y) = (world.instances[monitor].x, world.instances[monitor].y);
            if let Some(camera) = &self.camera {
                play_sound(context, camera, sound::SND_BREAK, x, y - SOUND_EMITTER_ABOVE, false, None);
            }
            self.effects.push(Effect::quick(sprite::SPR_EXPLOSION, x + MONITOR_EXPLOSION_OFFSET, y + MONITOR_EXPLOSION_OFFSET, 1.0, EFFECT_SPEED, C_WHITE));
        }
        self.broken_monitors = broken;
    }

    /// SimEvent::TrackerCaught: the tracker shows its catch to the tracking Eggman's own
    /// side (the survivors, or the killers when a demon tracked).
    /// The side comes from the health row, which the server sends about everyone: a
    /// tracking Eggman far away is left out of the view but still has a side.
    fn tracker_caught(&mut self, context: &Context, eggman: usize, victim: usize) {
        let round = context.net.round_setup.as_ref();
        let id_of = |index: usize| round.and_then(|setup| setup.players.get(index).copied());
        let own = self.game.players.first();
        let side_of = |index: usize| {
            if Some(index) == self.own_index {
                return own.map(Player::is_killers_side);
            }
            let id = id_of(index)?;
            let online = self.online.as_ref()?;
            online.health.iter().find(|(other, _)| *other == id).map(|(_, health)| health.is_killers_side())
        };
        let (Some(tracker_side), Some(own_side), Some(victim_id)) = (side_of(eggman), own.map(Player::is_killers_side), id_of(victim)) else { return };
        if tracker_side == own_side {
            self.indicators.tracker_caught(victim_id);
        }
    }

    /// The heal gauges this player sees over hurt teammates; the server does the healing.
    /// obj_tails Step_2 and obj_player_puppet Step_0: the flying sound stops once Tails
    /// is on the ground. The original stopped it for any Tails not flying, so a second
    /// Tails standing cut off the first one's flight; here it stops when nobody flies.
    fn stop_fly_sound_when_nobody_flies(&self, context: &mut Context) {
        let flies = |player: &Player| player.character == Character::Tails && player.state == crate::core::player::tails::TAILS_FLY;
        let own_flies = self.game.players.first().is_some_and(flies);
        let other_flies = self.online.as_ref().is_some_and(|online| online.others.iter().any(|(_, player)| flies(player)));
        if !own_flies && !other_flies {
            context.audio.stop(sound::SND_TAILS_FLY);
        }
    }

    fn show_heal_gauges(&mut self) {
        self.heal_gauges.clear();
        let (Some(online), Some(own)) = (&self.online, self.game.players.first()) else { return };
        let config = self.game.world.config.clone();
        for (id, teammate) in &online.others {
            let progress = self.heal_progress.entry(*id).or_insert(0.0);
            let step = crate::core::heal::heal_step(&self.game.world, &config, progress, own, teammate);
            if step.gauge_shown {
                self.heal_gauges.push((teammate.x.floor(), teammate.y.floor() - HEAL_GAUGE_ABOVE, *progress));
            }
        }
    }

    /// net_send_palette on SERVER_GAME_PLAYERS_READY, and again after demonizing.
    /// What the achievements need to know about this player and the round right now.
    fn achievement_situation(&self, context: &Context) -> crate::client::achievements::Situation {
        let (character, exe_character) = own_character(context);
        let own = self.game.players.first();
        crate::client::achievements::Situation {
            room: self.room_id,
            character,
            exe_character,
            demonized: own.is_some_and(|player| player.is_demonized()),
            hp: own.map_or(0, |player| player.hp),
            seconds_left: (self.hud.clock.minutes.max(0) * 60 + self.hud.clock.seconds.max(0)) as u32,
            full_demon_shot: self.game.world.config.tails.demonized_damage.iter().copied().max().unwrap_or(i32::MAX),
        }
    }

    /// Whether each player of the round, by their index there, is EXE.
    fn exe_by_round_index(&self, context: &Context) -> Vec<bool> {
        let Some(setup) = context.net.round_setup.as_ref() else { return Vec::new() };
        setup
            .players
            .iter()
            .enumerate()
            .map(|(index, id)| match self.online.as_ref().and_then(|online| online.others.iter().find(|(other, _)| other == id)) {
                Some((_, other)) => other.character == Character::Exe,
                None => Some(index) == self.own_index && own_character(context).0 == Character::Exe,
            })
            .collect()
    }

    /// This player as the achievements watch them every step: rings, lost health, and
    /// coming back as a demon.
    fn watch_own_for_achievements(&mut self, context: &mut Context) {
        let Some(own) = self.game.players.first() else { return };
        context.achievements.rings(own.rings);
        if let Some((hp, revival_times)) = self.own_last_seen {
            if own.hp < hp && !own.is_demonized() {
                context.achievements.hurt();
            }
            if own.revival_times >= 2 && revival_times < 2 {
                context.achievements.demonized();
            }
            if own.hp <= 0 && hp > 0 {
                context.achievements.died();
            }
        }
        self.own_last_seen = Some((own.hp, own.revival_times));
    }

    /// net_send_palette sends the pet's colours too, once, when this player has a pet.
    fn send_pet_skin_once(&mut self, context: &mut Context) {
        let Some(pet) = &self.pet else { return };
        if !context.net.is_server_ready || self.sent_pet_skin {
            return;
        }
        let Some(colours) = context.unlockables.pet_palettes.get(pet.palette()).cloned() else { return };
        context.net.send_pet_skin(&colours.from, &colours.to);
        self.sent_pet_skin = true;
    }

    fn send_skin_when_changed(&mut self, context: &mut Context) {
        let Some(player) = self.game.players.first() else { return };
        let table = skin_table(player);
        if !context.net.is_server_ready || self.sent_skin_table == Some(table) {
            return;
        }
        let Some(skin) = context.unlockables.skins.iter().find(|skin| skin.table == table) else { return };
        let (name, from, to) = (skin.name.clone(), skin.from.clone(), skin.to.clone());
        context.net.send_skin(&name, &from, &to);
        self.sent_skin_table = Some(table);
    }

    /// SERVER_GAME_SPAWN_RING: the big ring appears with the chase music, or gets ready.
    fn big_ring_changed(&mut self, context: &mut Context, ready: bool, spawn: u8) {
        if let Some((_, _, big_ring_ready, _)) = self.big_ring.as_mut() {
            *big_ring_ready = ready;
            return;
        }
        if ready {
            return;
        }
        match self.room_id {
            RoomId::Act9 => context.audio.crossfade(true, ACT9_CHASE_FADE_SECONDS),
            _ => context.audio.play_music(LEVELS[self.level_number].chase_music),
        }
        let world = &self.game.world;
        let spawns: Vec<InstanceId> = world.ids_of(ObjectId::Ringspawn).collect();
        let Some(&spawn) = spawns.get(spawn as usize % spawns.len().max(1)) else { return };
        self.big_ring = Some((world.instances[spawn].x, world.instances[spawn].y, false, 0.0));
        self.hud.escape = true;
    }

    /// SERVER_RING_STATE 1: the ring leaves with a sparkle that EXE does not see.
    /// SERVER_GAME_EXE_WINS, SERVER_GAME_SURVIVOR_WIN, SERVER_GAME_TIME_OVER.
    fn round_ended(&mut self, context: &mut Context, ending: RoundEnding) {
        let (own_character, _) = own_character(context);
        let own = self.game.players.first();
        let demon = own_character == Character::Exe || own.is_some_and(|player| player.revival_times >= 2);
        let (sound, card) = match ending {
            RoundEnding::ExeWon => (sound::SND_EXE_WINS, if demon { sprite::SPR_YOUKILLEDEVERYONE } else { sprite::SPR_GAMEOVER }),
            RoundEnding::TimeOver => (sound::SND_EXE_WINS, sprite::SPR_TIMEOVER),
            RoundEnding::SurvivorsEscaped => {
                let lost = demon || own.is_some_and(|player| player.hp <= 0);
                (sound::SND_SURVIVOR_WIN, if lost { sprite::SPR_GAMEOVER } else { sprite::SPR_SURVIVORSESCAPED })
            }
        };
        context.audio.play(sound, false);
        self.ending = Some((card, 0.0));
        self.game.end_round(ending);
        // The camera watches the round's EXE until the level closes.
        if let (Some(camera), Some(&exe)) = (self.camera.as_mut(), context.net.exe_ids.first()) {
            if Some(exe) != self.own_id {
                camera.mode = CameraMode::Spectate;
                camera.spectating = Some(exe);
                camera.locked = true;
            }
        }
        self.ending_fade_alarm = crate::core::config::ticks(ENDING_FADE_AFTER_SECONDS);
    }

    /// An event the server's simulation made that this client did not predict.
    fn on_server_event(&mut self, context: &mut Context, source: EventSource, event: &SimEvent) {
        let about_this_player = matches!(source, EventSource::DoneTo(victim) if Some(victim) == self.own_index);
        if event.is_private() && !about_this_player {
            return;
        }
        let player = match source {
            EventSource::Player(index) | EventSource::DoneTo(index) => context.net.round_setup.as_ref().and_then(|setup| setup.players.get(index).copied()),
            EventSource::World => None,
        };
        let now = self.achievement_situation(context);
        let exe_by_index = self.exe_by_round_index(context);
        let own_index = self.own_index;
        context.achievements.on_event(event, |index| Some(index) == own_index, about_this_player, |index| exe_by_index.get(index).copied().unwrap_or(false), &now);
        self.on_event(context, event, about_this_player, player);
    }

    /// What the client does with the simulation's events.
    /// `own`: the event is about this player (predicted, or done to it on the server);
    /// `player`: the id of the player whose step made it, when the server says so.
    fn on_event(&mut self, context: &mut Context, event: &SimEvent, own: bool, player: Option<u16>) {
        let act9 = self.room_id == RoomId::Act9;
        match *event {
            SimEvent::Effect { sprite, x, y, xscale, image_speed, yspd } => {
                // scr_amy_special: the hearts' irandom_range(-3, 3), left out of the simulation.
                let jitter = if matches!(sprite, sprite::SPR_ROSEHEART | sprite::SPR_EROSEHEART) { rand::gen_range(-3, 4) as f64 } else { 0.0 };
                let mut effect = Effect::quick(sprite, x, y + jitter, xscale, image_speed, effect_blend(act9));
                effect.set_yspd(yspd);
                self.effects.push(effect);
            }
            SimEvent::RingBroke { x, y, xspd } => self.ring_parts.extend(ring_pieces(x, y, xspd, act9)),
            // SERVER_RING_COLLECTED: the ring leaves with a sparkle that EXE does not see.
            SimEvent::RingTaken { x, y, .. } if own_character(context).0 != Character::Exe => {
                let mut sparkle = Effect::quick(sprite::SPR_RING_SPARKLE, x, y, 1.0, EFFECT_SPEED, effect_blend(act9));
                sparkle.depth = RING_DEPTH;
                self.effects.push(sparkle);
            }
            SimEvent::HidingWarning { shown } => {
                match (shown, self.hiding_warning.as_mut()) {
                    (true, None) => self.hiding_warning = Some(hiding_warning::HidingWarning::new(&mut context.audio)),
                    (false, Some(warning)) => warning.leave(),
                    _ => {}
                }
            }
            SimEvent::Lightning => {
                self.flash = 1.0;
                context.audio.play(sound::SND_THUNDER, false);
            }
            SimEvent::HealSparkles { x, y } => self.heal_sparkles.push(HealSparkle::new(x, y)),
            SimEvent::RedRingStarted => {
                context.audio.play(sound::MUS_MINDFUCK, false);
                self.red_screen = Some(0.0);
            }
            SimEvent::RedRingEnded => {
                context.audio.stop(sound::MUS_MINDFUCK);
                self.red_screen = None;
            }
            SimEvent::Trail { sprite, image_index, x, y, xscale } => {
                let (depth, colours) = match self.game.players.first().filter(|_| own) {
                    Some(own_player) => (OWN_PLAYER_DEPTH + TRAIL_BEHIND, own_skin(context, own_player)),
                    None => (self.others_depth + TRAIL_BEHIND, player.map_or_else(|| (palette::default_colours(), palette::default_colours()), |id| other_skin(&context.net.player_skins, id))),
                };
                self.effects.push(Effect::trail(sprite, image_index, x, y, xscale, depth, colours, effect_blend(act9)));
            }
            SimEvent::ChaosLiquid { x, y } => {
                let (depth, colours) = match self.game.players.first().filter(|_| own) {
                    Some(own_player) => (OWN_PLAYER_DEPTH + TRAIL_BEHIND, own_skin(context, own_player)),
                    None => (self.others_depth + TRAIL_BEHIND, player.map_or_else(|| (palette::default_colours(), palette::default_colours()), |id| other_skin(&context.net.player_skins, id))),
                };
                // obj_chaos_liquid Create: spd = choose(4, 4.5, 5, 5.5, 6, 6.5), and the
                // piece comes off a little above or below where Chaos is.
                let speed = LIQUID_SPEEDS[macroquad::rand::gen_range(0, LIQUID_SPEEDS.len())];
                let jitter = macroquad::rand::gen_range(-LIQUID_JITTER, LIQUID_JITTER + 1) as f64;
                self.effects.push(Effect::liquid(sprite::SPR_CHAOS_LIQUID, x, y + jitter, speed, depth, colours, effect_blend(act9)));
            }
            _ => {}
        }
        let wrap_width = (self.room_id == RoomId::Majongforest).then_some(self.game.world.room_width);
        let Some(camera) = self.camera.as_mut() else { return };
        match *event {
            SimEvent::LocalSound { sound } => context.audio.play(sound, false),
            SimEvent::TeleportFlash => self.teleport_flash = 1.0,
            SimEvent::SpikesMove => {
                // SERVER_MOVINGSPIKE_STATE: every spike sound emitter of the map, from 16 px above it.
                let world = &self.game.world;
                for emitter in world.ids_of(ObjectId::Soundemitter) {
                    if world.instances[emitter].vars == (ObjectVars::SoundEmitter { kind: SOUND_EMITTER_MOVING_SPIKES }) {
                        let (x, y) = (world.instances[emitter].x, world.instances[emitter].y - SOUND_EMITTER_ABOVE);
                        play_sound(context, camera, sound::SND_MOVINGSPIKE, x, y, false, wrap_width);
                    }
                }
            }
            SimEvent::Sound { sound, x, y } => play_sound(context, camera, sound, x, y, own, wrap_width),
            SimEvent::Taunt { exe, emotion, x, y } if !voices::voice_playing(&context.audio) => {
                play_sound(context, camera, voices::taunt(exe, emotion), x, y, own, wrap_width);
            }
            SimEvent::ExeVanished { x, y } => {
                if own && !voices::voice_playing(&context.audio) {
                    context.audio.play(voices::vanish(), false);
                }
                play_sound(context, camera, sound::SND_EXE_APPEAR, x, y, own, wrap_width);
            }
            SimEvent::ExeAppeared { x, y } => {
                play_sound(context, camera, sound::SND_EXE_APPEAR, x, y, own, wrap_width);
                if !voices::voice_playing(&context.audio) {
                    play_sound(context, camera, voices::appear(), x, y, own, wrap_width);
                }
            }
            SimEvent::SpawnBlackRing { x, y } if !voices::voice_playing(&context.audio) => {
                play_sound(context, camera, voices::black_ring(), x, y, own, wrap_width);
            }
            SimEvent::SpawnExellerClone { x, y, .. } if !voices::voice_playing(&context.audio) => {
                if let Some(line) = voices::clone_line() {
                    play_sound(context, camera, line, x, y, own, wrap_width);
                }
            }
            SimEvent::KilledByExe { killer, .. } if !voices::voice_playing(&context.audio) => {
                // net_sound_emit_at: from the killer's puppet, and plainly for the killer itself.
                let own_killer = self.game.players.first().filter(|_| Some(killer) == self.own_index);
                let killer_id = context.net.round_setup.as_ref().and_then(|setup| setup.players.get(killer).copied());
                let other_killer = || {
                    let (_, views) = self.online.as_ref()?.others_history.back()?;
                    let view = views.iter().find(|other| Some(other.id) == killer_id)?.view.as_ref()?;
                    Some((view.exe_character, view.x as f64, view.y as f64, false))
                };
                let killer = own_killer.map(|player| (player.exe_character, player.x, player.y, true)).or_else(other_killer);
                if let Some((exe, x, y, is_own)) = killer {
                    play_sound(context, camera, voices::kill(exe), x, y, is_own, wrap_width);
                }
            }
            SimEvent::DummyHit { worth, x, y, .. } => {
                let sounds = [sound::SND_DUMMY, sound::SND_DUMMY2, sound::SND_DUMMY3];
                context.audio.play(sounds[macroquad::rand::gen_range(0, sounds.len())], false);
                if let Some(fart_zone) = self.fart_zone.as_mut() {
                    fart_zone.show_hit(worth, x, y);
                }
            }
            // obj_rmzsonic destroy()
            SimEvent::SlugSquashed { x, y, facing_right } => {
                play_sound_near_ear(context, camera, sound::SND_SLIME, x, y);
                let mut death = Effect::quick(sprite::SPR_RAVINEMIST_SONICDEAD, x, y, if facing_right { 1.0 } else { -1.0 }, 1.0, C_WHITE);
                death.depth = ravine_mist::SLUG_DEPTH;
                self.effects.push(death);
            }
            SimEvent::SpringEcho { x, y } => self.indicators.spring_echo(x, y),
            SimEvent::SpawnBlackRing { .. } if own => self.indicators.black_ring_placed(),
            SimEvent::TrackerCaught { eggman, victim } => self.tracker_caught(context, eggman, victim),
            SimEvent::ControlsReversed { x, y } => {
                if let Some(echidna_ruins) = self.echidna_ruins.as_mut() {
                    echidna_ruins.reversed(x, y);
                }
            }
            SimEvent::PotatoBoom => {
                context.audio.play(sound::SND_BOOM, false);
                if let Some(fart_zone) = self.fart_zone.as_mut() {
                    fart_zone.boom();
                }
            }
            SimEvent::StageSwitched { .. } => {
                context.audio.play(sound::SND_NPTELEPORT, false);
                if let Some(not_perfect) = self.not_perfect.as_mut() {
                    not_perfect.switched();
                }
            }
            SimEvent::TailsDollNoticed { .. } => context.audio.play(sound::SND_TAILSBALL, false),
            SimEvent::TailsDollChases { .. } => context.audio.play(sound::SND_TAILSBALL_CHASE, false),
            SimEvent::TailsDollCaught { x, y, .. } => {
                if own {
                    context.audio.play(sound::SND_TAILSBALL_JUMPSCARE, false);
                    if let Some(dark_tower) = self.dark_tower.as_mut() {
                        dark_tower.scare();
                    }
                } else {
                    play_sound(context, camera, sound::SND_TAILSBALL_JUMPSCARE2, x, y, false, wrap_width);
                }
            }
            SimEvent::StopSound { sound } => context.audio.stop(sound),
            SimEvent::CameraShake => camera.shake(HIT_SHAKE.0, HIT_SHAKE.1, HIT_SHAKE.2),
            SimEvent::CameraOnPlayer => {
                if let Some(player) = self.game.players.first() {
                    camera.snap_to(player);
                }
            }
            // scr_player_instakill
            SimEvent::PlayerDied { .. } if own => camera.mode = CameraMode::Fade,
            SimEvent::BloodScreen { .. } => self.hud.blood_fade = 1.0,
            SimEvent::SmallCameraShake => camera.shake(SMALL_SHAKE.0, SMALL_SHAKE.1, SMALL_SHAKE.2),
            _ => {}
        }
    }

    fn draw(&mut self, context: &mut Context, current_time_ms: f64) {
        self.draw_world(context, current_time_ms);
        self.draw_gui(context, current_time_ms);
        // A spectator's chat is drawn where the lobby draws it, not inside the pause menu.
        if self.spectating {
            self.chat.draw(&mut context.canvas, pause::CHAT_LINES_TOP, pause::CHAT_BOX_TOP);
        }
        self.pause.draw(context, &self.chat, self.online.is_some());
    }

    /// Draw GUI: names and health above players, then obj_level's overlay.
    fn draw_gui(&mut self, context: &mut Context, current_time_ms: f64) {
        let view = self.camera.as_ref().map_or((0.0, 0.0), |camera| (camera.x, camera.y));
        let net = &context.net;
        let own_id = net.id.unwrap_or(0);
        let (own_character, _) = own_character(context);
        let own = HudPlayer {
            nickname: &context.options.nickname,
            player: self.game.players.first(),
            health: self.game.players.first().map(HealthView::of),
            character: own_character,
            is_exe: own_character == Character::Exe,
            death_timer: net.death_timers.get(&own_id).copied(),
            ping_ms: net.ping as i32,
        };
        let round_ids = net.round_setup.as_ref().map_or(&[][..], |setup| &setup.players[..]);
        let others: Vec<HudPlayer> = round_ids
            .iter()
            .filter(|&&id| id != own_id)
            .map(|&id| {
                let known = net.players.get(&id);
                let online = self.online.as_ref();
                let player = online.and_then(|online| online.others.iter().find(|(other, _)| *other == id)).map(|(_, player)| player);
                let health = online.and_then(|online| online.health.iter().find(|(other, _)| *other == id)).map(|(_, health)| *health);
                HudPlayer {
                    nickname: known.map_or("", |known| known.nickname.as_str()),
                    player,
                    health,
                    character: health.map_or(Character::Tails, |health| health.character),
                    is_exe: net.exe_ids.contains(&id),
                    death_timer: net.death_timers.get(&id).copied(),
                    ping_ms: net.pings.get(&id).copied().unwrap_or(0) as i32,
                }
            })
            .collect();

        let canvas = &mut context.canvas;
        if let Some(dark_tower) = self.dark_tower.as_mut() {
            dark_tower.draw_gui(canvas, current_time_ms);
        }
        if let Some(fart_zone) = &self.fart_zone {
            fart_zone.draw_gui(canvas);
        }
        if let Some(torture_cave) = &self.torture_cave {
            torture_cave.draw_gui(canvas);
        }
        if let Some(dotdotdot) = self.dotdotdot.as_mut() {
            dotdotdot.draw_gui(canvas);
        }
        if let Some(player) = own.player {
            let online = self.online.as_ref();
            let shown = indicators::View {
                own: player,
                others: online.map_or(&[][..], |online| &online.others[..]),
                entities: online.map_or(&[][..], |online| &online.entities[..]),
                own_id: self.own_id,
                view,
                big_ring: self.big_ring.map(|(x, y, _, _)| (x, y)),
                config: &self.game.world.config,
            };
            self.indicators.draw(canvas, &self.game.world, &shown);
        }
        if let Some(ravine_mist) = &self.ravine_mist {
            ravine_mist.draw_gui(canvas);
        }
        if let Some(echidna_ruins) = &self.echidna_ruins {
            echidna_ruins.draw_gui(canvas, self.game.players.first(), view);
        }
        if let Some(weed_zone) = &self.weed_zone {
            weed_zone.draw_gui(canvas, view, own_character == Character::Exe, current_time_ms);
        }
        if let Some(not_perfect) = self.not_perfect.as_ref().filter(|not_perfect| not_perfect.white > 0.0) {
            canvas.draw_sprite_ext(sprite::SPR_WHITE, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, C_WHITE, not_perfect.white);
        }
        let world = &self.game.world;
        let plate_shown = |x: f64, y: f64| self.desert_town.as_ref().is_none_or(|desert_town| desert_town.shows_plate_at(world, x, y));
        for other in others.iter().filter(|other| other.player.is_some_and(|player| plate_shown(player.x, player.y))) {
            self.hud.draw_above_other(canvas, other, view);
        }
        for &(x, y, progress) in &self.heal_gauges {
            let frame = progress * canvas.sprites.get(sprite::SPR_HEAL).frame_count as f64;
            canvas.draw_sprite(sprite::SPR_HEAL, frame, (x - view.0).floor(), (y - view.1).floor());
        }
        for entity in self.entity_views() {
            let EntityView::ExellerClone { owner, slot, x, y, .. } = entity else { continue };
            if !plate_shown(x as f64, y as f64) {
                continue;
            }
            let (x, y) = ((x as f64 - view.0).floor(), (y as f64 - view.1).floor());
            if own_character == Character::Exe {
                let bob = (current_time_ms / 200.0).sin();
                canvas.draw_sprite(sprite::SPR_EXELLER_CLONEARROW, slot as f64, x, (y - CLONE_ARROW_ABOVE - bob).floor());
            } else {
                let name = net.players.get(&owner).map_or("", |known| known.nickname.as_str());
                let colour = if self.room_id == RoomId::Act9 { C_DKGRAY } else { crate::client::canvas::C_RED };
                draw_text(canvas, x - text_width(canvas, name) / 2.0, y - CLONE_NAME_ABOVE, name, colour, 1.0);
            }
        }
        if let Some(player) = own.player {
            self.hud.draw_above_own(canvas, player, view);
        }
        if let Some(fade) = &self.black_fade {
            fade.draw_gui(canvas);
        }
        let player_list_held = context.input.held(context.options.keys.player_list.0);
        self.hud.draw(canvas, &own, &others, &context.options, current_time_ms, player_list_held);
        if let Some(player) = own.player {
            let keys = &context.options.keys;
            let up_or_down_held = context.input.held(keys.up.0) || context.input.held(keys.down.0);
            hud::draw_ability_gauges(canvas, player, &self.game.world.config, player.controls_enabled, up_or_down_held, self.hud.shown);
        }
        if self.flash > 0.0 {
            canvas.draw_sprite_ext(sprite::SPR_WHITE, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, C_WHITE, self.flash);
        }
        if self.teleport_flash > 0.0 {
            canvas.draw_sprite_ext(sprite::SPR_WHITE, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, C_WHITE, self.teleport_flash);
            self.teleport_flash -= TELEPORT_FLASH_FADE * step();
        }
        if self.room_id == RoomId::Majongforest {
            for other in &others {
                self.hud.draw_wrapped_above_other(canvas, other, view, self.game.world.room_width);
            }
        }
        if let Some(warning) = self.hiding_warning.as_mut() {
            if !warning.draw(canvas, &mut context.audio) {
                self.hiding_warning = None;
            }
        }
        if let Some((fade, hold)) = self.sudden_death.as_mut() {
            if *hold > 0 {
                *hold -= 1;
                *fade = (*fade + SUDDEN_DEATH_FADE_IN * step()).min(ENDING_MAX_FADE);
            } else {
                *fade -= SUDDEN_DEATH_FADE_OUT * step();
            }
            let frame_count = canvas.sprites.get(sprite::SPR_SUDDEN_DEATH).frame_count as f64;
            let frame = (current_time_ms / ENDING_FRAME_MILLISECONDS).floor() % frame_count;
            let scale = 1.5 - *fade;
            canvas.draw_sprite_ext(sprite::SPR_BLACK, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, C_WHITE, *fade);
            canvas.draw_sprite_ext(sprite::SPR_SUDDEN_DEATH, frame, 240.0, 135.0, scale, scale, 0.0, C_WHITE, *fade * 2.0);
            if *fade <= 0.0 {
                self.sudden_death = None;
            }
        }
        if let Some((card, fade)) = self.ending.as_mut() {
            if *fade < ENDING_MAX_FADE {
                *fade += ENDING_FADE_PER_STEP * step();
            }
            let frame_count = canvas.sprites.get(*card).frame_count as f64;
            let frame = (current_time_ms / ENDING_FRAME_MILLISECONDS).floor() % frame_count;
            let scale = 1.5 - *fade;
            canvas.draw_sprite_ext(sprite::SPR_BLACK, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, C_WHITE, *fade);
            canvas.draw_sprite_ext(*card, frame, 240.0, 135.0, scale, scale, 0.0, C_WHITE, *fade * 2.0);
        }
    }

    fn draw_world(&self, context: &mut Context, current_time_ms: f64) {
        let own_skin = self.game.players.first().map(|player| own_skin(context, player));
        let (canvas, net, unlockables) = (&mut context.canvas, &context.net, &context.unlockables);
        let (view_x, view_y) = self.camera.as_ref().map_or((0.0, 0.0), |camera| (camera.x, camera.y));
        let growth = self.dark_tower.as_ref().map_or(0.0, |dark_tower| dark_tower.view_growth);
        canvas.set_view_sized(view_x, view_y, VIEW_WIDTH + growth, VIEW_HEIGHT + growth);

        // Players and effects stand on layers made by instance_create_depth, which
        // are drawn before the room's layers of the same depth.
        let mut dynamic = vec![
            (self.others_depth, Dynamic::Others),
            (OWN_PLAYER_DEPTH, Dynamic::OwnPlayer),
            (RING_DEPTH, Dynamic::Rings),
            (OWN_PLAYER_DEPTH, Dynamic::Pet),
            (RED_SCREEN_DEPTH, Dynamic::RedScreen),
            (OWN_PLAYER_DEPTH + BIG_RING_BEHIND, Dynamic::BigRing),
            // obj_tails_projectile shares the players' layer and was created after them.
            (OWN_PLAYER_DEPTH, Dynamic::Entities(EntityKind::TailsShot)),
            (EGG_TRACKER_DEPTH, Dynamic::Entities(EntityKind::EggTracker)),
            (STOMP_WAVE_DEPTH, Dynamic::Entities(EntityKind::StompWave)),
            (EXELLER_CLONE_DEPTH, Dynamic::Entities(EntityKind::ExellerClone)),
            (BLACK_RING_DEPTH, Dynamic::Entities(EntityKind::BlackRing)),
            (HEAL_SPARKLE_DEPTH, Dynamic::HealSparkles),
            (REVIVAL_SIGN_DEPTH, Dynamic::RevivalSigns),
            (green_hill::RAIN_DEPTH, Dynamic::Rain),
            (you_cant_run::SMOKE_DEPTH, Dynamic::Smoke),
            (nasty_paradise::SNOW_DEPTH, Dynamic::Snow),
            (nasty_paradise::ICE_SHARD_DEPTH, Dynamic::IceShards),
            (nasty_paradise::SNOWBALL_PIECE_DEPTH, Dynamic::SnowballPieces),
            (volcano_valley::SHARD_DEPTH, Dynamic::VaseShards),
            (dark_tower::DRIP_DEPTH, Dynamic::Drips),
            (not_perfect::PARTICLE_DEPTH, Dynamic::SkyShapes),
            (weed_zone::GHOST1_DEPTH, Dynamic::Ghosts(false)),
            (fart_zone::TEXT_DEPTH, Dynamic::DummyTexts),
            (ravine_mist::SHARD_DEPTH, Dynamic::Shards),
            (STATIC_DEPTH, Dynamic::Static),
            (limp_city::RED_DEPTH, Dynamic::RedCity),
            (ravine_mist::SLUG_DEPTH, Dynamic::Slugs),
            (weed_zone::GHOST2_DEPTH, Dynamic::Ghosts(true)),
            (dark_tower::SHARD_DEPTH, Dynamic::StalactiteShards),
        ];
        dynamic.extend(self.depth_instances.iter().map(|&(depth, id)| (depth, Dynamic::Instance(id))));
        // After the eyes, which share their depth.
        dynamic.push((limp_city::PUPIL_DEPTH, Dynamic::Pupils));
        dynamic.extend(self.effects.iter().enumerate().map(|(index, effect)| (effect.depth, Dynamic::Effect(index))));
        dynamic.sort_by_key(|&(depth, _)| std::cmp::Reverse(depth));
        let mut dynamic = dynamic.into_iter().peekable();
        for (index, layer) in self.room.layers.iter().enumerate() {
            while let Some((_, item)) = dynamic.next_if(|&(depth, _)| layer.depth <= depth) {
                self.draw_dynamic(canvas, item, own_skin.as_ref(), net, unlockables, current_time_ms);
            }
            if !layer.visible {
                continue;
            }
            // The layer's own filters: _filter_tintfilter and _filter_greyscale.
            canvas.set_layer_tint(layer.tint);
            if layer.greyscale {
                canvas.set_black_white();
            }
            self.room.draw_layer(canvas, layer);
            if let LayerContent::Instances(_) = layer.content {
                for &id in &self.layer_instances[index] {
                    draw_instance(canvas, &self.game.world, id, self.game.players.first(), current_time_ms);
                }
            }
            canvas.set_layer_tint(C_WHITE);
            if layer.greyscale {
                canvas.reset_shader();
            }
            // _filter_heathaze on this layer: what is drawn down to here shimmers.
            if let Some(haze) = layer.heat_haze {
                canvas.heat_haze(haze, current_time_ms / 1000.0);
            }
        }
        for (_, item) in dynamic {
            self.draw_dynamic(canvas, item, own_skin.as_ref(), net, unlockables, current_time_ms);
        }
        canvas.set_view(0.0, 0.0);
    }

    /// The abilities' objects: the server's, or without a server this player's own.
    fn entity_views(&self) -> Vec<EntityView> {
        match &self.online {
            Some(online) => online.entities.clone(),
            None => entity_views(&self.game, &[0]),
        }
    }

    fn draw_entity(&self, canvas: &mut Canvas, entity: &EntityView, own_skin: Option<&(Colours, Colours)>, skins: &BTreeMap<u16, PlayerSkin>) {
        let dark = if self.room_id == RoomId::Act9 { C_DKGRAY } else { C_WHITE };
        match *entity {
            EntityView::TailsShot { x, y, sprite_index, image_index, dir } => {
                canvas.draw_sprite_ext(sprite_index, image_index as f64, x as f64, y as f64, dir as f64, 1.0, 0.0, C_WHITE, 1.0);
            }
            EntityView::EggTracker { x, y, sprite_index, image_index } => {
                canvas.draw_sprite_ext(sprite_index, image_index as f64, x as f64, y as f64, 1.0, 1.0, 0.0, dark, 1.0);
            }
            EntityView::StompWave { x, y, image_index, dir } => {
                // ponytail: the purple waves of a "menace" skin come with other players' skins
                canvas.draw_sprite_ext(sprite::SPR_EXETIOR_STOMPBALLS, image_index as f64, x as f64, y as f64, -dir as f64, 1.0, 0.0, dark, 1.0);
            }
            EntityView::BlackRing { x, y, alpha, .. } => {
                // ponytail: the purple ring of a "menace" Exetior comes with other players' skins
                let (x, y) = (x as f64, y as f64);
                canvas.draw_sprite_ext(sprite::SPR_BLACKRING, self.ring_frame, x, y, 1.0, 1.0, 0.0, dark, alpha as f64);
                // obj_blackring Draw: Majin Forest's copy at the other end.
                if self.room_id == RoomId::Majongforest {
                    let (room_width, ring_width) = (self.game.world.room_width, canvas.sprites.get(sprite::SPR_BLACKRING).width as f64);
                    if x < MAJIN_FOREST_SOUND_EDGE {
                        canvas.draw_sprite_ext(sprite::SPR_BLACKRING, self.ring_frame, x + room_width - MAJIN_FOREST_SOUND_EDGE + ring_width, y, 1.0, 1.0, 0.0, dark, alpha as f64);
                    } else if x >= room_width - MAJIN_FOREST_SOUND_EDGE {
                        canvas.draw_sprite_ext(sprite::SPR_BLACKRING, self.ring_frame, x - (room_width - MAJIN_FOREST_SOUND_EDGE) - ring_width, y, 1.0, 1.0, 0.0, dark, alpha as f64);
                    }
                }
            }
            // The rings are drawn with the room, at their own depth (Dynamic::Rings).
            EntityView::Ring { .. } => {}
            EntityView::ExellerClone { owner, x, y, sprite_index, image_xscale, .. } => {
                let blend = if self.room_id == RoomId::Act9 { 0x000000 } else { C_WHITE };
                let (from, to) = match own_skin.filter(|_| Some(owner) == self.own_id) {
                    Some((from, to)) => (from.clone(), to.clone()),
                    None => other_skin(skins, owner),
                };
                canvas.set_palette_swap(&from, &to);
                canvas.draw_sprite_ext(sprite_index, 0.0, x as f64, y as f64, image_xscale as f64, 1.0, 0.0, blend, 1.0);
                canvas.reset_shader();
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_dynamic(
        &self,
        canvas: &mut Canvas,
        item: Dynamic,
        own_skin: Option<&(Colours, Colours)>,
        net: &net::NetClient,
        unlockables: &crate::client::unlockables::Unlockables,
        current_time_ms: f64,
    ) {
        let (skins, revivals, pet_skins) = (&net.player_skins, &net.revivals, &net.pet_skins);
        let online = match item {
            Dynamic::Effect(index) => return self.effects[index].draw(canvas),
            Dynamic::Rings => {
                let act9 = self.room_id == RoomId::Act9;
                for ring in &self.game.world.rings {
                    draw_ring(canvas, ring, self.ring_frame, act9);
                }
                // obj_ringpart: depth of the player - 1
                for part in &self.ring_parts {
                    part.draw(canvas);
                }
                return;
            }
            Dynamic::Entities(kind) => {
                let entities = self.entity_views();
                for entity in entities.iter().filter(|entity| EntityKind::of(entity) == kind) {
                    self.draw_entity(canvas, entity, own_skin, skins);
                }
                return;
            }
            Dynamic::RevivalSigns => {
                if let (Some(online), Some(own)) = (&self.online, self.game.players.first()) {
                    for (id, downed) in &online.others {
                        let (shown, progress) = revivals.get(id).copied().unwrap_or((false, 0.0));
                        if let Some((sign, frame)) = revival_sign(&self.game.world, own, downed, shown, progress, canvas.sprites.get(sprite::SPR_REVIVAL).frame_count as f64) {
                            canvas.draw_sprite(sign, frame, downed.x, downed.y - REVIVAL_SIGN_ABOVE);
                        }
                    }
                }
                return;
            }
            Dynamic::HealSparkles => {
                for sparkle in &self.heal_sparkles {
                    sparkle.draw(canvas);
                }
                return;
            }
            Dynamic::Smoke => {
                if let Some(you_cant_run) = &self.you_cant_run {
                    you_cant_run.draw(canvas, current_time_ms);
                }
                return;
            }
            Dynamic::Snow | Dynamic::IceShards | Dynamic::SnowballPieces => {
                if let Some(nasty_paradise) = &self.nasty_paradise {
                    match item {
                        Dynamic::Snow => nasty_paradise.draw_snow(canvas),
                        Dynamic::IceShards => nasty_paradise.draw_shards(canvas),
                        _ => nasty_paradise.draw_pieces(canvas),
                    }
                }
                return;
            }
            Dynamic::RedCity | Dynamic::Pupils => {
                if let (Some(limp_city), Some(camera)) = (&self.limp_city, &self.camera) {
                    if matches!(item, Dynamic::RedCity) {
                        limp_city.draw_red(canvas, (camera.x, camera.y), current_time_ms);
                    } else {
                        // obj_player_puppet instances that are visible, and this player.
                        let others = self.online.iter().flat_map(|online| online.others.iter().map(|(_, player)| player)).filter(|player| !player.is_hiding);
                        let players: Vec<(f64, f64)> = self.game.players.iter().chain(others).map(|player| (player.x, player.y)).collect();
                        limp_city.draw_pupils(canvas, &self.game.world, &players);
                    }
                }
                return;
            }
            Dynamic::Static => {
                if let (Some(echidna_ruins), Some(camera)) = (&self.echidna_ruins, &self.camera) {
                    echidna_ruins.draw_static(canvas, (camera.x, camera.y), current_time_ms);
                }
                return;
            }
            Dynamic::Shards | Dynamic::Slugs => {
                if let Some(ravine_mist) = &self.ravine_mist {
                    match item {
                        Dynamic::Shards => ravine_mist.draw_shards(canvas, &self.game.world, current_time_ms),
                        _ => ravine_mist.draw_slugs(canvas, &self.game.world, current_time_ms),
                    }
                }
                return;
            }
            Dynamic::DummyTexts => {
                if let Some(fart_zone) = &self.fart_zone {
                    fart_zone.draw_texts(canvas, current_time_ms);
                }
                return;
            }
            Dynamic::Ghosts(second_kind) => {
                if let (Some(weed_zone), Some(camera)) = (&self.weed_zone, &self.camera) {
                    weed_zone.draw_ghosts(canvas, (camera.x, camera.y), second_kind, current_time_ms);
                }
                return;
            }
            Dynamic::SkyShapes => {
                if let (Some(not_perfect), Some(camera)) = (&self.not_perfect, &self.camera) {
                    not_perfect.draw_particles(canvas, (camera.x, camera.y));
                }
                return;
            }
            Dynamic::Drips | Dynamic::StalactiteShards => {
                if let Some(dark_tower) = &self.dark_tower {
                    match item {
                        Dynamic::Drips => dark_tower.draw_drips(canvas),
                        _ => dark_tower.draw_shards(canvas),
                    }
                }
                return;
            }
            Dynamic::Pet => {
                if let (Some(pet), Some(player)) = (&self.pet, self.game.players.first()) {
                    pet.draw(canvas, unlockables.pet_palettes.get(pet.palette()), player);
                }
                // Someone the server keeps out of view takes their pet along.
                for (id, player) in self.online.iter().flat_map(|online| online.others.iter()) {
                    if let Some(pet) = self.other_pets.get(id) {
                        pet.draw(canvas, pet_skins.get(id), player);
                    }
                }
                return;
            }
            Dynamic::VaseShards => {
                if let Some(volcano_valley) = &self.volcano_valley {
                    volcano_valley.draw_shards(canvas);
                }
                return;
            }
            Dynamic::Instance(id) => {
                draw_instance(canvas, &self.game.world, id, self.game.players.first(), current_time_ms);
                return;
            }
            Dynamic::Rain => {
                if let Some(green_hill) = &self.green_hill {
                    green_hill.draw_rain(canvas);
                }
                return;
            }
            Dynamic::BigRing => {
                if let Some((x, y, ready, alpha)) = self.big_ring {
                    let big_ring = if ready { sprite::SPR_BIGRING_READY } else { sprite::SPR_BIGRING };
                    let blend = if self.room_id == RoomId::Act9 { 0x000000 } else { C_WHITE };
                    let frame = current_time_ms * canvas.sprites.get(big_ring).fps as f64 / 1000.0;
                    canvas.draw_sprite_ext(big_ring, frame, x, y, 1.0, 1.0, 0.0, blend, alpha);
                }
                return;
            }
            Dynamic::RedScreen => {
                if let (Some(frame), Some(camera)) = (self.red_screen, &self.camera) {
                    draw_red_screen(canvas, frame, (camera.x, camera.y));
                }
                return;
            }
            Dynamic::OwnPlayer => {
                let (Some(player), Some((from, to))) = (self.game.players.first(), own_skin) else { return };
                // scr_move_basic: once the round runs.
                let blend = if self.room_id == RoomId::Act9 && self.game.world.round_started { 0x000000 } else { C_WHITE };
                return draw_player(canvas, player, &Skin { from, to }, blend, self.tail_frame, true, current_time_ms);
            }
            Dynamic::Others => match &self.online {
                Some(online) => online,
                None => return,
            },
        };
        for (id, player) in &online.others {
            // obj_player_puppet Step: dark grey while inactive, black on Act 9 above all.
            let blend = if self.room_id == RoomId::Act9 { 0x000000 } else if player.inactive { C_DKGRAY } else { C_WHITE };
            let (from, to) = other_skin(skins, *id);
            draw_player(canvas, player, &Skin { from: &from, to: &to }, blend, self.tail_frame, false, current_time_ms);
        }
    }
}

/// What a level draws besides the room: players and effects, by depth.
#[derive(Clone, Copy)]
enum Dynamic {
    OwnPlayer,
    Others,
    RevivalSigns,
    HealSparkles,
    Rain,
    Smoke,
    Snow,
    IceShards,
    SnowballPieces,
    VaseShards,
    Pet,
    Drips,
    SkyShapes,
    DummyTexts,
    Shards,
    Slugs,
    Static,
    RedCity,
    Pupils,
    /// Weed Zone's ghosts, of the second kind or not.
    Ghosts(bool),
    StalactiteShards,
    Instance(InstanceId),
    Entities(EntityKind),
    BigRing,
    Rings,
    RedScreen,
    Effect(usize),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EntityKind {
    TailsShot,
    EggTracker,
    StompWave,
    ExellerClone,
    BlackRing,
    Ring,
}

impl EntityKind {
    fn of(entity: &EntityView) -> EntityKind {
        match entity {
            EntityView::TailsShot { .. } => EntityKind::TailsShot,
            EntityView::EggTracker { .. } => EntityKind::EggTracker,
            EntityView::StompWave { .. } => EntityKind::StompWave,
            EntityView::ExellerClone { .. } => EntityKind::ExellerClone,
            EntityView::BlackRing { .. } => EntityKind::BlackRing,
            EntityView::Ring { .. } => EntityKind::Ring,
        }
    }
}

/// The simulation creates the room's instances in the order the room lists them, so
/// the n-th placed instance is the n-th simulated one.
fn instances_by_layer(room: &Room, world: &World) -> Result<(Vec<Vec<InstanceId>>, Vec<(i32, InstanceId)>)> {
    let placed = room.placed_in_creation_order();
    if placed.len() > world.instances.len() {
        bail!("the room places {} instances, the simulation made {}", placed.len(), world.instances.len());
    }
    let mut by_layer = vec![Vec::new(); room.layers.len()];
    let mut by_depth = Vec::new();
    for (id, (layer, instance)) in placed.iter().enumerate() {
        let simulated = world.instances[id].object;
        if simulated != instance.object {
            bail!("placed instance {} is {:?} in the room but {simulated:?} in the simulation", instance.id, instance.object);
        }
        match create_depth(simulated) {
            Some(depth) => by_depth.push((depth, id)),
            None => by_layer[*layer].push(id),
        }
    }
    for id in placed.len()..world.instances.len() {
        let object = world.instances[id].object;
        let Some(depth) = create_depth(object) else { bail!("{object:?} is made with the room but has no depth") };
        by_depth.push((depth, id));
    }
    Ok((by_layer, by_depth))
}

/// Without a server: a random spawn point of the level, as obj_spawnpoint does.
fn sandbox_spawn(context: &Context, world: &World) -> Result<(f64, f64)> {
    let spawn_object = if own_character(context).0 == Character::Exe { ObjectId::Exespawn } else { ObjectId::Spawnpoint };
    let spawns: Vec<InstanceId> = world.ids_of(spawn_object).collect();
    let Some(&spawn) = spawns.get(rand::gen_range(0, spawns.len().max(1))) else {
        bail!("the room has no {}", spawn_object.info().name);
    };
    Ok((world.instances[spawn].x, world.instances[spawn].y - SPAWN_ABOVE))
}

/// Without a server: this player standing at that spawn point, demonized when the
/// Singleplayer page picked a demon. None for the free camera, which plays nobody.
fn sandbox_player(context: &Context, config: &GameplayConfig, (x, y): (f64, f64)) -> Option<Player> {
    if context.net.freecam {
        return None;
    }
    let (character, exe_character) = own_character(context);
    if character == Character::Exe {
        let mut exe = Player::new_exe(exe_character, x, y, config);
        exe.pick_lost_pose(rand::gen_range(0, 2) == 1);
        return Some(exe);
    }
    let mut player = Player::new(character, x, y, config);
    if context.net.demonized {
        // The same turn a round gives a survivor nobody revived in time; its effect and
        // its sound belong to that moment, not to standing there demonized from the start.
        player.death_timer_end(config, true, &mut Vec::new());
    }
    Some(player)
}

/// The character the server gave this player (global.character, global.exeCharacter).
fn own_character(context: &Context) -> (Character, ExeCharacter) {
    let exe = match context.net.exe_character {
        1 => ExeCharacter::Chaos,
        2 => ExeCharacter::Exetior,
        3 => ExeCharacter::Exeller,
        _ => ExeCharacter::Original,
    };
    let character = match context.net.character {
        net::EXE_CHARACTER => Character::Exe,
        2 => Character::Knux,
        3 => Character::Eggman,
        4 => Character::Amy,
        5 => Character::Cream,
        6 => Character::Sally,
        _ => Character::Tails,
    };
    (character, exe)
}

/// global.palleteFrom and global.palleteTo: the chosen skin of the player's look.
fn own_skin(context: &Context, player: &Player) -> (Colours, Colours) {
    let table = skin_table(player);
    match context.unlockables.skins.iter().find(|skin| skin.table == table) {
        Some(skin) => (skin.from.clone(), skin.to.clone()),
        None => (palette::default_colours(), palette::default_colours()),
    }
}

/// The palette table of a player's look: character, demonized character or EXE character.
fn skin_table(player: &Player) -> usize {
    match player.character {
        Character::Exe => player.exe_character as usize + PALETTE_EXE,
        character if player.is_demonized() => character as usize + PALETTE_DEMON,
        character => character as usize,
    }
}

/// obj_revival_puppet over a downed teammate: the reviving progress while teammates
/// revive them, else whether this player could revive them here (scr_survivor_revive).
fn revival_sign(world: &World, own: &Player, downed: &Player, shown: bool, progress: f64, frames: f64) -> Option<(crate::core::resources::SpriteId, f64)> {
    if shown {
        return Some((sprite::SPR_REVIVAL, progress * frames));
    }
    let can_be_revived = downed.hp <= 0 && downed.revival_times == 0 && downed.character != Character::Exe;
    let can_revive = own.character != Character::Exe && own.revival_times < 2 && own.hp > 0;
    if !(can_be_revived && can_revive && crate::core::contact::bodies_touch(world, downed, own)) {
        return None;
    }
    Some((if own.rings >= REVIVAL_RINGS { sprite::SPR_REVIVAL } else { sprite::SPR_REVIVAL2 }, 0.0))
}

/// audio_play_sound for this player's own sounds; someone else's through their
/// emitter (obj_player_puppet): gain 1 - min(distance, 700) / 700 from the view's
/// centre, panned towards their side.
fn play_sound(context: &mut Context, camera: &Camera, sound: crate::core::resources::SoundId, x: f64, y: f64, own: bool, wrap_width: Option<f64>) {
    if own {
        context.audio.play(sound, false);
        return;
    }
    let listener = (camera.x + VIEW_WIDTH / 2.0, camera.y + VIEW_HEIGHT / 2.0);
    context.audio.play_at(sound, heard_gain(listener, x, y), pan_towards(listener, x, y));
    // scr_audio_play_3d on Majin Forest: a sound near one end also plays at the other
    // (obj_majong_sound, whose gain is measured from a point left of the view's centre).
    let Some(width) = wrap_width else { return };
    let mirrored_x = if x < MAJIN_FOREST_SOUND_EDGE {
        x + (width - MAJIN_FOREST_SOUND_EDGE)
    } else if x >= width - MAJIN_FOREST_SOUND_EDGE {
        x - (width - MAJIN_FOREST_SOUND_EDGE)
    } else {
        return;
    };
    play_sound_near_ear(context, camera, sound, mirrored_x, y);
}

/// Emitters whose gain the original measured from a point left of and above the view's
/// centre (obj_majong_sound, obj_hd_door, obj_hd_crystal), panned from the centre.
fn play_sound_near_ear(context: &mut Context, camera: &Camera, sound: crate::core::resources::SoundId, x: f64, y: f64) {
    let listener = (camera.x + VIEW_WIDTH / 2.0, camera.y + VIEW_HEIGHT / 2.0);
    let gain = heard_gain((camera.x + LEFT_EAR.0, camera.y + LEFT_EAR.1), x, y);
    context.audio.play_at(sound, gain, pan_towards(listener, x, y));
}

/// 1 - min(distance, 700) / 700
fn heard_gain(listener: (f64, f64), x: f64, y: f64) -> f64 {
    heard_gain_within(listener, x, y, HEARING_DISTANCE)
}

fn heard_gain_within(listener: (f64, f64), x: f64, y: f64, reach: f64) -> f64 {
    1.0 - (x - listener.0).hypot(y - listener.1).min(reach) / reach
}

fn pan_towards(listener: (f64, f64), x: f64, y: f64) -> f64 {
    let distance = (x - listener.0).hypot(y - listener.1);
    if distance > 0.0 {
        (x - listener.0) / distance
    } else {
        0.0
    }
}

/// Someone else's skin as they sent it, or the sprites' own colours until then.
fn other_skin(skins: &BTreeMap<u16, PlayerSkin>, id: u16) -> (Colours, Colours) {
    match skins.get(&id) {
        Some(skin) => (skin.from.clone(), skin.to.clone()),
        None => (palette::default_colours(), palette::default_colours()),
    }
}

/// The player's controls this tick.
fn held_buttons(context: &Context) -> Buttons {
    let keys = &context.options.keys;
    let input = &context.input;
    let bindings = [
        (keys.left, Buttons::LEFT),
        (keys.right, Buttons::RIGHT),
        (keys.up, Buttons::UP),
        (keys.down, Buttons::DOWN),
        (keys.jump, Buttons::A),
        (keys.special1, Buttons::B),
        (keys.special2, Buttons::C),
        (keys.emotion1, Buttons::EMOTION1),
        (keys.emotion2, Buttons::EMOTION2),
        (keys.emotion3, Buttons::EMOTION3),
        (keys.emotion4, Buttons::IDLE_POSE),
    ];
    Buttons(bindings.iter().filter(|(key, _)| input.held(key.0)).fold(0, |held, (_, button)| held | button))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::snapshot::PlayerView;

    #[test]
    fn others_move_smoothly_a_few_ticks_behind_the_newest_snapshot() {
        let config = GameplayConfig::default();
        let view_at = |x: f64| {
            let player = Player::new(Character::Amy, x, 50.0, &config);
            OtherPlayer { id: 7, view: Some(PlayerView::of(&player)), health: crate::core::snapshot::HealthView::of(&player) }
        };
        let mut online = Online { next_tick: 0, inputs: VecDeque::new(), others: Vec::new(), health: Vec::new(), others_history: VecDeque::new(), render_tick: 0.0, entities: Vec::new(), silent_ticks: 0 };
        online.others_history.push_back((10, vec![view_at(100.0)]));
        online.others_history.push_back((12, vec![view_at(120.0)]));
        online.others_history.push_back((16, vec![view_at(160.0)]));

        online.interpolate_others(&config);
        assert_eq!(online.render_tick, 12.0, "four ticks behind the newest snapshot");
        assert_eq!(online.others[0].1.x, 120.0);

        online.interpolate_others(&config);
        assert_eq!(online.render_tick, 13.0, "then one tick a tick");
        assert_eq!(online.others[0].1.x, 130.0, "a quarter of the way from the tick 12 snapshot to the tick 16 one");
    }
}
