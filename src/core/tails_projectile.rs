//! Tails' charged shot: flight from the server entity (entities/tails_projectile.rs
//! of AdvancedServer), breaking and hits from obj_tails_projectile.

use crate::core::resources::names::{sound, sprite};
use crate::core::resources::SpriteId;
use crate::core::collision::sprite_bbox;
use crate::core::config::{self, GameplayConfig};
use crate::core::events::SimEvent;
use crate::core::objects::ids::ObjectId;
use crate::core::player::hurt::Hit;
use crate::core::player::{gm_sign, Character, Player};
use crate::core::rooms::ids::RoomId;
use crate::core::world::World;

#[derive(Clone, Debug)]
pub struct TailsProjectile {
    /// Index of the Tails player who fired it.
    pub owner: usize,
    pub x: f64,
    pub y: f64,
    pub dir: f64,
    pub damage: i32,
    /// Shot by demonized Tails: hurts survivors instead of stunning EXE.
    pub hurts_survivors: bool,
    pub charge: i32,
    pub sprite_index: SpriteId,
    pub image_index: f64,
    pub image_speed: f64,
    pub is_breaking: bool,
    ticks_left: i32,
}

pub enum Outcome {
    Alive,
    Gone,
}

impl TailsProjectile {
    #[allow(clippy::too_many_arguments)]
    pub fn new(cfg: &GameplayConfig, owner: usize, x: f64, y: f64, dir: f64, damage: i32, hurts_survivors: bool, charge: i32) -> TailsProjectile {
        TailsProjectile {
            owner,
            x,
            y,
            dir,
            damage,
            hurts_survivors,
            charge,
            sprite_index: sprite::SPR_TAILS_RAY,
            image_index: charge as f64,
            image_speed: cfg.tails.projectile.image_speed,
            is_breaking: false,
            ticks_left: config::ticks(cfg.tails.projectile.lifetime_seconds),
        }
    }

    pub fn tick(&mut self, world: &World, players: &mut [Player], events: &mut Vec<SimEvent>) -> Outcome {
        self.advance_animation(world);
        if self.is_breaking {
            let last_frame = world.sprites.get(self.sprite_index).frame_count as f64 - 1.0;
            return if self.image_index >= last_frame { Outcome::Gone } else { Outcome::Alive };
        }
        let cfg = world.config.clone();
        if !self.fly(world, &cfg) {
            return Outcome::Gone;
        }
        for (index, player) in players.iter_mut().enumerate() {
            if self.try_hit(world, &cfg, player, index, events) {
                self.start_breaking(&cfg);
                break;
            }
        }
        if !self.is_breaking && self.touches_solid(world) {
            self.start_breaking(&cfg);
        }
        Outcome::Alive
    }

    fn advance_animation(&mut self, world: &World) {
        let meta = world.sprites.get(self.sprite_index);
        let frame_count = meta.frame_count as f64;
        self.image_index += self.image_speed * meta.fps as f64 / 60.0;
        if self.image_index >= frame_count {
            self.image_index -= frame_count;
        }
    }

    /// Returns false when the shot leaves the map or times out.
    fn fly(&mut self, world: &World, cfg: &GameplayConfig) -> bool {
        if self.ticks_left <= 0 {
            return false;
        }
        // Majin Forest has no bounds: the shot wraps around at the room width.
        if world.room == Some(RoomId::Majongforest) {
            if self.x < 0.0 {
                self.x = world.room_width;
            } else if self.x > world.room_width {
                self.x = 0.0;
            }
        } else if self.x <= 0.0 {
            return false;
        }
        self.x += self.dir * cfg.tails.projectile.speed_per_tick * config::step();
        self.ticks_left -= 1;
        true
    }

    /// Collision event with obj_solid_parent.
    // ponytail: bounding boxes only, precise block masks ignored for the shot
    fn touches_solid(&self, world: &World) -> bool {
        let shot = self.bbox(world);
        world.ids_of(ObjectId::SolidParent).any(|id| world.bbox(id).is_some_and(|solid| solid.overlaps(&shot)))
    }

    pub fn bbox(&self, world: &World) -> crate::core::collision::Bbox {
        sprite_bbox(world.sprites.get(self.sprite_index), self.x, self.y, self.dir, 1.0, 0.0)
    }

    pub fn start_breaking(&mut self, cfg: &GameplayConfig) {
        self.is_breaking = true;
        self.sprite_index = match self.charge {
            0 => sprite::SPR_TAIL_RAY_DEAD,
            1 => sprite::SPR_TAIL_RAY_DEAD1,
            2 => sprite::SPR_TAIL_RAY_DEAD2,
            3 => sprite::SPR_TAIL_RAY_DEAD3,
            _ => sprite::SPR_TAIL_RAY_DEAD4,
        };
        self.image_index = 0.0;
        self.image_speed = cfg.tails.projectile.image_speed;
    }

    /// obj_tails_projectile Step_0, applied to every player instead of only the local one.
    /// The first player hit (in player order) breaks the shot.
    // Stats: SimEvent::PlayerHit, counted by the server (states/round.rs apply_round_rules).
    fn try_hit(&self, world: &World, cfg: &GameplayConfig, player: &mut Player, index: usize, events: &mut Vec<SimEvent>) -> bool {
        if player.is_dead || !self.touches_player(world, player) {
            return false;
        }
        let projectile = &cfg.tails.projectile;
        let push_x = -gm_sign(self.x - player.x) * projectile.push_x;
        let is_survivor = player.character != Character::Exe && !player.is_demonized();

        if self.hurts_survivors && is_survivor {
            if player.hurttime > 0 {
                return false;
            }
            let hit = Hit { xpw: push_x, ypw: projectile.survivor_hit_push_y, ..Hit::damage(cfg, self.damage) };
            let hp_before = player.hp;
            player.hurt(world, cfg, hit, events);
            events.push(SimEvent::PlayerHit { victim: index, attacker: self.owner, damage: hp_before - player.hp, stun_seconds: 0.0 });
            events.push(SimEvent::ShotHit { shooter: self.owner, damage: self.damage, on_exe: false });
            if player.hp <= 0 {
                events.push(SimEvent::MercoinBonus { player: self.owner, bonus: crate::core::contact::BONUS_DEMON_KILL });
            }
            return true;
        }
        if !self.hurts_survivors && !is_survivor {
            if player.character == Character::Exe && player.invis_timer > 0 {
                return false;
            }
            if player.hurttime > 0 {
                return false;
            }
            if player.character == Character::Sally {
                if player.shield_timer > 0 {
                    player.is_hurt = true;
                    player.hurttime = config::ticks(cfg.sally.shield_block_invincibility_seconds);
                    player.shield_timer = 0;
                    player.show_shield_break(events);
                    return true;
                }
                player.is_sliding = false;
            }
            // The damage of a survivor's shot is the stun in whole seconds.
            player.shocked_timer = config::ticks(self.damage as f64);
            events.push(SimEvent::PlayerHit { victim: index, attacker: self.owner, damage: 0, stun_seconds: self.damage as f64 });
            player.xspd = push_x;
            player.yspd = projectile.exe_hit_push_y;
            player.is_grounded = false;
            events.push(SimEvent::Sound { sound: sound::SND_EXE_STUN, x: player.x, y: player.y });
            if player.character == Character::Exe {
                events.push(SimEvent::ShotHit { shooter: self.owner, damage: self.damage, on_exe: true });
            }
            return true;
        }
        false
    }

    fn touches_player(&self, world: &World, player: &Player) -> bool {
        let body = sprite_bbox(world.sprites.get(player.sprite_index), player.x, player.y, player.image_xscale, 1.0, 0.0);
        self.bbox(world).overlaps(&body)
    }
}
