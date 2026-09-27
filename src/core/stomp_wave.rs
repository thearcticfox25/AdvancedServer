//! Exetior's stomp shockwave (obj_exetior_stompballs): runs along the floor away
//! from the landing spot, hurting survivors it touches.

use crate::core::resources::names::sprite;
use crate::core::collision::sprite_bbox;
use crate::core::config::{self, GameplayConfig};
use crate::core::events::SimEvent;
use crate::core::objects::ids::ObjectId;
use crate::core::player::hurt::Hit;
use crate::core::player::{Character, Player};
use crate::core::world::World;

/// CLIENT_MERCOIN_BONUS 7: a survivor hit by a stomp wave.
const SHOCKWAVE_BONUS: u8 = 7;

#[derive(Clone, Debug)]
pub struct StompWave {
    pub x: f64,
    pub y: f64,
    /// -1 runs left, 1 runs right.
    pub dir: f64,
    /// How many waves came before this one on its side (the original stops after 1).
    pub generation: i32,
    /// The Exetior whose stomp made it (index into Game::players).
    pub owner: usize,
    pub image_index: f64,
    pub visible: bool,
    ticks_alive: i32,
}

pub enum WaveOutcome {
    Alive,
    /// The wave finished its animation and spawns the next one further out.
    AliveAndSpawns(StompWave),
    Gone,
}

impl StompWave {
    /// The two waves CLIENT_ERECTOR_BALLS creates beside the landing spot.
    pub fn pair(cfg: &GameplayConfig, x: f64, y: f64, owner: usize) -> [StompWave; 2] {
        let waves = &cfg.exetior.stomp_waves;
        let wave = |dir: f64| StompWave {
            x: x + dir * waves.first_offset_x,
            y: y + waves.offset_y,
            dir,
            generation: 0,
            owner,
            image_index: 0.0,
            visible: true,
            ticks_alive: 0,
        };
        [wave(-1.0), wave(1.0)]
    }

    pub fn tick(&mut self, world: &World, players: &mut [Player], events: &mut Vec<SimEvent>) -> WaveOutcome {
        let cfg = world.config.clone();
        let waves = &cfg.exetior.stomp_waves;
        let meta = world.sprites.get(sprite::SPR_EXETIOR_STOMPBALLS);
        let frame_count = meta.frame_count as f64;
        self.image_index = (self.image_index + meta.fps as f64 / 60.0) % frame_count;
        self.ticks_alive += 1;

        // The original removes a wave when its shockwave sound stops playing.
        if self.ticks_alive >= config::ticks(waves.lifetime_seconds) {
            return WaveOutcome::Gone;
        }
        if !self.on_floor(world) || self.generation > waves.max_generation {
            return WaveOutcome::Gone;
        }
        if self.image_index >= frame_count - 1.0 {
            let mut outcome = WaveOutcome::Alive;
            if self.generation <= waves.max_generation {
                self.generation += 1;
                let next = StompWave {
                    x: self.x + self.dir * waves.step_x,
                    generation: self.generation,
                    image_index: 0.0,
                    visible: true,
                    ticks_alive: 0,
                    ..*self
                };
                outcome = WaveOutcome::AliveAndSpawns(next);
            }
            self.visible = false;
            return outcome;
        }
        if !self.visible {
            return WaveOutcome::Alive;
        }
        for (index, player) in players.iter_mut().enumerate() {
            self.try_hurt(world, &cfg, player, index, events);
        }
        WaveOutcome::Alive
    }

    // ponytail: bounding boxes only, precise floor masks (slopes) ignored for the wave
    fn on_floor(&self, world: &World) -> bool {
        let wave = self.bbox(world);
        world.ids_of(ObjectId::FloorParent).any(|id| world.bbox(id).is_some_and(|floor| floor.overlaps(&wave)))
    }

    pub fn bbox(&self, world: &World) -> crate::core::collision::Bbox {
        // image_xscale = -dir: the wave sprite faces the way it runs.
        sprite_bbox(world.sprites.get(sprite::SPR_EXETIOR_STOMPBALLS), self.x, self.y, -self.dir, 1.0, 0.0)
    }

    /// Every client checked its own survivor, so one wave can hurt several of them.
    /// Each hit counts for Exetior (CLIENT_MERCOIN_BONUS 7, CLIENT_STATS_REPORT 2).
    fn try_hurt(&self, world: &World, cfg: &GameplayConfig, player: &mut Player, index: usize, events: &mut Vec<SimEvent>) {
        let is_survivor = player.character != Character::Exe && !player.is_demonized();
        if !is_survivor || player.hp <= 0 || player.hurttime > 0 {
            return;
        }
        let body = sprite_bbox(world.sprites.get(player.sprite_index), player.x, player.y, player.image_xscale, 1.0, 0.0);
        if !self.bbox(world).overlaps(&body) {
            return;
        }
        let waves = &cfg.exetior.stomp_waves;
        let hit = Hit { xpw: self.dir * waves.push_x, ..Hit::damage(cfg, waves.damage) };
        let hp_before = player.hp;
        player.hurt(world, cfg, hit, events);
        if player.hp < hp_before {
            events.push(SimEvent::MercoinBonus { player: self.owner, bonus: SHOCKWAVE_BONUS });
            events.push(SimEvent::PlayerHit { victim: index, attacker: self.owner, damage: hp_before - player.hp, stun_seconds: 0.0 });
        }
        if player.hp <= 0 {
            events.push(SimEvent::KilledByExe { victim: index, killer: self.owner });
        }
    }
}
