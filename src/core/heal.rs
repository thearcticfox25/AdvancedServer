//! Healing a teammate (scr_survivor_heal): a survivor with enough rings looks up
//! while touching a hurt teammate; a gauge fills, sparkles rise, and when it is full
//! the teammate gets health for the rings.

use crate::core::config::GameplayConfig;
use crate::core::contact::bodies_touch;
use crate::core::player::{Character, Player};
use crate::core::world::World;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HealStep {
    /// obj_heal_progress visible: the healer is shown the gauge over the teammate.
    pub gauge_shown: bool,
    /// Sparkles this tick (CLIENT_PLAYER_HEAL_PART).
    pub sparkles: bool,
    /// The gauge filled: the teammate is healed (CLIENT_PLAYER_HEAL).
    pub healed: bool,
}

/// One tick of `healer` over `target`, with the gauge `progress` the healer keeps for this teammate.
pub fn heal_step(world: &World, cfg: &GameplayConfig, progress: &mut f64, healer: &Player, target: &Player) -> HealStep {
    let rules = &cfg.rings;
    if target.hp <= 0 || target.hp >= cfg.hurt.max_hp || target.is_demonized() || healer.rings < rules.teammate_heal_rings {
        *progress = 0.0;
        return HealStep::default();
    }
    if healer.character == Character::Exe || healer.is_demonized() || healer.hp <= 0 {
        return HealStep::default();
    }
    if !bodies_touch(world, target, healer) {
        *progress = 0.0;
        return HealStep::default();
    }
    if !(healer.controls_enabled && healer.is_looking_up) {
        *progress = 0.0;
        return HealStep { gauge_shown: true, ..HealStep::default() };
    }
    let sparkles = (*progress * 100.0).floor() as i64 % rules.teammate_heal_sparkle_every_percent == 0;
    *progress += rules.teammate_heal_per_tick * crate::core::config::step();
    let healed = *progress >= 1.0;
    if healed {
        *progress = 0.0;
    }
    HealStep { gauge_shown: true, sparkles, healed }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::resources::sprites::Sprites;
    use std::path::Path;
    use std::sync::Arc;

    #[test]
    fn looking_up_at_a_hurt_teammate_fills_the_gauge_and_heals() {
        let textures = Path::new(env!("CARGO_MANIFEST_DIR")).join("Resources/Textures");
        let world = World::empty(Arc::new(Sprites::index(&textures).unwrap()), Arc::new(GameplayConfig::default()));
        let cfg = GameplayConfig::default();
        let mut healer = Player::new(Character::Cream, 100.0, 100.0, &cfg);
        healer.rings = 10;
        healer.is_looking_up = true;
        let mut target = Player::new(Character::Tails, 104.0, 100.0, &cfg);
        target.hp = cfg.hurt.max_hp - cfg.hurt.heal_hp;

        let mut progress = 0.0;
        let steps: Vec<HealStep> = (0..63).map(|_| heal_step(&world, &cfg, &mut progress, &healer, &target)).collect();
        assert!(steps[0].sparkles && steps.iter().all(|step| step.gauge_shown));
        assert_eq!(steps.iter().position(|step| step.healed), Some(62), "0.016 a tick passes 1 on the 63rd tick");

        healer.is_looking_up = false;
        progress = 0.5;
        assert!(!heal_step(&world, &cfg, &mut progress, &healer, &target).healed);
        assert_eq!(progress, 0.0, "looking away empties the gauge");
    }
}
