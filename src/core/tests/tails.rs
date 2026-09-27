//! Behavior checks of Tails' abilities in a real room.

use super::common::{greenhill_with, hold};
use crate::core::events::SimEvent;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Buttons, Character};

#[test]
fn pressing_jump_in_the_air_starts_flight() {
    let mut game = greenhill_with(&[Character::Tails]);
    hold(&mut game, 0, 120);
    assert!(game.players[0].is_grounded);

    // Jump, release, then press again while airborne. Flight is only ready after
    // the 7 second ground recharge (flyTimer from -420 upward), which the start value already is.
    hold(&mut game, Buttons::A, 1);
    hold(&mut game, 0, 5);
    hold(&mut game, Buttons::A, 1);
    assert!(game.players[0].is_flying, "second jump press in the air should start flight: {:?}", game.players[0]);
}

#[test]
fn releasing_b_fires_a_shot_that_flies() {
    let mut game = greenhill_with(&[Character::Tails]);
    hold(&mut game, 0, 120);

    hold(&mut game, Buttons::B, 60);
    assert!(game.players[0].attack_charge > 0, "holding B charges the shot");
    let events = hold(&mut game, 0, 1);
    assert!(
        events.iter().any(|event| matches!(event, SimEvent::SpawnTailsProjectile { damage: 2, .. })),
        "a 1 second charge stuns for 1 + 61 / 42 = 2 seconds: {events:?}"
    );
    assert_eq!(game.projectiles.len(), 1);
    let start_x = game.projectiles[0].x;
    let dir = game.projectiles[0].dir;
    hold(&mut game, 0, 1);
    let shot = &game.projectiles[0];
    assert!(!shot.is_breaking, "open ground ahead of the spawn point, the shot should not break yet");
    assert_eq!(shot.x, start_x + dir * 14.0, "the shot moves 14 px per tick");
}

#[test]
fn every_tails_has_a_shot_of_its_own() {
    // Two Tails in one round (duplicate survivors allowed): both fire at once.
    let mut game = greenhill_with(&[Character::Tails, Character::Tails]);
    hold(&mut game, 0, 120);
    hold(&mut game, Buttons::B, 60);
    hold(&mut game, 0, 1);
    assert_eq!(game.projectiles.len(), 2, "one flying shot per Tails");
    assert_ne!(game.projectiles[0].owner, game.projectiles[1].owner);

    // A second shot of the same Tails waits for its first one.
    hold(&mut game, Buttons::B, 60);
    hold(&mut game, 0, 1);
    assert!(game.projectiles.len() <= 2, "still at most one flying shot per Tails: {}", game.projectiles.len());
}

#[test]
fn a_demon_tails_shot_starts_at_the_demon_not_at_the_other_tails() {
    // The original kept one shot for the whole round: a second Tails firing moved it,
    // so a demonized Tails' shot could appear inside the survivor Tails.
    let mut game = greenhill_with(&[Character::Tails, Character::Tails]);
    let second_spawn = game.world.ids_of(ObjectId::Spawnpoint).nth(1).expect("greenhill has two spawn points");
    let (x, y) = (game.world.instances[second_spawn].x, game.world.instances[second_spawn].y - 18.0);
    assert!((x - game.players[0].x).hypot(y - game.players[0].y) > 128.0, "the two Tails stand apart");
    (game.players[1].x, game.players[1].y) = (x, y);
    game.players[1].revival_times = 2;
    hold(&mut game, 0, 120);
    let survivor_hp = game.players[0].hp;
    hold(&mut game, Buttons::B, 60);
    hold(&mut game, 0, 1);

    assert_eq!(game.projectiles.len(), 2, "the survivor's and the demon's shot fly together");
    for shot in &game.projectiles {
        let owner = &game.players[shot.owner];
        assert!((shot.x - owner.x).abs() < 64.0 && (shot.y - owner.y).abs() < 64.0, "shot of player {} starts at them", shot.owner);
        assert_eq!(shot.hurts_survivors, owner.is_demonized());
    }
    assert_eq!(game.players[0].hp, survivor_hp, "the demon's shot did not hit the survivor Tails far away");
}
