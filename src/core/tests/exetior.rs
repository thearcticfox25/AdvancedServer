//! Behavior checks of Exetior in a real room.

use super::common::{flat_floor_spot, greenhill_with, greenhill_with_exe, hold, hold_each};
use crate::core::config::GameplayConfig;
use crate::core::events::SimEvent;
use crate::core::player::{Buttons, Character, ExeCharacter, Player};

#[test]
fn ground_dash_recharges_in_one_second() {
    let mut game = greenhill_with_exe(ExeCharacter::Exetior);
    hold(&mut game, 0, 120);
    hold(&mut game, Buttons::B, 1);
    let exetior = &game.players[0];
    assert!(exetior.is_attacking);
    assert_eq!(exetior.attack_timer, 60 - 1);
}

#[test]
fn c_places_a_black_ring() {
    let mut game = greenhill_with_exe(ExeCharacter::Exetior);
    hold(&mut game, 0, 120);
    let events = hold(&mut game, Buttons::C, 1);
    assert!(events.iter().any(|event| matches!(event, SimEvent::SpawnBlackRing { .. })));
    assert_eq!(game.players[0].bring_timer, 42 * 60 - 1);
}

#[test]
fn stomp_falls_fast_spawns_waves_and_holds_exetior() {
    let mut game = greenhill_with_exe(ExeCharacter::Exetior);
    let (x, y) = flat_floor_spot(&game.world);
    game.players[0].x = x;
    game.players[0].y = y - 60.0;
    hold(&mut game, Buttons::B | Buttons::DOWN, 1);
    let exetior = &game.players[0];
    assert!(exetior.is_stomping && !exetior.can_move, "stomp holds Exetior: {exetior:?}");

    let mut waves_spawned = false;
    for _ in 0..60 {
        let events = hold(&mut game, 0, 1);
        if events.iter().any(|event| matches!(event, SimEvent::SpawnStompWaves { .. })) {
            waves_spawned = true;
            break;
        }
    }
    assert!(waves_spawned, "landing spawns the shockwaves: {:?}", game.players[0]);
    assert_eq!(game.stomp_waves.len(), 2, "one wave to each side");

    let mut released = false;
    for _ in 0..120 {
        hold(&mut game, 0, 1);
        if game.players[0].can_move && !game.players[0].is_stomping {
            released = true;
            break;
        }
    }
    assert!(released, "Exetior moves again once the landing pose has played");
}

#[test]
fn stomp_wave_kill_names_the_exetior_as_the_killer() {
    let cfg = GameplayConfig::default();
    let wave_damage = cfg.exetior.stomp_waves.damage;
    let mut game = greenhill_with_exe(ExeCharacter::Exetior);
    let (x, y) = flat_floor_spot(&game.world);
    game.players[0].x = x;
    game.players[0].y = y;
    // Far enough that Exetior's falling body (an attack) does not touch Tails.
    game.players.push(Player::new(Character::Tails, x + 60.0, y, &GameplayConfig::default()));
    hold_each(&mut game, &[0, 0], 120);
    assert!(game.players[1].is_grounded);
    game.players[1].hp = wave_damage;

    hold_each(&mut game, &[Buttons::A, 0], 1);
    hold_each(&mut game, &[0, 0], 8);
    hold_each(&mut game, &[Buttons::B | Buttons::DOWN, 0], 1);
    let mut kill = None;
    for _ in 0..90 {
        let events = hold_each(&mut game, &[0, 0], 1);
        kill = events.into_iter().find(|event| matches!(event, SimEvent::KilledByExe { .. }));
        if game.players[1].hp < wave_damage {
            break;
        }
    }
    assert_eq!(game.players[1].hp, 0, "the wave reaches Tails standing 60 px away and deals its damage: {:?}", game.stomp_waves);
    assert!(matches!(kill, Some(SimEvent::KilledByExe { victim: 1, killer: 0 })), "the kill line is Exetior's: {kill:?}");
}

#[test]
fn a_black_ring_costs_rings_then_health_and_keeps_the_next_one_away() {
    let cfg = GameplayConfig::default();
    let mut game = greenhill_with(&[Character::Tails]);
    hold(&mut game, 0, 120);
    let tails = game.players[0].clone();
    game.world.place_black_ring(tails.x - 15.0, tails.y - 15.0, None);
    game.players[0].rings = 6;
    hold(&mut game, 0, 40);
    assert_eq!((game.players[0].rings, game.players[0].hp), (1, cfg.hurt.max_hp), "five rings pay for it once it has faded in");
    assert!(game.world.black_rings.is_empty(), "taken");

    game.world.place_black_ring(tails.x - 15.0, tails.y - 15.0, None);
    hold(&mut game, 0, 40);
    assert_eq!(game.players[0].hp, cfg.hurt.max_hp - cfg.hazards.black_ring_damage, "without five rings it hurts");

    // Exetior cannot put a ring down next to another.
    let mut game = greenhill_with_exe(ExeCharacter::Exetior);
    hold(&mut game, 0, 120);
    let exetior = game.players[0].clone();
    game.world.place_black_ring(exetior.x + 20.0, exetior.y, None);
    let events = hold(&mut game, Buttons::C, 1);
    assert!(!events.iter().any(|event| matches!(event, SimEvent::SpawnBlackRing { .. })), "too near the first ring");
}
