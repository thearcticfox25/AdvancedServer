//! Rings the round rules put on the map (obj_ring, obj_redring): each fades in,
//! then the first survivor touching it takes it, and EXE or a demon attacking
//! into a normal ring breaks it. Taking one is SERVER_RING_COLLECTED of the
//! taker's client in the original.

use crate::core::resources::names::{sound, sprite};
use crate::core::collision::sprite_bbox;
use crate::core::config::GameplayConfig;
use crate::core::events::{SimEvent, EFFECT_SPEED};
use crate::core::player::{Character, Player};
use crate::core::world::World;

pub struct MapRing {
    pub x: f64,
    pub y: f64,
    pub red: bool,
    /// image_alpha: the ring can be touched once this reaches 1.
    pub alpha: f64,
    /// The map's ring spawner this one stands on, free again when it goes; Cream's
    /// rings stand where she laid them and have none.
    pub spawner: Option<u8>,
}

/// What happened to a ring this tick, and to whom (an index into the players).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RingTouch {
    Taken(usize),
    Broken(usize),
}

impl MapRing {
    pub fn new(x: f64, y: f64, red: bool) -> MapRing {
        MapRing { x, y, red, alpha: 0.0, spawner: None }
    }

    /// A ring on the map's ring spawner number `spawner`.
    pub fn on_spawner(x: f64, y: f64, red: bool, spawner: u8) -> MapRing {
        MapRing { x, y, red, alpha: 0.0, spawner: Some(spawner) }
    }

    /// Step of obj_ring / obj_redring, checked against every player in turn as each
    /// client checked its own player.
    pub fn tick(&mut self, world: &World, cfg: &GameplayConfig, players: &[Player]) -> Option<RingTouch> {
        if self.alpha < 1.0 {
            self.alpha += if self.red { cfg.rings.red_fade_in_per_tick } else { cfg.rings.fade_in_per_tick } * crate::core::config::step();
        }
        if self.alpha < 1.0 {
            return None;
        }
        let ring_sprite = if self.red { sprite::SPR_REDRING } else { sprite::SPR_RING };
        let ring = sprite_bbox(world.sprites.get(ring_sprite), self.x, self.y, 1.0, 1.0, 0.0);
        players.iter().enumerate().filter(|(_, player)| !player.removed).find_map(|(index, player)| {
            let body = sprite_bbox(world.sprites.get(player.sprite_index), player.x, player.y, player.image_xscale, 1.0, 0.0);
            if !ring.overlaps(&body) {
                return None;
            }
            let demon = player.character == Character::Exe || player.revival_times >= 2;
            if self.red {
                let can_take = player.character != Character::Exe && player.red_ring_timer <= 0 && player.hp > 0 && player.revival_times < 2;
                return can_take.then_some(RingTouch::Taken(index));
            }
            if demon {
                let breaks = !matches!(player.character, Character::Tails | Character::Cream) && player.is_attacking;
                return breaks.then_some(RingTouch::Broken(index));
            }
            (player.hp > 0).then_some(RingTouch::Taken(index))
        })
    }
}

impl MapRing {
    /// obj_ring Step: a shot of demonized Tails breaks an ordinary ring it touches, for
    /// the one who fired it. Red rings have a Step of their own without this.
    pub fn broken_by_shot(&self, world: &World, shots: &[crate::core::tails_projectile::TailsProjectile]) -> Option<usize> {
        if self.red || self.alpha < 1.0 {
            return None;
        }
        let ring = sprite_bbox(world.sprites.get(sprite::SPR_RING), self.x, self.y, 1.0, 1.0, 0.0);
        shots.iter().find(|shot| shot.hurts_survivors && shot.bbox(world).overlaps(&ring)).map(|shot| shot.owner)
    }
}

impl Player {
    /// SERVER_RING_COLLECTED: a ring (or a red ring) reached this player.
    /// `can_heal`: the server allows healing with rings.
    pub fn take_ring(&mut self, world: &World, cfg: &GameplayConfig, red: bool, can_heal: bool, events: &mut Vec<SimEvent>) {
        if self.character == Character::Exe || self.revival_times >= 2 {
            return;
        }
        let rules = &cfg.rings;
        if red {
            if self.red_ring_timer > 0 {
                return;
            }
            events.push(SimEvent::Sound { sound: sound::SND_REDRING, x: self.x, y: self.y });
            events.push(SimEvent::RedRingStarted);
            self.red_ring_timer = crate::core::config::original_ticks(rules.red_ring_ticks);
            return;
        }
        self.rings += 1;
        self.shorten_red_ring(world, cfg);
        if self.rings >= rules.rings_to_heal && self.hp < cfg.hurt.max_hp && can_heal {
            self.rings = 0;
            self.hp += cfg.hurt.heal_hp;
            events.push(SimEvent::Sound { sound: sound::SND_HEAL, x: self.x, y: self.y });
            events.push(SimEvent::RingsHealed);
        } else {
            events.push(SimEvent::Sound { sound: sound::SND_RING, x: self.x, y: self.y });
        }
    }

    /// SERVER_VVVASE_STATE: the rings of a vase this player broke. Unlike a ring, healing
    /// spends only the rings it needs and the ring sound plays too.
    pub fn take_vase_rings(&mut self, world: &World, cfg: &GameplayConfig, rings: i32, events: &mut Vec<SimEvent>) {
        if self.is_dead {
            return;
        }
        events.push(SimEvent::Sound { sound: sound::SND_RING, x: self.x, y: self.y });
        events.push(SimEvent::Effect { sprite: sprite::SPR_RING_SPARKLE, x: self.x, y: self.y, xscale: 1.0, image_speed: EFFECT_SPEED, yspd: 0.0 });
        self.rings += rings;
        self.shorten_red_ring(world, cfg);
        // The server's canHeal is whether the breaker has rings, which it always has by now.
        if self.rings >= cfg.rings.rings_to_heal && self.hp < cfg.hurt.max_hp {
            self.rings -= cfg.rings.rings_to_heal;
            self.hp += cfg.hurt.heal_hp;
            events.push(SimEvent::Sound { sound: sound::SND_HEAL, x: self.x, y: self.y });
            events.push(SimEvent::RingsHealed);
        }
    }

    /// Every ring taken under a red ring shortens it.
    fn shorten_red_ring(&mut self, world: &World, cfg: &GameplayConfig) {
        let rules = &cfg.rings;
        let min_ticks = crate::core::config::original_ticks(rules.red_ring_min_ticks);
        if self.red_ring_timer >= min_ticks {
            let last_minute = world.timer_ticks < crate::core::config::ticks(cfg.hurt.last_minute_seconds) as i64;
            let shortening = if last_minute { rules.last_minute_red_ring_shortening_ticks } else { rules.red_ring_shortening_ticks };
            // The shortening is in 60 Hz steps too.
            let shortening = shortening / crate::core::config::step();
            self.red_ring_timer = ((self.red_ring_timer as f64 - shortening) as i32).max(min_ticks);
        }
    }

    /// ringBreak of obj_ring: the ring shatters under an attack.
    pub fn break_ring(&self, ring: &MapRing, events: &mut Vec<SimEvent>) {
        events.push(SimEvent::Sound { sound: sound::SND_EXE_RINGSHUTTER, x: self.x, y: self.y });
        events.push(SimEvent::RingBroke { x: ring.x, y: ring.y, xspd: self.xspd });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::resources::sprites::Sprites;
    use std::path::Path;
    use std::sync::Arc;

    fn world() -> World {
        let textures = Path::new(env!("CARGO_MANIFEST_DIR")).join("Resources/Textures");
        World::empty(Arc::new(Sprites::index(&textures).unwrap()), Arc::new(GameplayConfig::default()))
    }

    #[test]
    fn a_ring_fades_in_then_goes_to_the_survivor_and_breaks_under_an_attack() {
        let (world, cfg) = (world(), GameplayConfig::default());
        let tails = Player::new(Character::Tails, 100.0, 100.0, &cfg);
        let mut ring = MapRing::new(100.0, 100.0, false);
        let touches: Vec<Option<RingTouch>> = (0..31).map(|_| ring.tick(&world, &cfg, std::slice::from_ref(&tails))).collect();
        assert!(touches[..30].iter().all(Option::is_none), "still fading in for 30 ticks (the float sum stays below 1)");
        assert_eq!(touches[30], Some(RingTouch::Taken(0)));

        let mut exe = Player::new_exe(crate::core::player::ExeCharacter::Original, 100.0, 100.0, &cfg);
        assert_eq!(ring.tick(&world, &cfg, std::slice::from_ref(&exe)), None, "EXE does not take rings");
        exe.is_attacking = true;
        assert_eq!(ring.tick(&world, &cfg, &[exe]), Some(RingTouch::Broken(0)));
    }

    #[test]
    fn a_shot_of_demonized_tails_breaks_an_ordinary_ring_only() {
        let (world, cfg) = (world(), GameplayConfig::default());
        let mut ring = MapRing::new(100.0, 100.0, false);
        ring.alpha = 1.0;
        let demon_shot = crate::core::tails_projectile::TailsProjectile::new(&cfg, 3, 100.0, 100.0, 1.0, 1, true, 0);
        let survivor_shot = crate::core::tails_projectile::TailsProjectile::new(&cfg, 4, 100.0, 100.0, 1.0, 1, false, 0);
        assert_eq!(ring.broken_by_shot(&world, std::slice::from_ref(&survivor_shot)), None, "a survivor's shot leaves rings alone");
        assert_eq!(ring.broken_by_shot(&world, &[survivor_shot, demon_shot.clone()]), Some(3), "broken for the shooter");
        let mut red = MapRing::new(100.0, 100.0, true);
        red.alpha = 1.0;
        assert_eq!(red.broken_by_shot(&world, &[demon_shot]), None, "red rings have no such Step");
    }

    #[test]
    fn ten_rings_heal_a_hurt_survivor() {
        let (mut world, cfg) = (world(), GameplayConfig::default());
        world.timer_ticks = 10_000;
        let mut tails = Player::new(Character::Tails, 0.0, 0.0, &cfg);
        tails.hp = cfg.hurt.max_hp - cfg.hurt.heal_hp;
        tails.rings = 9;
        let mut events = Vec::new();
        tails.take_ring(&world, &cfg, false, true, &mut events);
        assert_eq!((tails.rings, tails.hp), (0, cfg.hurt.max_hp));
        assert!(events.contains(&SimEvent::RingsHealed));

        tails.red_ring_timer = 100;
        tails.take_ring(&world, &cfg, false, true, &mut Vec::new());
        assert_eq!(tails.red_ring_timer, 70, "a ring shortens the red ring by half a second");
    }
}
