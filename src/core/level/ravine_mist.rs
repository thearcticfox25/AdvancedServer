//! Ravine Mist's shards and slugs (the server's rmz_init, rmz_shard, rmz_slug): the
//! survivors must find enough shards before the big ring opens, and drop the ones they
//! carry when they die; slugs crawl out of spawners, some carrying a ring.

use super::Random;
use crate::core::resources::names::sprite;
use crate::core::collision::sprite_bbox;
use crate::core::config::{step, ticks, GameplayConfig, RavineMistRules};
use crate::core::events::{SimEvent, EFFECT_SPEED};
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Character, Player};
use crate::core::world::World;
use serde::{Deserialize, Serialize};

/// obj_ravintmist_shard: its sides and bottom are checked this far from its middle.
const SHARD_HALF_WIDTH: f64 = 7.0;
const SHARD_SIDE_BELOW: f64 = 7.0;
const SHARD_FEET: (f64, f64) = (14.0, 21.0);
const SHARD_SETTLE_STEPS: i32 = 7;
/// Enough to leave any wall; the original loops until the shard is out.
const MAX_PUSH: usize = 64;
/// obj_rmzsonic: image_alpha grows by this a step, and the slug does nothing until it is 1.
const SLUG_FADE_IN: f64 = 0.016;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RavineMist {
    shards: Vec<ShardView>,
    speeds: Vec<f64>,
    next_id: u16,
    /// The shards each player carries, by their place among the players.
    carried: Vec<usize>,
    spawners: Vec<Spawner>,
    /// shard_amount and shards_needed of the round's rules.
    amount: usize,
    needed: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Spawner {
    x: f64,
    y: f64,
    /// Ticks before the first slug may come.
    offset: i32,
    timer: i32,
    slug: Option<Slug>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Slug {
    id: u16,
    x: f64,
    going_right: bool,
    ring: SlugRing,
    age: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum SlugRing {
    None = 0,
    Ring = 1,
    RedRing = 2,
}

/// What clients are told of Ravine Mist.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RavineMistView {
    /// SERVER_RMZSHARD_STATE 3: how many shards have been found.
    pub found: u8,
    pub shards: Vec<ShardView>,
    pub slugs: Vec<SlugView>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShardView {
    pub id: u16,
    pub x: f32,
    pub y: f32,
    pub falling: bool,
}

/// A slug as SERVER_RMZSLIME_STATE tells it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlugView {
    pub id: u16,
    pub x: f32,
    pub y: f32,
    pub ring: SlugRing,
    pub facing_right: bool,
    /// Ticks since it crawled out (it fades in).
    pub age: u16,
}

impl SlugView {
    pub fn alpha(&self) -> f64 {
        (self.age as f64 * SLUG_FADE_IN).min(1.0)
    }

    /// A slug that has not faded in yet does nothing.
    pub fn awake(&self) -> bool {
        self.age as f64 * SLUG_FADE_IN >= 1.0
    }

    /// obj_rmzsonic Step: its sprite by the ring it carries.
    pub fn sprite(&self) -> crate::core::resources::SpriteId {
        match self.ring {
            SlugRing::None => sprite::SPR_RAVINEMIST_SONIC,
            SlugRing::Ring => sprite::SPR_RAVINEMIST_SONIC2,
            SlugRing::RedRing => sprite::SPR_RAVINEMIST_SONIC3,
        }
    }

    pub fn xscale(&self) -> f64 {
        if self.facing_right { 1.0 } else { -1.0 }
    }

    pub fn bbox(&self, world: &World) -> crate::core::collision::Bbox {
        sprite_bbox(world.sprites.get(self.sprite()), self.x as f64, self.y as f64, self.xscale(), 1.0, 0.0)
    }
}

impl RavineMist {
    pub(super) fn new(rules: &RavineMistRules, random: &mut Random) -> RavineMist {
        // A shuffle of the spots; the first `shard_amount` get a shard.
        let mut spots = rules.shard_spots.clone();
        for slot in 0..spots.len().saturating_sub(1) {
            let pick = slot + random.below((spots.len() - slot) as u32) as usize;
            spots.swap(slot, pick);
        }
        let mut level = RavineMist { shards: Vec::new(), speeds: Vec::new(), next_id: 0, carried: Vec::new(), spawners: Vec::new(), amount: rules.shard_amount, needed: rules.shards_needed };
        for &[x, y] in spots.iter().take(rules.shard_amount) {
            level.place_shard(x, y, false);
        }
        if rules.slugs {
            level.spawners = rules
                .slug_spawners
                .iter()
                .map(|&[x, y]| Spawner { x, y, offset: ticks(random.below(2) as f64), timer: 0, slug: None })
                .collect();
        }
        level
    }

    fn place_shard(&mut self, x: f64, y: f64, falling: bool) {
        self.next_id = self.next_id.wrapping_add(1);
        self.shards.push(ShardView { id: self.next_id, x: x as f32, y: y as f32, falling });
        self.speeds.push(0.0);
    }

    pub(super) fn tick(&mut self, cfg: &GameplayConfig, world: &World, players: &mut [Player], random: &mut Random, events: &mut Vec<SimEvent>) {
        let rules = &cfg.levels.ravine_mist;
        self.carried.resize(players.len(), 0);
        // rmz_spawnshards: a dead survivor's shards fall where they died.
        for (index, player) in players.iter().enumerate() {
            if !player.is_dead {
                continue;
            }
            for _ in 0..std::mem::take(&mut self.carried[index]) {
                let spread = random.below(2 * rules.shard_drop_spread as u32 + 1) as f64 - rules.shard_drop_spread as f64;
                self.place_shard(player.x + spread, player.y, true);
            }
        }
        for (shard, speed) in self.shards.iter_mut().zip(&mut self.speeds) {
            if shard.falling {
                fall(rules, world, shard, speed);
            }
        }
        // obj_ravintmist_shard Step: a survivor who is up takes it.
        let shard_sprite = world.sprites.get(sprite::SPR_RAVINEMIST_SHARD);
        let mut index = 0;
        while index < self.shards.len() {
            let shard = self.shards[index];
            let shard_box = sprite_bbox(shard_sprite, shard.x as f64, shard.y as f64, 1.0, 1.0, 0.0);
            let taker = players.iter().position(|player| {
                let can_take = !player.removed && player.character != Character::Exe && player.hp > 0 && player.revival_times < 2;
                can_take && sprite_bbox(world.sprites.get(player.sprite_index), player.x, player.y, player.image_xscale, 1.0, 0.0).overlaps(&shard_box)
            });
            match taker {
                Some(taker) => {
                    self.carried[taker] += 1;
                    players[taker].shards += 1;
                    events.push(SimEvent::ShardFound { player: taker });
                    events.push(SimEvent::Effect { sprite: sprite::SPR_SHARD_SPARKLE, x: shard.x as f64, y: shard.y as f64, xscale: 1.0, image_speed: EFFECT_SPEED, yspd: 0.0 });
                    self.shards.remove(index);
                    self.speeds.remove(index);
                }
                None => index += 1,
            }
        }

        let every = ticks(rules.slug_every_seconds);
        for spawner in &mut self.spawners {
            if let Some(slug) = spawner.slug.as_mut() {
                slug.age += 1;
                if slug.going_right {
                    slug.x += rules.slug_speed * step();
                    slug.going_right = slug.x < spawner.x + rules.slug_walk;
                } else {
                    slug.x -= rules.slug_speed * step();
                    slug.going_right = slug.x <= spawner.x - rules.slug_walk;
                }
                continue;
            }
            if spawner.offset > 0 {
                spawner.offset -= 1;
                continue;
            }
            spawner.timer += 1;
            if spawner.timer >= every {
                spawner.timer = 0;
                let roll = random.below(100);
                let ring = if roll < rules.slug_red_ring_chance {
                    SlugRing::RedRing
                } else if roll < rules.slug_red_ring_chance + rules.slug_ring_chance {
                    SlugRing::Ring
                } else {
                    SlugRing::None
                };
                self.next_id = self.next_id.wrapping_add(1);
                spawner.slug = Some(Slug { id: self.next_id, x: spawner.x, going_right: random.below(2) != 0, ring, age: 0 });
            }
        }
    }

    /// CLIENT_RMZSLIME_HIT: the slug is squashed; a survivor's hit takes its ring.
    pub(super) fn hit_slug(&mut self, world: &World, cfg: &GameplayConfig, id: u16, hitter: Option<&mut Player>, events: &mut Vec<SimEvent>) {
        let Some(spawner) = self.spawners.iter_mut().find(|spawner| spawner.slug.as_ref().is_some_and(|slug| slug.id == id)) else { return };
        let Some(slug) = spawner.slug.take() else { return };
        events.push(SimEvent::SlugSquashed { x: slug.x, y: spawner.y, facing_right: slug.going_right });
        let Some(player) = hitter else { return };
        // SERVER_RMZSLIME_RINGBONUS
        let takes = !player.is_dead && player.revival_times < 2 && player.character != Character::Exe;
        if slug.ring == SlugRing::None || !takes {
            return;
        }
        let red = slug.ring == SlugRing::RedRing;
        if !red || player.red_ring_timer <= 0 {
            events.push(SimEvent::Effect { sprite: sprite::SPR_RING_SPARKLE, x: player.x, y: player.y, xscale: 1.0, image_speed: EFFECT_SPEED, yspd: 0.0 });
        }
        // The server's canHeal is whether the hitter has rings, which it has by now.
        player.take_ring(world, cfg, red, true, events);
    }

    /// How many shards are no longer on the map (the ones carried count as found).
    fn found(&self) -> usize {
        self.amount.saturating_sub(self.shards.len().min(self.amount))
    }

    pub fn enough_found(&self) -> bool {
        self.found() >= self.needed
    }

    pub(super) fn view(&self) -> RavineMistView {
        let slugs = self
            .spawners
            .iter()
            .filter_map(|spawner| {
                let slug = spawner.slug.as_ref()?;
                Some(SlugView { id: slug.id, x: slug.x as f32, y: spawner.y as f32, ring: slug.ring, facing_right: slug.going_right, age: slug.age.min(u16::MAX as i32) as u16 })
            })
            .collect();
        RavineMistView { found: self.found() as u8, shards: self.shards.clone(), slugs }
    }
}

/// obj_ravintmist_shard Step of a dropped shard: out of walls, then down to the floor.
fn fall(rules: &RavineMistRules, world: &World, shard: &mut ShardView, speed: &mut f64) {
    let (mut x, mut y) = (shard.x as f64, shard.y as f64);
    let near = world.collision_circle_list(x, y, 64.0, ObjectId::FloorParent);
    let solid = |px: f64, py: f64| world.position_meeting_any(px, py, &near);
    for _ in 0..MAX_PUSH {
        if !solid(x - SHARD_HALF_WIDTH, y + SHARD_SIDE_BELOW) {
            break;
        }
        x += 1.0;
    }
    for _ in 0..MAX_PUSH {
        if !solid(x + SHARD_HALF_WIDTH, y + SHARD_SIDE_BELOW) {
            break;
        }
        x -= 1.0;
    }
    // collision_rectangle with precise masks: any floor pixel in the box.
    let floor_in = |top: f64, bottom: f64| {
        (top as i64..=bottom as i64).any(|py| ((x - SHARD_HALF_WIDTH) as i64..=(x + SHARD_HALF_WIDTH) as i64).any(|px| solid(px as f64, py as f64)))
    };
    if !floor_in(y + SHARD_FEET.0, y + SHARD_FEET.1) {
        if *speed < rules.shard_max_fall_speed {
            *speed += rules.shard_gravity * step();
        }
        y += *speed * step();
    } else {
        for step in 0..SHARD_SETTLE_STEPS {
            if floor_in(y + SHARD_FEET.1 - 1.0 - step as f64, y + SHARD_FEET.1) {
                y -= 1.0;
            }
        }
        shard.falling = false;
    }
    (x, y) = out_of_pit(world, x, y);
    (shard.x, shard.y) = (x as f32, y as f32);
    if y > world.room_height {
        shard.falling = false;
    }
}

/// obj_abyss Step: a shard that falls into a pit is put by the pit's way out
/// (obj_abyss_target), half its height up and to the left of it, and falls on from there.
fn out_of_pit(world: &World, x: f64, y: f64) -> (f64, f64) {
    let shard = world.sprites.get(sprite::SPR_RAVINEMIST_SHARD);
    let shard_box = crate::core::collision::sprite_bbox(shard, x, y, 1.0, 1.0, 0.0);
    let pit = world.ids_of(ObjectId::Abyss).find(|&id| world.bbox(id).is_some_and(|pit| pit.overlaps(&shard_box)));
    let Some(target) = pit.and_then(|pit| world.instance_nearest(world.instances[pit].x, world.instances[pit].y, ObjectId::AbyssTarget)) else {
        return (x, y);
    };
    let half_height = f64::from(shard.height) / 2.0;
    (world.instances[target].x - half_height, world.instances[target].y - half_height)
}

pub(super) fn apply(world: &mut World, view: &RavineMistView) {
    world.ravine_mist = Some(view.clone());
}

#[cfg(test)]
mod pit_tests {
    use super::*;

    #[test]
    fn a_shard_in_a_pit_is_put_by_the_way_out() {
        let world = crate::core::tests::common::started_room(crate::core::rooms::ids::RoomId::Ravinemist);
        let pit = world.ids_of(ObjectId::Abyss).next().expect("Ravine Mist has pits");
        let pit_box = world.bbox(pit).unwrap();
        let (x, y) = ((pit_box.left + pit_box.right) / 2.0, (pit_box.top + pit_box.bottom) / 2.0);
        let target = world.instance_nearest(world.instances[pit].x, world.instances[pit].y, ObjectId::AbyssTarget).unwrap();
        let half = f64::from(world.sprites.get(sprite::SPR_RAVINEMIST_SHARD).height) / 2.0;
        assert_eq!(out_of_pit(&world, x, y), (world.instances[target].x - half, world.instances[target].y - half));
        assert_eq!(out_of_pit(&world, 10.0, 10.0), (10.0, 10.0), "away from every pit nothing moves");
    }
}
