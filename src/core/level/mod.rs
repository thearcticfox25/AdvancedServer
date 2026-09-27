//! The maps' own objects that the server decides for everyone: the round entities
//! of the original server (src/entities) and the instance variables their packets
//! set on clients. The server steps them; a snapshot carries only what players may
//! see of them (LevelView, no timers or random state), and every side applies that
//! view to its world, where the players' ticks read it.

mod act9;
mod balls;
mod dark_tower;
mod doors;
mod dummy;
mod echidna_ruins;
mod gas;
mod ice;
mod lanterns;
mod lava;
mod limp_city;
mod lifts;
mod monitors;
mod not_perfect;
mod ravine_mist;
mod snowballs;
mod spikes;
mod thunder;
mod torture_cave;
mod vases;

use crate::core::config::GameplayConfig;
use crate::core::events::SimEvent;
use crate::core::objects::ids::ObjectId;
use crate::core::rooms::ids::RoomId;
use crate::core::world::World;
use serde::{Deserialize, Serialize};

pub use act9::Act9Walls;
pub use dark_tower::{DarkTower, DarkTowerView};
pub use doors::Doors;
pub use gas::Gas;
pub use ice::IceBlocks;
pub use lanterns::Lanterns;
pub use lava::{LavaColumns, LavaView};
pub use limp_city::{hang_eyes, watched_eye, ChainState, LimpCity, LimpCityView};
#[cfg(test)]
pub use limp_city::EyeView;
pub use lifts::{LiftView, Lifts};
pub use monitors::Monitors;
pub use not_perfect::{push_out_of_walls, NotPerfect, NotPerfectView};
pub use ravine_mist::{RavineMist, RavineMistView};
#[cfg(test)]
pub use ravine_mist::{ShardView, SlugRing, SlugView};
pub use snowballs::{SnowballView, Snowballs};
pub use spikes::SpikeClock;
pub use thunder::Thunder;
pub use vases::Vases;

/// The entities a map's init of the original server made (src/maps), where they
/// are ported. A client's game keeps the default, which never changes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Level {
    random: Random,
    thunder: Option<Thunder>,
    spikes: Option<SpikeClock>,
    gas: Option<Gas>,
    monitors: Option<Monitors>,
    doors: Option<Doors>,
    lifts: Option<Lifts>,
    ice: Option<IceBlocks>,
    snowballs: Option<Snowballs>,
    lava: Option<LavaColumns>,
    vases: Option<Vases>,
    dark_tower: Option<DarkTower>,
    not_perfect: Option<NotPerfect>,
    act9_walls: Option<Act9Walls>,
    lanterns: Option<Lanterns>,
    balls: Option<balls::BallSwing>,
    dummy: Option<dummy::Dummy>,
    ravine_mist: Option<RavineMist>,
    echidna_ruins: Option<echidna_ruins::EchidnaRuins>,
    limp_city: Option<LimpCity>,
    torture_cave: Option<torture_cave::TortureCave>,
    /// How far Act 9's walls have closed after the last tick.
    act9_closed: f32,
    /// Ticks until the next ring appears, and how often one does. Only whoever owns
    /// the level counts this down: the server in a round, the game itself in
    /// Singleplayer. A client in a round has no Level, so it never puts out rings of
    /// its own, it is shown the server's.
    ring_timer: i32,
    ring_seconds: f64,
}

/// What clients are told of a level's objects.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LevelView {
    /// Green Hill's water shocks.
    pub water_shocks: Option<bool>,
    /// The frame every moving spike shows.
    pub spike_frame: Option<u8>,
    /// You Can't Run: the nid of the smoke areas full of gas.
    pub gassed_area: Option<u8>,
    /// Kind and Fair: a bit for every broken speed monitor, by nid.
    pub broken_monitors: Option<u16>,
    /// Haunting Dream: whether the doors are open and the crystals can be used, and how
    /// far each door slid open.
    pub doors: Option<(bool, bool, Vec<u16>)>,
    /// Priceless Freedom's lifts.
    pub lifts: Vec<LiftView>,
    /// Nasty Paradise: a bit for every broken ice block, by nid.
    pub broken_ice: Option<u16>,
    /// Nasty Paradise's snowballs by path nid, None while not rolling.
    pub snowballs: Vec<Option<SnowballView>>,
    /// Volcano Valley's lava columns.
    pub lava: Vec<LavaView>,
    /// Volcano Valley: a bit for every broken vase, by nid.
    pub broken_vases: Option<u16>,
    pub dark_tower: Option<DarkTowerView>,
    /// Not Perfect's stage, whether the next one's shadow shows, and whether it just switched.
    pub not_perfect: Option<NotPerfectView>,
    /// How far Act 9's walls have closed, 0 to 1.
    pub act9_walls: Option<f32>,
    /// Weed Zone: the nid of the lit lantern.
    pub lit_lantern: Option<u8>,
    /// Dark Tower's and Fart Zone's balls: where they are between -1 and 1.
    pub ball_swing: Option<f32>,
    /// Fart Zone's training dummy.
    pub dummy: Option<dummy::DummyView>,
    pub ravine_mist: Option<RavineMistView>,
    pub echidna_ruins: Option<echidna_ruins::EchidnaRuinsView>,
    pub limp_city: Option<LimpCityView>,
    pub torture_cave: Option<torture_cave::TortureCaveView>,
}

impl Level {
    /// The objects of a round in `world`'s room; `seed` feeds their random timers.
    pub fn new(world: &World, seed: u64) -> Level {
        let (cfg, room) = (&*world.config, world.room);
        let mut random = Random::new(seed);
        let thunder = (room == Some(RoomId::Greenhill)).then(|| Thunder::new(&cfg.levels.green_hill, &mut random));
        let spikes = matches!(room, Some(RoomId::Youcantrun | RoomId::Dotdotdot | RoomId::Fartzone)).then(SpikeClock::default);
        let gas = (room == Some(RoomId::Youcantrun)).then(Gas::default);
        let monitors = (room == Some(RoomId::Kindandfair)).then(Monitors::default);
        let doors = (room == Some(RoomId::Haundream)).then(Doors::default);
        let lifts = (room == Some(RoomId::Pricelessfreedom)).then(Lifts::default);
        let ice = (room == Some(RoomId::Nastyparadise)).then(IceBlocks::default);
        let snowballs = (room == Some(RoomId::Nastyparadise) && cfg.levels.nasty_paradise.snowballs_roll).then(Snowballs::default);
        let lava = match room {
            Some(RoomId::Volcanovalley) => Some(LavaColumns::new(&cfg.levels.volcano_valley.lava, world, ObjectId::VvLavacolumn, &mut random)),
            Some(RoomId::Marijuna) => Some(LavaColumns::new(&cfg.levels.echidna_ruins.lava, world, ObjectId::MarijunaLavaplatform, &mut random)),
            _ => None,
        };
        let vases = (room == Some(RoomId::Volcanovalley)).then(|| Vases::new(cfg, &mut random));
        let dark_tower = (room == Some(RoomId::Dartower)).then(|| DarkTower::new(world));
        let not_perfect = (room == Some(RoomId::Notperfect)).then(NotPerfect::default);
        let act9_walls = (room == Some(RoomId::Act9) && cfg.levels.act9.walls_close).then(Act9Walls::default);
        let balls = matches!(room, Some(RoomId::Dartower | RoomId::Fartzone)).then(balls::BallSwing::default);
        let dummy = (room == Some(RoomId::Fartzone)).then(dummy::Dummy::default);
        let limp_city = (room == Some(RoomId::Limpcity)).then(LimpCity::new);
        let torture_cave = (room == Some(RoomId::Torturecave)).then(torture_cave::TortureCave::new);
        let echidna_ruins = (room == Some(RoomId::Marijuna)).then(|| echidna_ruins::EchidnaRuins::new(&cfg.levels.echidna_ruins, &mut random));
        let ravine_mist = (room == Some(RoomId::Ravinemist)).then(|| RavineMist::new(&cfg.levels.ravine_mist, &mut random));
        let lanterns = (room == Some(RoomId::Weedzone)).then(|| Lanterns::new(&cfg.levels.weed_zone, &mut random));
        let ring_seconds = cfg.rings.spawn_every_seconds;
        Level {
            random,
            thunder,
            spikes,
            gas,
            monitors,
            doors,
            lifts,
            ice,
            snowballs,
            lava,
            vases,
            dark_tower,
            not_perfect,
            act9_walls,
            act9_closed: 0.0,
            lanterns,
            balls,
            dummy,
            ravine_mist,
            echidna_ruins,
            limp_city,
            torture_cave,
            ring_timer: crate::core::config::ticks(ring_seconds),
            ring_seconds,
        }
    }

    /// A round puts a ring out every so many seconds, which the map and how many are
    /// playing decide (the server's map_ring); Singleplayer keeps the config's own.
    pub fn set_ring_seconds(&mut self, seconds: f64) {
        self.ring_seconds = seconds;
        self.ring_timer = crate::core::config::ticks(seconds);
    }

    /// One ring on a free ring spawner of the map, every `ring_seconds` (the server's
    /// "spawn a ring each time the clock passes a multiple of the coefficient").
    pub(crate) fn put_out_rings(&mut self, world: &World, rings: &mut Vec<crate::core::rings::MapRing>) {
        let rules = &world.config.rings;
        if !rules.enabled || self.ring_seconds <= 0.0 {
            return;
        }
        self.ring_timer -= 1;
        if self.ring_timer > 0 {
            return;
        }
        self.ring_timer = crate::core::config::ticks(self.ring_seconds);
        let spawners: Vec<usize> = world.ids_of(ObjectId::RingSpawner).collect();
        let free: Vec<u8> = (0..spawners.len() as u8)
            .filter(|number| !rings.iter().any(|ring| ring.spawner == Some(*number)))
            .collect();
        if free.is_empty() {
            return;
        }
        let number = free[self.random.below(free.len() as u32) as usize];
        let spawner = spawners[number as usize];
        let (x, y) = (world.instances[spawner].x, world.instances[spawner].y);
        let red = (self.random.below(100) as i32) < rules.red_chance_percent;
        rings.push(crate::core::rings::MapRing::on_spawner(x, y, red, number));
    }

    pub fn tick(&mut self, world: &World, players: &mut [crate::core::player::Player], events: &mut Vec<SimEvent>) {
        let cfg = &world.config;
        if let Some(thunder) = self.thunder.as_mut() {
            thunder.tick(&cfg.levels.green_hill, &mut self.random, events);
        }
        if let Some(spikes) = self.spikes.as_mut() {
            spikes.tick(&cfg.hazards, events);
        }
        if let Some(gas) = self.gas.as_mut() {
            gas.tick(&cfg.levels.you_cant_run, &mut self.random);
        }
        if let Some(monitors) = self.monitors.as_mut() {
            monitors.tick();
        }
        if let Some(doors) = self.doors.as_mut() {
            doors.tick(world);
        }
        if let Some(lifts) = self.lifts.as_mut() {
            lifts.tick(&cfg.levels.priceless_freedom, world, players);
        }
        if let Some(ice) = self.ice.as_mut() {
            ice.tick();
        }
        if let Some(snowballs) = self.snowballs.as_mut() {
            snowballs.tick(&cfg.levels.nasty_paradise, world);
        }
        if let Some(lava) = self.lava.as_mut() {
            let rules = if world.room == Some(RoomId::Marijuna) { &cfg.levels.echidna_ruins.lava } else { &cfg.levels.volcano_valley.lava };
            lava.tick(rules, &mut self.random);
        }
        if let Some(dark_tower) = self.dark_tower.as_mut() {
            dark_tower.tick(cfg, world, players, &mut self.random, events);
        }
        if let Some(not_perfect) = self.not_perfect.as_mut() {
            not_perfect.tick(cfg, world, players, events);
        }
        if let Some(walls) = self.act9_walls.as_mut() {
            self.act9_closed = walls.tick(world);
        }
        if let Some(balls) = self.balls.as_mut() {
            balls.tick(&cfg.levels.dark_tower);
        }
        if let Some(torture_cave) = self.torture_cave.as_mut() {
            torture_cave.tick(&cfg.levels.torture_cave, &mut self.random);
        }
        if let Some(limp_city) = self.limp_city.as_mut() {
            limp_city.tick(&cfg.levels.limp_city, players);
        }
        if let Some(echidna_ruins) = self.echidna_ruins.as_mut() {
            echidna_ruins.tick(&cfg.levels.echidna_ruins, &mut self.random);
        }
        if let Some(ravine_mist) = self.ravine_mist.as_mut() {
            ravine_mist.tick(cfg, world, players, &mut self.random, events);
        }
        if let Some(dummy) = self.dummy.as_mut() {
            dummy.tick(&cfg.levels.fart_zone, world);
        }
        if let Some(lanterns) = self.lanterns.as_mut() {
            lanterns.tick(&cfg.levels.weed_zone, world, &mut self.random);
        }
    }

    /// Someone's hit pushed Fart Zone's dummy; a shot or a wave also makes it rest a while.
    pub fn push_dummy(&mut self, speed: f64, rest_ticks: Option<i32>) {
        if let Some(dummy) = self.dummy.as_mut() {
            dummy.push(speed);
            if let Some(ticks) = rest_ticks {
                dummy.rest(ticks);
            }
        }
    }

    /// `player` asks to look through a Limp City eye (or to stop).
    pub fn request_eye(&mut self, cfg: &GameplayConfig, index: usize, player: &mut crate::core::player::Player, eye: u8, target: u8, on: bool) {
        if let Some(limp_city) = self.limp_city.as_mut() {
            limp_city.request_eye(&cfg.levels.limp_city, index, player, eye, target, on);
        }
    }

    /// A Ravine Mist slug was hit, by `hitter` or by a shot or a wave (None).
    pub fn hit_slug(&mut self, world: &World, id: u16, hitter: Option<&mut crate::core::player::Player>, events: &mut Vec<SimEvent>) {
        if let Some(ravine_mist) = self.ravine_mist.as_mut() {
            ravine_mist.hit_slug(world, &world.config, id, hitter, events);
        }
    }

    /// Ravine Mist: how many shards are no longer lying on the map (0 elsewhere). The
    /// round rules open the big ring by this count.
    pub fn shards_found(&self) -> u8 {
        self.ravine_mist.as_ref().map_or(0, |ravine_mist| ravine_mist.view().found)
    }

    /// Ravine Mist: whether enough shards have been found for the big ring (true elsewhere).
    pub fn big_ring_may_open(&self) -> bool {
        self.ravine_mist.as_ref().is_none_or(RavineMist::enough_found)
    }

    /// `breaker` broke one of Volcano Valley's vases.
    pub fn break_vase(&mut self, world: &World, nid: u8, breaker: &mut crate::core::player::Player, events: &mut Vec<SimEvent>) {
        if let Some(vases) = self.vases.as_mut() {
            vases.hit(world, &world.config, nid, breaker, events);
        }
    }

    /// Someone broke one of Nasty Paradise's ice blocks.
    pub fn break_ice(&mut self, cfg: &GameplayConfig, nid: u8) {
        if let Some(ice) = self.ice.as_mut() {
            ice.hit(&cfg.levels.nasty_paradise, nid);
        }
    }

    /// Someone touched one of Priceless Freedom's lifts.
    pub fn board_lift(&mut self, world: &World, lift: crate::core::world::InstanceId, index: usize, player: &mut crate::core::player::Player) {
        if let Some(lifts) = self.lifts.as_mut() {
            lifts.board(world, lift, index, player);
        }
    }

    /// Someone used one of Haunting Dream's crystals.
    pub fn toggle_doors(&mut self, cfg: &GameplayConfig) {
        if let Some(doors) = self.doors.as_mut() {
            doors.toggle(&cfg.levels.haunting_dream);
        }
    }

    /// A speed monitor was hit by `breaker`'s attack, or by a shot (None).
    pub fn hit_monitor(&mut self, cfg: &GameplayConfig, nid: u8, breaker: Option<&mut crate::core::player::Player>) {
        if let Some(monitors) = self.monitors.as_mut() {
            monitors.hit(&cfg.levels.kind_and_fair, &mut self.random, nid, breaker);
        }
    }

    pub fn view(&self) -> LevelView {
        LevelView {
            water_shocks: self.thunder.as_ref().map(Thunder::water_shocks),
            spike_frame: self.spikes.as_ref().map(SpikeClock::frame),
            gassed_area: self.gas.as_ref().and_then(Gas::gassed_area),
            broken_monitors: self.monitors.as_ref().map(Monitors::broken_mask),
            doors: self.doors.as_ref().map(Doors::view),
            lifts: self.lifts.as_ref().map_or_else(Vec::new, Lifts::view),
            broken_ice: self.ice.as_ref().map(IceBlocks::broken_mask),
            snowballs: self.snowballs.as_ref().map_or_else(Vec::new, Snowballs::view),
            lava: self.lava.as_ref().map_or_else(Vec::new, LavaColumns::view),
            broken_vases: self.vases.as_ref().map(Vases::broken_mask),
            dark_tower: self.dark_tower.as_ref().map(DarkTower::view),
            not_perfect: self.not_perfect.as_ref().map(NotPerfect::view),
            act9_walls: self.act9_walls.as_ref().map(|_| self.act9_closed),
            lit_lantern: self.lanterns.as_ref().and_then(Lanterns::lit),
            ball_swing: self.balls.as_ref().map(balls::BallSwing::swing),
            dummy: self.dummy.as_ref().and_then(dummy::Dummy::view),
            ravine_mist: self.ravine_mist.as_ref().map(RavineMist::view),
            echidna_ruins: self.echidna_ruins.as_ref().map(echidna_ruins::EchidnaRuins::view),
            limp_city: self.limp_city.as_ref().map(LimpCity::view),
            torture_cave: self.torture_cave.as_ref().map(torture_cave::TortureCave::view),
        }
    }
}

/// The black rings a round in `room` starts with (the server's map inits).
pub fn map_black_rings(room: RoomId, cfg: &GameplayConfig) -> usize {
    match room {
        RoomId::Pricelessfreedom => cfg.levels.priceless_freedom.black_rings,
        RoomId::Fartzone => cfg.levels.fart_zone.black_rings,
        _ => 0,
    }
}

impl LevelView {
    /// Sets the instance variables the original's packets set.
    pub fn apply(&self, world: &mut World) {
        if let Some(water_shocks) = self.water_shocks {
            thunder::apply(world, water_shocks);
        }
        if let Some(frame) = self.spike_frame {
            spikes::apply(world, frame);
        }
        gas::apply(world, self.gassed_area);
        if let Some(broken) = self.broken_monitors {
            monitors::apply(world, broken);
        }
        if let Some((open, usable, slid)) = &self.doors {
            doors::apply(world, *open, *usable, slid);
        }
        lifts::apply(world, &self.lifts);
        if let Some(broken) = self.broken_ice {
            ice::apply(world, broken);
        }
        snowballs::apply(world, &self.snowballs);
        lava::apply(world, &self.lava);
        if let Some(broken) = self.broken_vases {
            vases::apply(world, broken);
        }
        if let Some(dark_tower) = &self.dark_tower {
            dark_tower::apply(world, dark_tower);
        }
        if let Some(not_perfect) = self.not_perfect {
            not_perfect::apply(world, not_perfect);
        }
        if let Some(closed) = self.act9_walls {
            act9::apply(world, closed);
        }
        lanterns::apply(world, self.lit_lantern);
        if let Some(dummy) = self.dummy {
            dummy::apply(world, dummy);
        }
        if let Some(ravine_mist) = &self.ravine_mist {
            ravine_mist::apply(world, ravine_mist);
        }
        if let Some(limp_city) = &self.limp_city {
            limp_city::apply(world, limp_city);
        }
        if let Some(torture_cave) = &self.torture_cave {
            torture_cave::apply(world, torture_cave);
        }
        if let Some(echidna_ruins) = self.echidna_ruins {
            echidna_ruins::apply(world, echidna_ruins);
        }
        if let Some(swing) = self.ball_swing {
            balls::apply(world, swing);
        }
    }
}

/// xorshift64: the server entities' random timers, reproducible from a seed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Random(u64);

impl Default for Random {
    fn default() -> Random {
        Random::new(1)
    }
}

impl Random {
    pub fn new(seed: u64) -> Random {
        // Zero would stay zero forever.
        Random(seed.max(1))
    }

    /// A whole number from 0 to `bound` - 1 (0 when `bound` is 0), as rand's gen_range(0..bound).
    pub fn below(&mut self, bound: u32) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        if bound == 0 {
            0
        } else {
            (self.0 % bound as u64) as u32
        }
    }
}
