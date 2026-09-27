//! One running round: the room, every player and the shots in flight.
//! The server steps this with everyone's buttons; the client steps a copy
//! to predict its own player.

use crate::core::egg_tracker::EggTracker;
use crate::core::events::{EventSource, SimEvent};
use crate::core::exeller_clone::ExellerClone;
use crate::core::level::Level;
use crate::core::player::{Buttons, Player};
use crate::core::stomp_wave::{StompWave, WaveOutcome};
use crate::core::tails_projectile::{Outcome, TailsProjectile};
use crate::core::objects::ids::ObjectId;
use crate::core::world::{ObjectVars, World};

/// How a round ended (SERVER_GAME_EXE_WINS, SERVER_GAME_SURVIVOR_WIN, SERVER_GAME_TIME_OVER).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RoundEnding {
    ExeWon,
    SurvivorsEscaped,
    TimeOver,
}

pub struct Game {
    pub world: World,
    pub players: Vec<Player>,
    pub projectiles: Vec<TailsProjectile>,
    pub trackers: Vec<EggTracker>,
    pub stomp_waves: Vec<StompWave>,
    pub exeller_clones: Vec<ExellerClone>,
    pub shield_shards: Vec<crate::core::shield_shards::ShieldShards>,
    /// The map's own objects the server decides (None in a client's game, which applies
    /// the server's views instead).
    pub level: Option<Level>,
    /// Ids for entities that players refer to later (Exeller clones).
    next_entity_id: i32,
    /// Who made each event the last tick returned, in the same order.
    pub event_sources: Vec<EventSource>,
}

impl Game {
    pub fn new(world: World) -> Game {
        Game {
            world,
            players: Vec::new(),
            projectiles: Vec::new(),
            trackers: Vec::new(),
            stomp_waves: Vec::new(),
            exeller_clones: Vec::new(),
            shield_shards: Vec::new(),
            level: None,
            next_entity_id: 0,
            event_sources: Vec::new(),
        }
    }

    /// The round is over: nobody can be hurt any more, and the players stop as each
    /// client of the original stopped its own player on the ending packet.
    pub fn end_round(&mut self, ending: RoundEnding) {
        self.world.game_ends = true;
        for player in &mut self.players {
            let is_exe = player.character == crate::core::player::Character::Exe;
            match ending {
                RoundEnding::ExeWon if is_exe => {
                    player.won = true;
                    player.controls_enabled = false;
                }
                RoundEnding::ExeWon => {}
                RoundEnding::TimeOver => {
                    player.won |= is_exe;
                    player.controls_enabled = false;
                }
                RoundEnding::SurvivorsEscaped => {
                    player.lost |= is_exe;
                    player.controls_enabled = false;
                }
            }
        }
    }

    /// One tick. `buttons[i]` belongs to `players[i]`; a missing entry means nothing pressed.
    // ponytail: players then shots, each in list order; GameMaker interleaves all instances per event
    pub fn tick(&mut self, buttons: &[Buttons]) -> Vec<SimEvent> {
        let mut events = Vec::new();
        self.world.animate();
        self.world.fade_in_black_rings();
        // The server's map entities, before the room's instances and players step.
        // A client in a round has no level: it is shown what the server's level did,
        // so nothing here puts out rings or takes them twice.
        if let Some(level) = self.level.as_mut() {
            level.tick(&self.world, &mut self.players, &mut events);
            let mut rings = std::mem::take(&mut self.world.rings);
            level.put_out_rings(&self.world, &mut rings);
            self.world.rings = rings;
            level.view().apply(&mut self.world);
        }
        for event in &events {
            if let SimEvent::StageSwitched { dx, dy } = *event {
                for clone in &mut self.exeller_clones {
                    clone.move_to_next_stage(&self.world, dx, dy);
                }
            }
        }
        let mut sources: Vec<EventSource> = events.iter().map(|event| event.done_to().map_or(EventSource::World, EventSource::DoneTo)).collect();

        for index in 0..self.players.len() {
            if self.players[index].removed {
                continue;
            }
            let pressed = buttons.get(index).copied().unwrap_or_default();
            let first_new_event = events.len();
            self.players[index].tick(&self.world, pressed, &mut events);
            spawn_projectiles(&self.world.config, &mut self.projectiles, index, &events[first_new_event..]);
            spawn_trackers(&mut self.trackers, index, &events[first_new_event..]);
            spawn_stomp_waves(&self.world.config, &mut self.stomp_waves, index, &events[first_new_event..]);
            self.handle_clone_requests(index, first_new_event, &mut events);
            let vases_hit: Vec<u8> = events[first_new_event..].iter().filter_map(|event| match *event {
                SimEvent::VaseHit { nid } => Some(nid),
                _ => None,
            }).collect();
            if let (Some(level), Some(&SimEvent::DummyPushed { speed })) = (self.level.as_mut(), events[first_new_event..].iter().find(|event| matches!(event, SimEvent::DummyPushed { .. }))) {
                level.push_dummy(speed, None);
            }
            for event in &events[first_new_event..] {
                match (event, self.level.as_mut()) {
                    (&SimEvent::MonitorHit { nid }, Some(level)) => level.hit_monitor(&self.world.config, nid, Some(&mut self.players[index])),
                    (&SimEvent::IceBlockHit { nid }, Some(level)) => level.break_ice(&self.world.config, nid),
                    (&SimEvent::EyeRequest { eye, target, on }, Some(level)) => level.request_eye(&self.world.config, index, &mut self.players[index], eye, target, on),
                    (&SimEvent::CrystalUsed, Some(level)) => level.toggle_doors(&self.world.config),
                    (&SimEvent::LiftTouched { lift }, Some(level)) => level.board_lift(&self.world, lift, index, &mut self.players[index]),
                    (&SimEvent::SpawnBlackRing { x, y }, _) => self.world.place_black_ring(x, y, Some(index)),
                    // scr_cream_special: her rings, which the round rules used to place.
                    (&SimEvent::SpawnCreamRings { x, y, demonized }, Some(_)) => {
                        const AROUND: [(f64, f64); 3] = [(26.0, 0.0), (0.0, -26.0), (-27.0, 0.0)];
                        const RED: [(f64, f64); 2] = [(25.0, 0.0), (-27.0, 0.0)];
                        let places: &[(f64, f64)] = if demonized { &RED } else { &AROUND };
                        for (offset_x, offset_y) in places {
                            self.world.rings.push(crate::core::rings::MapRing::new(x + offset_x, y + offset_y, demonized));
                        }
                    }
                    _ => {}
                }
            }
            sources.resize(events.len(), EventSource::Player(index));
            let slugs_hit: Vec<u16> = events[first_new_event..].iter().filter_map(|event| match *event {
                SimEvent::SlugHit { id } => Some(id),
                _ => None,
            }).collect();
            // What the server gives back is not predicted by the player's client.
            if let Some(level) = self.level.as_mut() {
                for id in slugs_hit {
                    level.hit_slug(&self.world, id, Some(&mut self.players[index]), &mut events);
                }
                for nid in vases_hit {
                    level.break_vase(&self.world, nid, &mut self.players[index], &mut events);
                }
            }
            sources.resize(events.len(), EventSource::DoneTo(index));
        }
        let before_contact = self.players.clone();
        let cfg = self.world.config.clone();
        for (index, victim) in self.players.iter_mut().enumerate().filter(|(_, player)| !player.removed) {
            crate::core::contact::resolve_victim(&self.world, &cfg, victim, index, &before_contact, &mut events);
            sources.resize(events.len(), EventSource::DoneTo(index));
        }
        self.break_shields(&cfg, &mut events, &mut sources);
        // obj_fart_controller Step: the curse passes to the first player its holder touches.
        for holder in 0..self.players.len() {
            if self.players[holder].removed || !self.players[holder].can_pass_the_curse(&cfg) {
                continue;
            }
            let body = |player: &Player| crate::core::collision::sprite_bbox(self.world.sprites.get(player.sprite_index), player.x, player.y, player.image_xscale, 1.0, 0.0);
            let holder_body = body(&self.players[holder]);
            let touched = (0..self.players.len()).find(|&other| other != holder && !self.players[other].removed && body(&self.players[other]).overlaps(&holder_body));
            if let Some(other) = touched {
                self.players[other].potato_ticks = crate::core::config::ticks(cfg.levels.fart_zone.potato_seconds);
                self.players[holder].potato_ticks = 0;
            }
        }
        self.touch_rings(&mut events, &mut sources);
        // obj_blackring Step: survivors take the rings they touch, in player order.
        for index in 0..self.players.len() {
            let Some(ring) = self.world.black_ring_taken_by(&self.players[index]) else { continue };
            self.world.black_rings.remove(ring);
            self.players[index].take_black_ring(index, &self.world, &cfg, &mut events);
            sources.resize(events.len(), EventSource::DoneTo(index));
        }

        let (world, players) = (&self.world, &mut self.players);
        self.projectiles.retain_mut(|shot| matches!(shot.tick(world, players, &mut events), Outcome::Alive));
        // obj_kaf_speedbox End Step and obj_nap_iceblock Step: a Tails shot breaks a
        // monitor or an ice block too.
        for shot in &self.projectiles {
            let shot_box = shot.bbox(&self.world);
            for block in self.world.ids_of(ObjectId::NapIceblock) {
                let world = &self.world;
                if let (ObjectVars::IceBlock { nid }, true, Some(bbox)) = (&world.instances[block].vars, world.instances[block].visible, world.bbox(block)) {
                    if let Some(level) = self.level.as_mut().filter(|_| bbox.overlaps(&shot_box)) {
                        level.break_ice(&world.config, *nid);
                    }
                }
            }
            for monitor in self.world.ids_of(ObjectId::KafSpeedbox) {
                let world = &self.world;
                if let (ObjectVars::Monitor { nid, broken: false }, Some(bbox)) = (&world.instances[monitor].vars, world.bbox(monitor)) {
                    if bbox.overlaps(&shot_box) {
                        if let Some(level) = self.level.as_mut() {
                            level.hit_monitor(&world.config, *nid, None);
                        }
                    }
                }
            }
        }
        self.trackers.retain_mut(|tracker| tracker.tick(world, players, &mut events));
        self.tick_stomp_waves(&mut events);
        for clone in &mut self.exeller_clones {
            clone.tick(&self.world, &self.players);
        }
        self.hit_dummy_with_shots_and_waves(&mut events);
        self.squash_slugs_with_shots_and_waves(&mut events);
        sources.resize(events.len(), EventSource::World);
        self.event_sources = sources;
        events
    }

    /// A demonized Sally's broken shield (spr_shieldbreak2) hurts the survivors who touch
    /// it while the break plays; the breaks of this tick start hurting from the next one.
    fn break_shields(&mut self, cfg: &crate::core::config::GameplayConfig, events: &mut Vec<SimEvent>, sources: &mut Vec<EventSource>) {
        let new_breaks: Vec<(usize, f64, f64)> = events
            .iter()
            .zip(sources.iter())
            .filter_map(|(event, source)| match (event, source) {
                (&SimEvent::Effect { sprite, x, y, .. }, &(EventSource::Player(owner) | EventSource::DoneTo(owner))) if sprite == crate::core::resources::names::sprite::SPR_SHIELDBREAK2 => Some((owner, x, y)),
                _ => None,
            })
            .collect();
        let (world, players) = (&self.world, &mut self.players);
        self.shield_shards.retain_mut(|shards| shards.tick(world, cfg, players, events));
        sources.resize(events.len(), EventSource::World);
        for (owner, x, y) in new_breaks {
            self.shield_shards.push(crate::core::shield_shards::ShieldShards::new(&self.world, owner, x, y));
        }
    }

    /// obj_ring and obj_redring Step: whoever owns the level decides who takes a ring
    /// or breaks it, as the round rules used to. A client in a round has no level and
    /// only draws the rings the server sent it.
    fn touch_rings(&mut self, events: &mut Vec<SimEvent>, sources: &mut Vec<EventSource>) {
        if self.level.is_none() {
            return;
        }
        let cfg = self.world.config.clone();
        let mut touched = Vec::new();
        let mut rings = std::mem::take(&mut self.world.rings);
        let projectiles = &self.projectiles;
        rings.retain_mut(|ring| match ring.tick(&self.world, &cfg, &self.players).or_else(|| ring.broken_by_shot(&self.world, projectiles).map(crate::core::rings::RingTouch::Broken)) {
            Some(touch) => {
                touched.push((touch, crate::core::rings::MapRing::new(ring.x, ring.y, ring.red)));
                false
            }
            None => true,
        });
        self.world.rings = rings;
        for (touch, ring) in touched {
            let index = match touch {
                crate::core::rings::RingTouch::Taken(index) | crate::core::rings::RingTouch::Broken(index) => index,
            };
            let Some(player) = self.players.get_mut(index) else { continue };
            let can_heal = player.rings > 0;
            match touch {
                crate::core::rings::RingTouch::Taken(_) => {
                    player.take_ring(&self.world, &cfg, ring.red, can_heal, events);
                    events.push(SimEvent::RingTaken { x: ring.x, y: ring.y, red: ring.red });
                }
                crate::core::rings::RingTouch::Broken(_) => player.break_ring(&ring, events),
            }
            sources.resize(events.len(), EventSource::DoneTo(index));
        }
    }

    /// obj_rmzsonic Step: a shot or a stomp wave squashes an awake slug.
    fn squash_slugs_with_shots_and_waves(&mut self, events: &mut Vec<SimEvent>) {
        let (Some(level), Some(ravine_mist)) = (self.level.as_mut(), &self.world.ravine_mist) else { return };
        let world = &self.world;
        for slug in ravine_mist.slugs.iter().filter(|slug| slug.awake()) {
            let slug_box = slug.bbox(world);
            let shot = self.projectiles.iter().any(|shot| shot.bbox(world).overlaps(&slug_box));
            let wave = self.stomp_waves.iter().any(|wave| wave.visible && wave.bbox(world).overlaps(&slug_box));
            if shot || wave {
                level.hit_slug(world, slug.id, None, events);
            }
        }
    }

    /// obj_fart_dummy Step: a shot that is still flying or a stomp wave pushes the dummy
    /// (every client saw it, so everyone is told).
    fn hit_dummy_with_shots_and_waves(&mut self, events: &mut Vec<SimEvent>) {
        let Some(dummy) = self.world.ids_of(ObjectId::FartDummy).next() else { return };
        let Some(level) = self.level.as_mut() else { return };
        let world = &self.world;
        let (Some(dummy_box), ObjectVars::Dummy { resting: false, .. }) = (world.bbox(dummy), &world.instances[dummy].vars) else { return };
        let (x, y) = (world.instances[dummy].x, world.instances[dummy].y);
        let rules = &world.config.levels.fart_zone;
        if let Some(shot) = self.projectiles.iter_mut().find(|shot| !shot.is_breaking && shot.bbox(world).overlaps(&dummy_box)) {
            let worth = if shot.hurts_survivors { -shot.damage / 20 } else { shot.damage };
            level.push_dummy(crate::core::player::gm_sign(x - shot.x) * shot.charge as f64, Some(crate::core::config::original_ticks(rules.dummy_rest_ticks)));
            events.push(SimEvent::DummyHit { worth, x, y, everyone: true });
            shot.start_breaking(&world.config);
            return;
        }
        if let Some(wave) = self.stomp_waves.iter().find(|wave| wave.visible && wave.bbox(world).overlaps(&dummy_box)) {
            // The original pushed by the facing of whichever player's client saw it; the stomper's here.
            let facing = self.players.get(wave.owner).map_or(1.0, |owner| owner.image_xscale);
            level.push_dummy(facing, Some(crate::core::config::original_ticks(rules.dummy_wave_rest_ticks)));
            events.push(SimEvent::DummyHit { worth: -1, x, y, everyone: true });
        }
    }

    /// Clone placement and teleport requests Exeller made during its tick.
    fn handle_clone_requests(&mut self, owner: usize, first_new_event: usize, events: &mut Vec<SimEvent>) {
        let requests: Vec<SimEvent> = events[first_new_event..]
            .iter()
            .filter(|event| matches!(event, SimEvent::SpawnExellerClone { .. } | SimEvent::TeleportToClone { .. }))
            .cloned()
            .collect();
        let cfg = self.world.config.clone();
        for request in requests {
            match request {
                SimEvent::SpawnExellerClone { x, y, dir } => {
                    // The server's limit (entities/exeller_clone.rs), counted per Exeller:
                    // the original had one Exeller per round, a round here may have several.
                    let owned_clones = self.exeller_clones.iter().filter(|clone| clone.owner == owner).count();
                    if owned_clones as i32 >= cfg.exeller.max_clones {
                        continue;
                    }
                    let id = self.next_entity_id;
                    self.next_entity_id += 1;
                    self.exeller_clones.push(ExellerClone::new(id, owner, x, y, dir));
                    self.players[owner].exeller_clone_placed(id);
                    events.push(SimEvent::Sound { sound: crate::core::resources::names::sound::SND_EXELLER_CLONE, x, y });
                    let effect_y = y + cfg.exeller.clone_effect_offset_y;
                    events.push(SimEvent::Effect { sprite: crate::core::resources::names::sprite::SPR_RING_TELEPORT, x, y: effect_y, xscale: 1.0, image_speed: 2.0, yspd: 0.0 });
                }
                SimEvent::TeleportToClone { slot } => {
                    let wanted = self.players[owner].clones[slot as usize];
                    let Some(position) = self.exeller_clones.iter().position(|clone| clone.id == wanted) else { continue };
                    let clone = self.exeller_clones.remove(position);
                    self.players[owner].exeller_teleported(&cfg, slot, clone.x, clone.y, events);
                }
                _ => {}
            }
        }
    }

    /// Waves created this tick start moving on the next one, like instances created during a GameMaker event.
    fn tick_stomp_waves(&mut self, events: &mut Vec<SimEvent>) {
        let (world, players) = (&self.world, &mut self.players);
        let mut new_waves = Vec::new();
        self.stomp_waves.retain_mut(|wave| match wave.tick(world, players, events) {
            WaveOutcome::Alive => true,
            WaveOutcome::AliveAndSpawns(next) => {
                new_waves.push(next);
                true
            }
            WaveOutcome::Gone => false,
        });
        self.stomp_waves.extend(new_waves);
    }
}

fn spawn_stomp_waves(cfg: &crate::core::config::GameplayConfig, waves: &mut Vec<StompWave>, owner: usize, new_events: &[SimEvent]) {
    for event in new_events {
        if let SimEvent::SpawnStompWaves { x, y } = *event {
            waves.extend(StompWave::pair(cfg, x, y, owner));
        }
    }
}

fn spawn_trackers(trackers: &mut Vec<EggTracker>, owner: usize, new_events: &[SimEvent]) {
    for event in new_events {
        if let SimEvent::SpawnEggTracker { x, y } = *event {
            trackers.push(EggTracker::new(x, y, owner));
        }
    }
}

/// The original client ignored a new Tails shot while one existed, because a round
/// had one Tails. A round here may have several Tails, demonized or not, so the
/// limit is one flying shot per Tails.
fn spawn_projectiles(cfg: &crate::core::config::GameplayConfig, projectiles: &mut Vec<TailsProjectile>, owner: usize, new_events: &[SimEvent]) {
    for event in new_events {
        if let SimEvent::SpawnTailsProjectile { x, y, dir, damage, hurts_survivors, charge } = *event {
            if !projectiles.iter().any(|shot| shot.owner == owner) {
                projectiles.push(TailsProjectile::new(cfg, owner, x, y, dir, damage, hurts_survivors, charge));
            }
        }
    }
}
