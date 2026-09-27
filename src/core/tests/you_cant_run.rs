//! You Can't Run's gas.

use super::common::started_room;
use crate::core::config::GameplayConfig;
use crate::core::events::SimEvent;
use crate::core::level::LevelView;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Buttons, Character, Player};
use crate::core::rooms::ids::RoomId;
use crate::core::world::ObjectVars;

#[test]
fn breathing_gas_brings_the_red_screen_then_eats_rings_then_hurts() {
    let cfg = GameplayConfig::default();
    let mut world = started_room(RoomId::Youcantrun);
    let area = world.ids_of(ObjectId::YcrSmokearea).next().unwrap();
    let ObjectVars::SmokeArea { nid, .. } = world.instances[area].vars else { panic!("smoke areas know their gas") };
    let bbox = world.bbox(area).unwrap();
    let mut tails = Player::new(Character::Tails, bbox.left + 40.0, bbox.top + 40.0, &GameplayConfig::default());
    tails.rings = 1;
    // The area is clear for a tick first, as it is before the gas ever comes.
    tails.tick(&world, Buttons(0), &mut Vec::new());
    LevelView { gassed_area: Some(nid), ..LevelView::default() }.apply(&mut world);

    let mut events = Vec::new();
    let tick = |tails: &mut Player, events: &mut Vec<SimEvent>, count| {
        for _ in 0..count {
            tails.tick(&world, Buttons(0), events);
        }
    };
    tick(&mut tails, &mut events, 61);
    assert!(events.contains(&SimEvent::RedRingStarted), "a second of gas brings the red screen");
    tick(&mut tails, &mut events, 60);
    assert_eq!((tails.rings, tails.hp), (0, cfg.hurt.max_hp), "the next second eats the ring");
    tick(&mut tails, &mut events, 60);
    assert_eq!(tails.hp, cfg.hurt.max_hp - cfg.levels.you_cant_run.gas_damage, "without rings the gas hurts");

    LevelView::default().apply(&mut world);
    let mut clear = Vec::new();
    tails.tick(&world, Buttons(0), &mut clear);
    assert!(tails.gas_timer < 0, "out of the gas the count starts over");
}

#[test]
fn a_clients_game_keeps_the_gas_the_servers_view_set() {
    let mut game = crate::core::game::Game::new(started_room(RoomId::Youcantrun));
    let area = game.world.ids_of(ObjectId::YcrSmokearea).next().unwrap();
    let ObjectVars::SmokeArea { nid, .. } = game.world.instances[area].vars else { panic!("smoke areas know their gas") };
    LevelView { gassed_area: Some(nid), ..LevelView::default() }.apply(&mut game.world);
    game.tick(&[]);
    assert!(matches!(game.world.instances[area].vars, ObjectVars::SmokeArea { gassed: true, .. }), "a game without a level applies no view of its own");
}
