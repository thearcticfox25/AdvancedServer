//! Authoritative rounds (states.gameplay.authoritative_round): the server moves
//! every player with the core from the buttons their clients send and sends each
//! client the result every tick. A client claims nothing about its player.

use crate::config::cfg;
use crate::packet::{Packet, PacketType};
use crate::server::{ExeChar, OutboxMsg, Server, SurvChar};
use anyhow::{bail, Context, Result};
use std::collections::VecDeque;
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use crate::core::resources::sprites::Sprites;
use crate::core::config::{gameplay_config, GameplayConfig};
use crate::core::events::{EventSource, SimEvent};
use crate::core::game::Game;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Buttons, Character, ExeCharacter, Player};
use crate::core::rooms::LEVEL_ROOMS;
use crate::core::snapshot::{entity_views, HealthView, OtherPlayer, PlayerView, RoundSetup, Snapshot};
use crate::core::world::World;

/// obj_spawnpoint and obj_exespawn create the player this far above themselves.
const SPAWN_ABOVE: f64 = 18.0;
/// Buttons a client may be ahead of the server by. A client whose clock runs fast
/// would otherwise pile up delay; the oldest buttons beyond this are dropped.
const MAX_QUEUED_INPUTS: usize = 8;
/// After this many ticks without new buttons the player lets go of everything: a
/// silent client stands still, visible and hittable. Before that the last buttons
/// stay held, so a late packet does not stop a running player for one tick.
const HOLD_LAST_BUTTONS_TICKS: u32 = 6;
/// obj_player_puppet alarm[1] = 60: a player whose client sent nothing for this long
/// is inactive: drawn dark grey, and EXE does not bounce off them.
const INACTIVE_AFTER_SECONDS: f64 = 1.0;
/// CLIENT_ROUND_INPUT carries at most this many ticks.
const MAX_INPUTS_PER_PACKET: u8 = 16;
/// A survivor needs this many rings to revive a teammate (scr_survivor_revive).
const REVIVAL_RINGS: i32 = 3;
/// states.gameplay.limit_player_view: pixels beyond the edge of the window a player
/// may still be told about. It covers the camera's own slack -- looking up or down
/// moves it 100 px (client camera.rs LOOK_DISTANCE) -- so nothing is ever cut off
/// something a client is about to draw.
const VIEW_MARGIN: f64 = 128.0;
/// How long a player Eggman's tracker caught keeps being shown to that Eggman
/// (client indicators.rs TRACKED_SECONDS).
const TRACKED_SECONDS: f64 = 3.0;

/*Йо, на старом сервере гуляла толпа ворья:
Он пакеты пересылал - коммутатор, не судья!
Он даже карты не знал: где стены, где пол, где провал -
Вслепую раздавал всё, что клиент ему наврал.
Клиент кричит "я тут!" - и сервер верит на слово,
Тейлз летит сквозь стены, Эггман в текстурах снова.
Спидхак, флай над картой, кольца из воздуха в миг -
Сервер всё разослал, и никто не проверил их.
EXE бьёт - мимо: хитбокс у клиента в голове,
А выживший с нулём HP гуляет по траве.
Ревайв без колец, эскейп в начале раунда - смешно,
Читер правит раундом, а сервер кивает всё равно.
Но переписан мир, и теперь сервер - закон:
Клиент шлёт только кнопки, а двигает игрока - он.
Карту знает сервер сам: каждый тайл, каждый провал,
Сквозь стену не пройдёшь - он её уже считал.
Где ты стоишь - решает он, а не ты сам,
Твоё "я уже у выхода" он выкинет к чертям.
Восемь тиков в очереди - спидхак упёрся в лимит,
Шесть тиков тишины - стоишь, и EXE тебя разит.
Кольца считает сервер: три кольца - и есть ревайв,
Нарисовал себе сотню? Сервер скажет: "Не вайб."
Ноклип и ВХ в прошлом: видишь только то, что рядом,
Сервер правит соло - читер провожает раунд взглядом!*/

pub struct Round {
    game: Game,
    /// Parallel to game.players.
    players: Vec<RoundPlayer>,
    /// SERVER_ROUND_CONFIG as it was sent at the start, kept for a client that asks to
    /// watch the round after it began (states::spectate).
    setup_packet: Vec<u8>,
    tick: u32,
    /// The events of the latest tick and whose code made them.
    events: Vec<(EventSource, SimEvent)>,
    /// Events of changes the round rules made to players since the last tick.
    rule_events: Vec<(EventSource, SimEvent)>,
    /// Each healer's gauge over each teammate (healer * players + teammate).
    heal_progress: Vec<f64>,
}

struct RoundPlayer {
    id: u16,
    /// The player a tracker of this one's side caught and the ticks their arrow still
    /// points at them, so a cut view does not take the catch away (see seen_by).
    tracked: Option<(usize, i32)>,
    /// Client ticks and their buttons, oldest first, not applied yet.
    queued: VecDeque<(u32, Buttons)>,
    newest_received: Option<u32>,
    last_applied: Option<u32>,
    held: Buttons,
    ticks_without_input: u32,
    /// Health when last looked at, for the damage the results screen counts.
    last_hp: i32,
}

impl RoundPlayer {
    /// The buttons for this server tick.
    fn next_buttons(&mut self) -> Buttons {
        match self.queued.pop_front() {
            Some((tick, buttons)) => {
                self.last_applied = Some(tick);
                self.held = buttons;
                self.ticks_without_input = 0;
            }
            None => {
                self.ticks_without_input += 1;
                if self.ticks_without_input > HOLD_LAST_BUTTONS_TICKS {
                    self.held = Buttons::default();
                }
            }
        }
        self.held
    }
}

fn sprites() -> Result<Arc<Sprites>> {
    static SPRITES: OnceLock<Arc<Sprites>> = OnceLock::new();
    if let Some(sprites) = SPRITES.get() {
        return Ok(sprites.clone());
    }
    let textures = crate::core::resources::content_folder().join(crate::core::resources::TEXTURES);
    let sprites = Arc::new(Sprites::index(&textures).with_context(|| format!("sprites in {}", textures.display()))?);
    Ok(SPRITES.get_or_init(|| sprites).clone())
}

/// SERVER_ROUND_CONFIG as the round's players were sent it, for a spectator who
/// starts watching after the round began (states::spectate).
pub fn config_packet(round: &Round) -> &[u8] {
    &round.setup_packet
}

/// Whether the round's level lets the big ring open yet (Ravine Mist's shards).
pub fn big_ring_may_open(round: &Round) -> bool {
    round.game.level.as_ref().is_none_or(crate::core::level::Level::big_ring_may_open)
}

/// Called by game_init: the round's room with every player of the round at a spawn point.
pub fn start(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let round = match create(server) {
        Ok(round) => round,
        Err(error) => {
            log::error!("{} the authoritative round could not start: {error:#}", crate::server::lobby_tag(server.id));
            return;
        }
    };
    let tick_rate = crate::config::cfg().server_config.networking.tick_rate;
    let setup = RoundSetup { players: round.players.iter().map(|player| player.id).collect(), config: (*gameplay_config()).clone(), tick_rate };
    let mut round = round;
    round.setup_packet = vec![0, PacketType::SERVER_ROUND_CONFIG as u8];
    round.setup_packet.extend(setup.encode());
    for peer in server.peers.iter().filter(|peer| peer.in_game) {
        outbox.push(OutboxMsg::SendTo(peer.id, round.setup_packet.clone(), true));
    }
    server.game.round = Some(round);
}

fn create(server: &Server) -> Result<Round> {
    let Some(&room) = LEVEL_ROOMS.get(server.game.map as usize) else {
        bail!("map {} is not a level", server.game.map);
    };
    let config = gameplay_config();
    let mut world = World::empty(sprites()?, config.clone());
    world.load_room(&crate::core::resources::content_folder().join(crate::core::resources::MAPS), room)?;
    world.place_map_black_rings(crate::core::level::map_black_rings(room, &config));
    let survivor_spawns: Vec<usize> = world.ids_of(ObjectId::Spawnpoint).collect();
    let exe_spawns: Vec<usize> = world.ids_of(ObjectId::Exespawn).collect();

    let mut game_players = Vec::new();
    let mut players = Vec::new();
    for peer in server.peers.iter().filter(|peer| peer.in_game) {
        let is_exe = server.lobby.is_exe(peer.id);
        let spawns = if is_exe { &exe_spawns } else { &survivor_spawns };
        // Each player picks a spawn point on their own, as each client did in obj_spawnpoint.
        let Some(&spawn) = spawns.get(rand::random::<usize>() % spawns.len().max(1)) else {
            bail!("room {room:?} has no spawn point for {}", if is_exe { "EXE" } else { "survivors" });
        };
        let (x, y) = (world.instances[spawn].x, world.instances[spawn].y - SPAWN_ABOVE);
        let player = if is_exe {
            let mut exe = Player::new_exe(exe_character(peer.exe_char), x, y, &config);
            exe.pick_lost_pose(rand::random());
            exe
        } else {
            Player::new(survivor_character(peer.surv_char), x, y, &config)
        };
        game_players.push(player);
        players.push(RoundPlayer {
            id: peer.id,
            queued: VecDeque::new(),
            newest_received: None,
            last_applied: None,
            held: Buttons::default(),
            ticks_without_input: 0,
            tracked: None,
            last_hp: game_players.last().map_or(0, |player: &Player| player.hp),
        });
    }
    let mut game = Game::new(world);
    game.players = game_players;
    let mut level = crate::core::level::Level::new(&game.world, rand::random());
    // How often this map puts out a ring: its own number, one second shorter with
    // more than three playing (the map_ring of the original server).
    level.set_ring_seconds(server.game.ring_coff as f64);
    game.level = Some(level);
    // Ambush (gmcycle) demonized some players before the round existed.
    let cfg = game.world.config.clone();
    for (index, peer) in server.peers.iter().filter(|peer| peer.in_game).enumerate() {
        if peer.plr.flags & crate::player::flags::DEMONIZED != 0 {
            game.players[index].death_timer_end(&cfg, true, &mut Vec::new());
        }
    }
    let player_count = game.players.len();
    Ok(Round { game, players, setup_packet: Vec::new(), tick: 0, events: Vec::new(), rule_events: Vec::new(), heal_progress: vec![0.0; player_count * player_count] })
}

fn survivor_character(character: SurvChar) -> Character {
    match character {
        SurvChar::Knux => Character::Knux,
        SurvChar::Eggman => Character::Eggman,
        SurvChar::Amy => Character::Amy,
        SurvChar::Cream => Character::Cream,
        SurvChar::Sally => Character::Sally,
        SurvChar::Tails | SurvChar::None => Character::Tails,
    }
}

fn exe_character(character: ExeChar) -> ExeCharacter {
    match character {
        ExeChar::Chaos => ExeCharacter::Chaos,
        ExeChar::Exetior => ExeCharacter::Exetior,
        ExeChar::Exeller => ExeCharacter::Exeller,
        ExeChar::Original | ExeChar::None => ExeCharacter::Original,
    }
}

/// CLIENT_ROUND_INPUT: u32 client tick of the newest buttons, u8 count, then
/// count u16 buttons, oldest first. Ticks the server already has are skipped.
pub fn receive_input(player_id: u16, packet: &mut Packet, server: &mut Server) {
    packet.pos = 2;
    let (Some(newest_tick), Some(count)) = (packet.read_u32(), packet.read_u8()) else { return };
    let count = count.min(MAX_INPUTS_PER_PACKET);
    let Some(round) = server.game.round.as_mut() else { return };
    let Some(player) = round.players.iter_mut().find(|player| player.id == player_id) else { return };
    for offset in (0..count).rev() {
        let Some(buttons) = packet.read_u16() else { break };
        let Some(tick) = newest_tick.checked_sub(offset as u32) else { continue };
        if player.newest_received.is_some_and(|newest| tick <= newest) {
            continue;
        }
        player.queued.push_back((tick, Buttons(buttons)));
        player.newest_received = Some(tick);
    }
    while player.queued.len() > MAX_QUEUED_INPUTS {
        player.queued.pop_front();
    }
    if let Some(peer) = server.find_peer_mut(player_id) {
        peer.plr.last_packet = Some(Instant::now());
        peer.plr.timeout = 0.0;
    }
}

/// CLIENT_ROUND_PAUSE: one byte, whether the player's pause menu is open. Their
/// character keeps standing in the round; the snapshot tells everyone else.
pub fn receive_pause(player_id: u16, packet: &mut Packet, server: &mut Server) {
    packet.pos = 2;
    let Some(paused) = packet.read_u8() else { return };
    let Some(round) = server.game.round.as_mut() else { return };
    let Some(index) = round.players.iter().position(|player| player.id == player_id) else { return };
    if let Some(player) = round.game.players.get_mut(index) {
        player.paused = paused != 0;
    }
}

/// One server tick of the round: everyone's buttons, one simulation step, and a
/// snapshot to every player. Before all clients are ready players only fall into place.
pub fn tick(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let Some(mut round) = server.game.round.take() else { return };
    round.game.world.round_started = server.game.started;
    round.game.world.timer_ticks = crate::core::config::ticks(server.game.time_sec as f64) as i64;
    let buttons: Vec<Buttons> = round.players.iter_mut().map(RoundPlayer::next_buttons).collect();
    let inactive_after = crate::core::config::ticks(INACTIVE_AFTER_SECONDS) as u32;
    for (player, round_player) in round.game.players.iter_mut().zip(&round.players) {
        player.inactive = round_player.ticks_without_input > inactive_after;
    }
    let events = round.game.tick(&buttons);
    round.events = std::mem::take(&mut round.rule_events);
    round.events.extend(round.game.event_sources.iter().copied().zip(events));
    round.tick += 1;
    count_damage_taken(&mut round, server);

    let healers = if server.game.started { heal_teammates(&mut round) } else { Vec::new() };
    for &healer in &healers {
        if let Some(peer) = server.find_peer_mut(round.players[healer].id) {
            peer.plr.stats.hp_restored += 1;
        }
    }
    sync_players_to_rules(&round, server);
    sync_level_to_rules(&round, server);
    track_catches(&mut round);
    send_snapshots(&round, server, outbox);
    let ids: Vec<u16> = round.players.iter().map(|player| player.id).collect();
    let hps: Vec<i32> = round.game.players.iter().map(|player| player.hp).collect();
    let events = std::mem::take(&mut round.events);
    let revivals = revivals(&round);
    server.game.round = Some(round);
    apply_round_rules(&ids, &hps, &events, server, outbox);
    escape_through_big_ring(&ids, server, outbox);
    if server.game.started {
        for (reviver, target, extra_rings) in revivals {
            crate::states::game::revival_progress(ids[reviver], ids[target], extra_rings, server, outbox);
        }
    }
}

/// What the round rules read about the level: Ravine Mist's shards, which decide when
/// its big ring opens. The rules used to count the shards a client said it had picked up.
fn sync_level_to_rules(round: &Round, server: &mut Server) {
    server.game.shards_found = round.game.level.as_ref().map_or(0, crate::core::level::Level::shards_found);
}

/// What the round rules keep reading about players (camping, escaping, death timers, results).
fn sync_players_to_rules(round: &Round, server: &mut Server) {
    for (player, simulated) in round.players.iter().zip(&round.game.players) {
        let Some(peer) = server.find_peer_mut(player.id) else { continue };
        peer.plr.pos = (simulated.x as f32, simulated.y as f32);
        peer.plr.vel = (simulated.xspd as f32, simulated.yspd as f32);
        peer.plr.state = simulated.state as u8;
        peer.plr.is_attacking = crate::core::contact::seen_attacking(simulated);
        if simulated.character != Character::Exe {
            peer.plr.hp = simulated.hp.clamp(i8::MIN as i32, i8::MAX as i32) as i8;
            peer.plr.revival_times = simulated.revival_times.clamp(0, u8::MAX as i32) as u8;
            peer.plr.rings = simulated.rings.clamp(0, i16::MAX as i32) as i16;
            peer.plr.red_ring = simulated.red_ring_timer > 0;
        }
    }
}




/// obj_bigring Step: a survivor touching the ready big ring escapes and leaves the level.
fn escape_through_big_ring(ids: &[u16], server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if server.game.bring_state != crate::server::BigRingState::Activated {
        return;
    }
    let Some(round) = server.game.round.as_ref() else { return };
    let world = &round.game.world;
    let spawns: Vec<usize> = world.ids_of(ObjectId::Ringspawn).collect();
    let Some(&spawn) = spawns.get(server.game.bring_loc as usize % spawns.len().max(1)) else { return };
    let (x, y) = (world.instances[spawn].x, world.instances[spawn].y);
    let big_ring = crate::core::collision::sprite_bbox(world.sprites.get(crate::core::resources::names::sprite::SPR_BIGRING_READY), x, y, 1.0, 1.0, 0.0);
    let escaping: Vec<usize> = round
        .game
        .players
        .iter()
        .enumerate()
        .filter(|(_, player)| {
            let can_escape = !player.removed && !player.is_dead && player.character != Character::Exe && player.revival_times < 2 && player.red_ring_timer <= 0;
            let body = crate::core::collision::sprite_bbox(world.sprites.get(player.sprite_index), player.x, player.y, player.image_xscale, 1.0, 0.0);
            can_escape && big_ring.overlaps(&body)
        })
        .map(|(index, _)| index)
        .collect();
    for index in escaping {
        if crate::states::game::player_escaped(ids[index], server, outbox) {
            if let Some(round) = server.game.round.as_mut() {
                round.game.players[index].removed = true;
            }
        }
    }
}

/// net_sound_emit: an invisible EXE's sounds and taunts reach nobody else, springs excepted.
fn silenced_by_invisibility(player: &Player, event: &SimEvent) -> bool {
    let invisible = player.character == Character::Exe && player.invis_timer > 0;
    invisible && matches!(event, SimEvent::Taunt { .. } | SimEvent::Sound { .. }) && !matches!(event, SimEvent::Sound { sound, .. } if *sound == crate::core::resources::names::sound::SND_SPRING)
}

/// The round rules ended the round.
pub fn end(server: &mut Server, ending: crate::core::game::RoundEnding) {
    if let Some(round) = server.game.round.as_mut() {
        round.game.end_round(ending);
    }
}

/// Runs `change` on the simulated player of `player_id` for a round rule (demonizing,
/// reviving); its events go out with the next snapshot.
pub fn change_player(server: &mut Server, player_id: u16, change: impl FnOnce(&mut Player, &GameplayConfig, &mut Vec<SimEvent>)) {
    let Some(round) = server.game.round.as_mut() else { return };
    let Some(index) = round.players.iter().position(|player| player.id == player_id) else { return };
    let config = round.game.world.config.clone();
    let mut events = Vec::new();
    change(&mut round.game.players[index], &config, &mut events);
    round.rule_events.extend(events.into_iter().map(|event| (EventSource::DoneTo(index), event)));
}

/// scr_survivor_heal of every survivor over every teammate. Returns who healed someone.
fn heal_teammates(round: &mut Round) -> Vec<usize> {
    let count = round.game.players.len();
    let config = round.game.world.config.clone();
    let mut healers = Vec::new();
    for healer in 0..count {
        for target in 0..count {
            let (healer_player, target_player) = (&round.game.players[healer], &round.game.players[target]);
            if healer == target || healer_player.removed || target_player.removed {
                continue;
            }
            let step = crate::core::heal::heal_step(&round.game.world, &config, &mut round.heal_progress[healer * count + target], healer_player, target_player);
            let (x, y) = (target_player.x, target_player.y);
            if step.sparkles {
                round.rule_events.push((EventSource::World, SimEvent::HealSparkles { x, y }));
            }
            if !step.healed {
                continue;
            }
            // CLIENT_PLAYER_HEAL reaching the teammate's client.
            let target_player = &mut round.game.players[target];
            if target_player.hp < config.hurt.max_hp {
                target_player.hp += config.hurt.heal_hp;
                round.rule_events.push((EventSource::DoneTo(target), SimEvent::Sound { sound: crate::core::resources::names::sound::SND_HEAL, x, y }));
            }
            round.game.players[healer].rings -= config.rings.teammate_heal_rings;
            round.rule_events.push((EventSource::World, SimEvent::TeammateHealed { healer, target }));
            healers.push(healer);
        }
    }
    healers
}

/// Reviver, downed survivor and the reviver's rings beyond three, for every
/// survivor looking down on a downed teammate this tick (scr_survivor_revive).
fn revivals(round: &Round) -> Vec<(usize, usize, u8)> {
    let world = &round.game.world;
    let players = &round.game.players;
    let mut found = Vec::new();
    for (target_index, target) in players.iter().enumerate() {
        let can_be_revived = !target.removed && target.hp <= 0 && target.revival_times == 0 && target.character != Character::Exe;
        if !can_be_revived {
            continue;
        }
        for (reviver_index, reviver) in players.iter().enumerate() {
            let can_revive = !reviver.removed && reviver.character != Character::Exe && reviver.revival_times < 2 && reviver.hp > 0;
            let reviving = reviver.controls_enabled && reviver.is_looking_down && reviver.rings >= REVIVAL_RINGS;
            if reviver_index != target_index && can_revive && reviving && crate::core::contact::bodies_touch(world, target, reviver) {
                found.push((reviver_index, target_index, (reviver.rings - REVIVAL_RINGS).clamp(0, u8::MAX as i32) as u8));
            }
        }
    }
    found
}

/// CLIENT_STATS_REPORT 3 of the original, which every survivor's scr_player_hurt sent:
/// the health a survivor lost this tick, from whatever took it (hits, hazards, black
/// rings). A demon loses none it counts, and EXE none at all.
fn count_damage_taken(round: &mut Round, server: &mut Server) {
    for (round_player, player) in round.players.iter_mut().zip(&round.game.players) {
        let lost = round_player.last_hp - player.hp;
        round_player.last_hp = player.hp;
        if lost <= 0 || player.character == Character::Exe || player.is_demonized() {
            continue;
        }
        if let Some(peer) = server.find_peer_mut(round_player.id) {
            peer.plr.stats.damage_taken = peer.plr.stats.damage_taken.saturating_add(lost as u16);
        }
        crate::status::with_status(|status| status.damage_taken += lost as u32);
    }
}

/// What the round rules of states::game make of the simulation's events. `hps`: every
/// player's health after the tick, by their index in the round.
// ponytail: mercoin bonuses are not kept: the port has no currency
fn apply_round_rules(ids: &[u16], hps: &[i32], events: &[(EventSource, SimEvent)], server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    for (source, event) in events {
        let player = match source {
            EventSource::Player(index) | EventSource::DoneTo(index) => ids.get(*index).copied(),
            EventSource::World => None,
        };
        match event {
            SimEvent::PlayerDied { revival_times } => {
                if let Some(id) = player {
                    crate::states::game::player_death_state(id, true, *revival_times as u8, server, outbox);
                }
            }
            // CLIENT_STATS_REPORT 1 and 2 of the original: a stun counts for both players,
            // a hit's damage for the attacker, and a kill when it left the victim at 0.
            SimEvent::PlayerHit { victim, attacker, damage, stun_seconds } => {
                if *stun_seconds > 0.0 {
                    if let Some(peer) = ids.get(*victim).and_then(|&id| server.find_peer_mut(id)) {
                        peer.plr.stats.stun_time = peer.plr.stats.stun_time.saturating_add(stun_seconds.round() as u16);
                    }
                    if let Some(peer) = ids.get(*attacker).and_then(|&id| server.find_peer_mut(id)) {
                        peer.plr.stats.stuns += 1;
                    }
                }
                if *damage > 0 {
                    let killed = hps.get(*victim).is_some_and(|&hp| hp <= 0);
                    if let Some(peer) = ids.get(*attacker).and_then(|&id| server.find_peer_mut(id)) {
                        peer.plr.stats.damage = peer.plr.stats.damage.saturating_add(*damage as u16);
                        if killed {
                            peer.plr.stats.kills += 1;
                        }
                    }
                }
            }
            // The rings are the simulation's now (core/game.rs and core/level), so the rules
            // only keep the count the results screen shows.
            SimEvent::RingTaken { red: false, .. } => {
                if let Some(peer) = player.and_then(|id| server.find_peer_mut(id)) {
                    peer.plr.stats.rings += 1;
                }
            }
            _ => {}
        }
    }
}


/// A snapshot to every client that is following the round: its players, and the
/// spectators watching it (states::spectate).
fn send_snapshots(round: &Round, server: &Server, outbox: &mut Vec<OutboxMsg>) {
    let everyone: Vec<OtherPlayer> = round
        .players
        .iter()
        .zip(&round.game.players)
        .filter(|(_, simulated)| !simulated.removed)
        .map(|(player, simulated)| OtherPlayer { id: player.id, view: Some(PlayerView::of(simulated)), health: HealthView::of(simulated) })
        .collect();
    let ids: Vec<u16> = round.players.iter().map(|player| player.id).collect();
    let entities = entity_views(&round.game, &ids);
    let level = round.game.level.as_ref().map(crate::core::level::Level::view).unwrap_or_default();

    for (index, player) in round.players.iter().enumerate() {
        let snapshot = Snapshot {
            tick: round.tick,
            acked_input: player.last_applied,
            own: Some(round.game.players[index].clone()).filter(|own| !own.removed),
            others: others_for(round, &everyone, Some(index)),
            events: events_for(round, Some(index)),
            entities: entities.clone(),
            level: level.clone(),
        };
        send_snapshot(player.id, snapshot, outbox);
    }

    // A spectator plays nobody, so there is no own player in their snapshot and nothing
    // is left out of it: they are watching the round, not playing in it.
    for peer in server.peers.iter().filter(|peer| peer.is_spectating) {
        let snapshot = Snapshot {
            tick: round.tick,
            acked_input: None,
            own: None,
            others: everyone.clone(),
            events: events_for(round, None),
            entities: entities.clone(),
            level: level.clone(),
        };
        send_snapshot(peer.id, snapshot, outbox);
    }
}

fn send_snapshot(player_id: u16, snapshot: Snapshot, outbox: &mut Vec<OutboxMsg>) {
    let mut packet = vec![0, PacketType::SERVER_ROUND_SNAPSHOT as u8];
    packet.extend(snapshot.encode());
    outbox.push(OutboxMsg::SendTo(player_id, packet, false));
}

/// The others as one client is told about them. `watcher` is the index of its own
/// player, or None for a spectator, who is told about everyone.
fn others_for(round: &Round, everyone: &[OtherPlayer], watcher: Option<usize>) -> Vec<OtherPlayer> {
    let Some(watcher) = watcher else { return everyone.to_vec() };
    let own_id = round.players[watcher].id;
    let seen = seen_by(round, watcher);
    everyone
        .iter()
        .filter(|other| other.id != own_id)
        .map(|other| match seen.as_ref() {
            // Where they are is left out, what the health row says about them is not.
            Some(seen) if !seen.contains(&other.id) => OtherPlayer { view: None, ..other.clone() },
            _ => other.clone(),
        })
        .collect()
}

/// The events of this tick one client is told about. A client predicted its own
/// player's events, and the others' private ones (camera shake, the red flash of a
/// hit, sounds only they hear) are not its business; a spectator, who predicted
/// nothing and is nobody's victim, gets what is left.
fn events_for(round: &Round, watcher: Option<usize>) -> Vec<(EventSource, SimEvent)> {
    round
        .events
        .iter()
        .filter(|(source, event)| match source {
            EventSource::Player(owner) => Some(*owner) != watcher && !event.is_private() && !silenced_by_invisibility(&round.game.players[*owner], event),
            EventSource::DoneTo(victim) => Some(*victim) == watcher || !event.is_private(),
            EventSource::World => !event.is_private(),
        })
        .cloned()
        .collect()
}

/// The ids of the players `watcher` may be told the place of, or None while the
/// server tells everyone about everyone (states.gameplay.limit_player_view off, and
/// for anyone who is down: their camera is already watching the others).
///
/// What a player may see is their own window, and whoever the game points out to
/// them wherever they stand: the arrows of the indicators are drawn from those
/// players (client/screens/level/indicators.rs), so leaving them out of the snapshot
/// would quietly take the arrows away.
fn seen_by(round: &Round, watcher: usize) -> Option<Vec<u16>> {
    let own = round.game.players.get(watcher)?;
    if !cfg().states.gameplay.limit_player_view || own.hp <= 0 || own.is_dead || own.removed {
        return None;
    }
    let world = &round.game.world;
    let room = (world.room_width, world.room_height);
    // Limp City: an eye's camera shows what is around the other eye as well.
    let windows: Vec<_> = std::iter::once((own.x, own.y))
        .chain(crate::core::level::watched_eye(world, own))
        .map(|(x, y)| view_around(x, y, room))
        .collect();
    let body = |player: &Player| crate::core::collision::sprite_bbox(world.sprites.get(player.sprite_index), player.x, player.y, player.image_xscale, 1.0, 0.0);
    let mut seen: Vec<u16> = round
        .players
        .iter()
        .zip(&round.game.players)
        .filter(|(_, player)| windows.iter().any(|window| window.overlaps(&body(player))))
        .map(|(player, _)| player.id)
        .collect();

    let also_show = |index: usize, seen: &mut Vec<u16>| {
        if let Some(player) = round.players.get(index) {
            seen.push(player.id);
        }
    };
    let nearest = |wanted: &dyn Fn(&Player) -> bool| {
        round
            .game
            .players
            .iter()
            .enumerate()
            .filter(|(index, player)| *index != watcher && !player.removed && wanted(player))
            .min_by(|a, b| distance_between(own, a.1).total_cmp(&distance_between(own, b.1)))
            .map(|(index, _)| index)
    };
    if own.character == Character::Exe {
        // obj_exe_indicator: the nearest survivor, and whoever carries a red ring.
        if let Some(index) = nearest(&crate::core::contact::hunted) {
            also_show(index, &mut seen);
        }
        for (index, _) in round.game.players.iter().enumerate().filter(|(_, player)| crate::core::contact::hunted(player) && player.red_ring_timer > 0) {
            also_show(index, &mut seen);
        }
        // obj_exeller_indicator_up: this Exeller's own clones show what they see.
        let revealed: Vec<usize> = round.game.exeller_clones.iter().filter(|clone| clone.owner == watcher).filter_map(|clone| clone.revealed).collect();
        for index in revealed {
            also_show(index, &mut seen);
        }
    }
    // obj_demon_indicator: a demon is shown the nearest killer.
    if own.is_demonized() {
        if let Some(index) = nearest(&|player: &Player| player.character == Character::Exe) {
            also_show(index, &mut seen);
        }
    }
    // obj_surv_indicator: whoever a tracker of this player's side caught, while it shows them.
    if let Some((victim, _)) = round.players[watcher].tracked {
        also_show(victim, &mut seen);
    }
    Some(seen)
}

/// The window a camera looking at (`x`, `y`) holds, grown by VIEW_MARGIN: centred on
/// that point and kept inside the room, as obj_camera keeps it (client camera.rs Camera::step).
fn view_around(x: f64, y: f64, room: (f64, f64)) -> crate::core::collision::Bbox {
    let corner = |position: f64, size: f64, room: f64| (position - size / 2.0).clamp(0.0, (room - size).max(0.0));
    let left = corner(x, crate::core::VIEW_WIDTH, room.0);
    let top = corner(y, crate::core::VIEW_HEIGHT, room.1);
    crate::core::collision::Bbox {
        left: left - VIEW_MARGIN,
        top: top - VIEW_MARGIN,
        right: left + crate::core::VIEW_WIDTH + VIEW_MARGIN,
        bottom: top + crate::core::VIEW_HEIGHT + VIEW_MARGIN,
    }
}

fn distance_between(own: &Player, other: &Player) -> f64 {
    (own.x - other.x).hypot(own.y - other.y)
}

/// SimEvent::TrackerCaught: the catch is shown to the tracking Eggman's whole side, the
/// survivors or, for a demon Eggman, the killers, as long as their clients draw the
/// arrow (level/mod.rs tracker_caught), so a cut view keeps it (see seen_by).
fn track_catches(round: &mut Round) {
    let catches: Vec<(usize, usize)> = round
        .events
        .iter()
        .filter_map(|(_, event)| match event {
            SimEvent::TrackerCaught { eggman, victim } => Some((*eggman, *victim)),
            _ => None,
        })
        .collect();
    for player in round.players.iter_mut() {
        player.tracked = player.tracked.and_then(|(victim, left)| (left > 1).then_some((victim, left - 1)));
    }
    for (eggman, victim) in catches {
        let Some(tracker_side) = round.game.players.get(eggman).map(Player::is_killers_side) else { continue };
        for (player, body) in round.players.iter_mut().zip(&round.game.players) {
            if body.is_killers_side() == tracker_side {
                player.tracked = Some((victim, crate::core::config::ticks(TRACKED_SECONDS)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The window a player is told about follows their camera, which is kept inside
    /// the room: someone standing in a corner sees the whole screen the camera shows
    /// them, not half of it.
    #[test]
    fn the_view_of_a_player_in_a_corner_is_the_camera_s_own() {
        let room = (2000.0, 1000.0);
        let middle = view_around(1000.0, 500.0, room);
        assert_eq!(
            (middle.left, middle.right),
            (1000.0 - crate::core::VIEW_WIDTH / 2.0 - VIEW_MARGIN, 1000.0 + crate::core::VIEW_WIDTH / 2.0 + VIEW_MARGIN),
            "centred on a player in the open"
        );

        let corner = view_around(20.0, 20.0, room);
        assert!(corner.right >= crate::core::VIEW_WIDTH, "the camera stops at the wall, so the view reaches the far side of the screen: {corner:?}");
        assert!(corner.bottom >= crate::core::VIEW_HEIGHT, "and downwards too: {corner:?}");

        // A room smaller than the screen is all of it, whatever the clamp does.
        let small = view_around(10.0, 10.0, (100.0, 100.0));
        assert!(small.right >= 100.0 && small.bottom >= 100.0, "{small:?}");
    }

    /// EXE (id 1) standing next to Tails (id 2) on Green Hill, attacking for a moment;
    /// Tails in their pause menu or not. Returns Tails' health before and after, and the server.
    fn exe_attacks_tails(tails_paused: bool) -> (i32, i32, Server) {
        crate::config::set_test_config();
        crate::status::init_empty_status();
        let mut server = Server::new(0);
        server.game.map = LEVEL_ROOMS.iter().position(|&room| room == crate::core::rooms::ids::RoomId::Greenhill).unwrap() as i8;
        server.game.started = true;
        server.lobby.exes = vec![1];
        for (id, exe) in [(1, true), (2, false)] {
            let mut peer = crate::server::PeerData::new(id, format!("10.0.0.{id}"));
            peer.in_game = true;
            if exe {
                peer.exe_char = ExeChar::Original;
            } else {
                peer.surv_char = SurvChar::Tails;
            }
            server.peers.push(peer);
        }
        let mut outbox = Vec::new();
        start(&mut server, &mut outbox);
        {
            let round = server.game.round.as_mut().expect("the round starts");
            let (x, y) = (round.game.players[0].x, round.game.players[0].y);
            (round.game.players[1].x, round.game.players[1].y) = (x + 16.0, y);
            round.game.players[0].image_xscale = 1.0;
        }
        for _ in 0..120 {
            tick(&mut server, &mut outbox);
        }
        let mut pause = Packet::new(PacketType::CLIENT_ROUND_PAUSE);
        let _ = pause.write_u8(u8::from(tails_paused));
        receive_pause(2, &mut pause, &mut server);
        let before = server.game.round.as_ref().unwrap().game.players[1].hp;
        for client_tick in 0..48 {
            let round = server.game.round.as_mut().unwrap();
            let buttons = if client_tick < 2 { Buttons::B } else { 0 };
            round.players[0].queued.push_back((client_tick, Buttons(buttons)));
            round.players[1].queued.push_back((client_tick, Buttons::default()));
            tick(&mut server, &mut outbox);
        }
        let after = server.game.round.as_ref().unwrap().game.players[1].hp;
        (before, after, server)
    }

    #[test]
    fn a_player_in_the_pause_menu_is_hit_like_anyone() {
        let (before, after, _) = exe_attacks_tails(false);
        assert!(after < before, "the attack lands at all: {before} -> {after}");
        let (before, after, _) = exe_attacks_tails(true);
        assert!(after < before, "and lands on a paused Tails too: {before} -> {after}");
    }

    /// CLIENT_STATS_REPORT of the original: the hit counts as damage for EXE and as
    /// damage taken for Tails, for the results screen.
    #[test]
    fn a_hit_counts_for_the_results_on_both_sides() {
        let (before, after, server) = exe_attacks_tails(false);
        let lost = (before - after) as u16;
        assert!(lost > 0);
        let stats = |id: u16| server.find_peer(id).unwrap().plr.stats.clone();
        assert_eq!(stats(1).damage, lost, "EXE's damage");
        assert_eq!(stats(2).damage_taken, lost, "Tails' damage taken");
    }
}
