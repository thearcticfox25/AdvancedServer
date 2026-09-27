//! Torture Cave's acid clouds.

use super::common::{hold_each, started_room};
use crate::core::config::{ticks, GameplayConfig};
use crate::core::game::Game;
use crate::core::level::Level;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Character, Player};
use crate::core::rooms::ids::RoomId;
use crate::core::world::ObjectVars;

#[test]
fn rising_acid_burns_rings_then_health() {
    let mut game = Game::new(started_room(RoomId::Torturecave));
    game.level = Some(Level::new(&game.world, 2));
    let rules = game.world.config.levels.torture_cave.clone();
    hold_each(&mut game, &[], ticks(rules.acid_seconds) as usize + 60);
    let cloud = game.world.ids_of(ObjectId::AbadonCloud).find(|&id| matches!(game.world.instances[id].vars, ObjectVars::AcidCloud { burning: true, .. })).expect("a burning cloud");
    assert!(game.world.instances[cloud].visible && game.world.instances[cloud].image_index >= rules.acid_harmful_frame);
    let bbox = game.world.bbox(cloud).unwrap();
    let mut eggman = Player::new(Character::Eggman, (bbox.left + bbox.right) / 2.0, bbox.bottom - 30.0, &GameplayConfig::default());
    eggman.rings = 1;
    game.players.push(eggman);
    for _ in 0..40 {
        let (x, y) = ((bbox.left + bbox.right) / 2.0, bbox.bottom - 30.0);
        (game.players[0].x, game.players[0].y, game.players[0].yspd) = (x, y, 0.0);
        hold_each(&mut game, &[0], 1);
    }
    assert_eq!(game.players[0].rings, 0, "a ring first");
    assert!(game.players[0].hp < 100, "then health");
}
