//! The shards of a demonized Sally's shield (obj_quickeffect with spr_shieldbreak2):
//! while the break plays, a survivor who touches it is hurt (scr_collision_objects).
//! Sally, EXE and demonized players are not.

use crate::core::config::{ticks_per_second, GameplayConfig};
use crate::core::events::SimEvent;
use crate::core::player::hurt::Hit;
use crate::core::player::{Character, Player};
use crate::core::resources::names::sprite;
use crate::core::world::World;

/// CLIENT_MERCOIN_BONUS 5: a survivor hurt by a demonized Sally's shield shards.
pub const SHIELD_SHARDS_BONUS: u8 = 5;
/// scr_player_hurt(20, -image_xscale * 3): pushed back the way the victim faces.
const PUSH_X: f64 = 3.0;

#[derive(Clone, Debug)]
pub struct ShieldShards {
    /// The Sally whose shield broke, who the hits are counted for.
    pub owner: usize,
    pub x: f64,
    pub y: f64,
    /// Until the break's last frame: obj_quickeffect destroys itself after it.
    ticks_left: i32,
}

impl ShieldShards {
    /// The break of `owner`'s shield at (x, y), lasting as long as its animation.
    pub fn new(world: &World, owner: usize, x: f64, y: f64) -> ShieldShards {
        let meta = world.sprites.get(sprite::SPR_SHIELDBREAK2);
        let seconds = meta.frame_count as f64 / f64::from(meta.fps.max(1));
        ShieldShards { owner, x, y, ticks_left: (seconds * ticks_per_second()).ceil() as i32 }
    }

    /// One step: hurts the survivors it touches. False once the break is over.
    pub fn tick(&mut self, world: &World, cfg: &GameplayConfig, players: &mut [Player], events: &mut Vec<SimEvent>) -> bool {
        let shards = crate::core::collision::sprite_bbox(world.sprites.get(sprite::SPR_SHIELDBREAK2), self.x, self.y, 1.0, 1.0, 0.0);
        for (index, player) in players.iter_mut().enumerate() {
            let hurtable = !matches!(player.character, Character::Exe | Character::Sally) && !player.is_demonized() && player.hurttime <= 0 && player.hp > 0;
            let body = crate::core::collision::sprite_bbox(world.sprites.get(player.sprite_index), player.x, player.y, player.image_xscale, 1.0, 0.0);
            if player.removed || !hurtable || !body.overlaps(&shards) {
                continue;
            }
            let hp_before = player.hp;
            let hit = Hit { xpw: -player.image_xscale * PUSH_X, ..Hit::damage(cfg, cfg.contact.damage) };
            player.hurt(world, cfg, hit, events);
            if player.hp != hp_before {
                // CLIENT_MERCOIN_BONUS 5: the hit counts for the Sally whose shield it was.
                events.push(SimEvent::MercoinBonus { player: self.owner, bonus: SHIELD_SHARDS_BONUS });
                events.push(SimEvent::BloodScreen { victim: index });
                events.push(SimEvent::PlayerHit { victim: index, attacker: self.owner, damage: hp_before - player.hp, stun_seconds: 0.0 });
            }
        }
        self.ticks_left -= 1;
        self.ticks_left > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::resources::sprites::Sprites;
    use std::path::Path;
    use std::sync::Arc;

    #[test]
    fn shards_hurt_a_survivor_but_not_sally_and_then_are_gone() {
        let textures = Path::new(env!("CARGO_MANIFEST_DIR")).join("Resources/Textures");
        let world = World::empty(Arc::new(Sprites::index(&textures).unwrap()), Arc::new(GameplayConfig::default()));
        let cfg = GameplayConfig::default();
        let mut players = [Player::new(Character::Sally, 100.0, 100.0, &cfg), Player::new(Character::Tails, 100.0, 100.0, &cfg)];
        let mut shards = ShieldShards::new(&world, 0, 100.0, 100.0);
        let mut events = Vec::new();
        assert!(shards.tick(&world, &cfg, &mut players, &mut events));
        assert_eq!(players[0].hp, cfg.hurt.max_hp, "Sally's own shards leave her alone");
        assert_eq!(players[1].hp, cfg.hurt.max_hp - cfg.contact.damage, "Tails is hurt");
        assert!(events.contains(&SimEvent::PlayerHit { victim: 1, attacker: 0, damage: cfg.contact.damage, stun_seconds: 0.0 }));
        let mut ticks = 1;
        while shards.tick(&world, &cfg, &mut players, &mut Vec::new()) {
            ticks += 1;
            assert!(ticks < 1000, "the break ends");
        }
    }
}
