//! Behavior checks of Exeller and its clones in a real room.

use super::common::{greenhill_with_exe, hold, hold_each};
use crate::core::config::GameplayConfig;
use crate::core::events::SimEvent;
use crate::core::player::{Buttons, Character, ExeCharacter, Player, NO_CLONE};

#[test]
fn clones_are_placed_with_a_pause_and_at_most_two() {
    let mut game = greenhill_with_exe(ExeCharacter::Exeller);
    hold(&mut game, 0, 120);

    let events = hold(&mut game, Buttons::C, 1);
    assert!(events.iter().any(|event| matches!(event, SimEvent::SpawnExellerClone { .. })));
    assert_eq!(game.exeller_clones.len(), 1);
    assert_eq!(game.players[0].clones[0], game.exeller_clones[0].id, "first clone takes the up slot");

    hold(&mut game, 0, 1);
    hold(&mut game, Buttons::C, 1);
    assert_eq!(game.exeller_clones.len(), 1, "no second clone within 3 seconds");

    hold(&mut game, 0, 180);
    hold(&mut game, Buttons::C, 1);
    assert_eq!(game.exeller_clones.len(), 2, "second clone after the pause");
    assert_ne!(game.players[0].clones[1], NO_CLONE, "second clone takes the down slot");

    hold(&mut game, 0, 180);
    hold(&mut game, Buttons::C, 1);
    assert_eq!(game.exeller_clones.len(), 2, "never more than two clones");
}

#[test]
fn up_and_c_teleports_to_the_first_clone() {
    let mut game = greenhill_with_exe(ExeCharacter::Exeller);
    hold(&mut game, 0, 120);
    hold(&mut game, Buttons::C, 1);
    let (clone_x, clone_y) = (game.exeller_clones[0].x, game.exeller_clones[0].y);

    hold(&mut game, Buttons::RIGHT, 90);
    assert!(game.players[0].x > clone_x + 50.0, "ran away from the clone");
    hold(&mut game, 0, 1);
    hold(&mut game, Buttons::UP | Buttons::C, 1);
    let exeller = &game.players[0];
    assert!((exeller.x - clone_x).abs() < 16.0 && (exeller.y - clone_y).abs() < 16.0, "back at the clone: {exeller:?}");
    assert_eq!(exeller.clones[0], NO_CLONE);
    assert_eq!(exeller.clone_count, 0);
    assert!(exeller.clone_timer > 29 * 60, "30 second recharge after a teleport");
    assert!(game.exeller_clones.is_empty(), "the clone is used up");
}

#[test]
fn clone_reveals_a_nearby_survivor_only() {
    let mut game = greenhill_with_exe(ExeCharacter::Exeller);
    hold(&mut game, 0, 120);
    hold(&mut game, Buttons::C, 1);
    let (clone_x, clone_y) = (game.exeller_clones[0].x, game.exeller_clones[0].y);
    game.players.push(Player::new(Character::Tails, clone_x + 60.0, clone_y, &GameplayConfig::default()));
    game.players.push(Player::new(Character::Knux, clone_x + 2000.0, clone_y - 200.0, &GameplayConfig::default()));
    hold_each(&mut game, &[0, 0, 0], 2);
    assert_eq!(game.exeller_clones[0].revealed, Some(1), "Tails 60 px away is shown, Knuckles far away is not");
}
