//! obj_abyss and obj_deathtp on the maps that have them.

use super::common::{hold_each, started_room};
use crate::core::config::GameplayConfig;
use crate::core::game::Game;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{animation::DEAD, Character, Player, Sensor};
use crate::core::rooms::ids::RoomId;

/// A game in `room` with Tails dropped into the middle of the first instance of `pit`.
fn tails_in_first(room: RoomId, pit: ObjectId) -> (Game, (f64, f64)) {
    let world = started_room(room);
    let pit_id = world.ids_of(pit).next().expect("the map has this pit");
    let bbox = world.bbox(pit_id).unwrap();
    let centre = ((bbox.left + bbox.right) / 2.0, (bbox.top + bbox.bottom) / 2.0);
    let mut game = Game::new(world);
    game.players.push(Player::new(Character::Tails, centre.0, centre.1, &GameplayConfig::default()));
    (game, centre)
}

#[test]
fn abyss_puts_the_player_back_on_the_nearest_target_with_damage() {
    let (mut game, (x, y)) = tails_in_first(RoomId::Angelisland, ObjectId::Abyss);
    let abyss = game.world.ids_of(ObjectId::Abyss).next().unwrap();
    let target = game.world.instance_nearest(game.world.instances[abyss].x, game.world.instances[abyss].y, ObjectId::AbyssTarget).unwrap();
    hold_each(&mut game, &[0], 1);
    let (tails, target) = (&game.players[0], &game.world.instances[target]);
    let cfg = GameplayConfig::default();
    assert_eq!(tails.hp, cfg.hurt.max_hp - cfg.hazards.abyss_damage, "abyss damage");
    // Targets sit on the floor, which lifts a standing player 18 px up in the same tick.
    assert!((tails.x - target.x).abs() < 1.0 && (tails.y - target.y).abs() <= 18.0, "at the target {:?}, from ({x}, {y}): {tails:?}", (target.x, target.y));
    assert_eq!(tails.shocked_timer, 0, "Angel Island's abyss does not stun");
}

#[test]
fn torture_cave_abyss_stuns_a_demonized_player() {
    let (mut game, _) = tails_in_first(RoomId::Torturecave, ObjectId::Abyss);
    game.players[0].revival_times = 2;
    hold_each(&mut game, &[0], 1);
    assert!(game.players[0].shocked_timer > 0, "shouldStun is set on this map: {:?}", game.players[0]);
}

#[test]
fn death_pit_brings_a_body_back_to_the_nearest_point() {
    let (mut game, (x, y)) = tails_in_first(RoomId::Dartower, ObjectId::Deathtp);
    let tails = &mut game.players[0];
    tails.hp = 0;
    tails.is_dead = true;
    tails.state = DEAD;
    tails.sensor_bl = Sensor { x, y, coll: false };
    let point = game.world.instance_nearest(x, y, ObjectId::DeathtpPoint).unwrap();
    let (point_x, point_y) = (game.world.instances[point].x, game.world.instances[point].y);
    hold_each(&mut game, &[0], 1);
    let tails = &game.players[0];
    assert!((tails.x - point_x).abs() < 1.0 && (tails.y - (point_y - 2.0)).abs() <= 18.0, "at {:?}: {tails:?}", (point_x, point_y));
}

#[test]
fn majin_forest_wraps_around_and_catches_a_fall() {
    let world = started_room(RoomId::Majongforest);
    let mut game = Game::new(world);
    let width = game.world.room_width;
    game.players.push(Player::new(Character::Tails, 239.0, 600.0, &GameplayConfig::default()));
    game.players[0].xspd = -1.0;
    hold_each(&mut game, &[0], 1);
    assert!(game.players[0].x > width - 260.0, "running out on the left comes in on the right: {}", game.players[0].x);

    let (mut game, _) = tails_in_first(RoomId::Majongforest, ObjectId::MajongTrigger);
    let events = hold_each(&mut game, &[0], 1);
    assert!(events.contains(&crate::core::events::SimEvent::TeleportFlash));
    let bottom = game.world.bbox(game.world.ids_of(ObjectId::MajongTrigger).next().unwrap()).unwrap().top;
    assert!(game.players[0].y < bottom - 100.0, "back up on a point: {:?}", game.players[0]);
}

/// Below the room, where the trigger is not: the forest still teleports instead of
/// letting the abyss rule of every other map take a fifth of the player's health.
#[test]
fn majin_forest_teleports_a_fall_below_the_room_without_hurting_it() {
    let world = started_room(RoomId::Majongforest);
    let (below, x) = (world.room_height + 100.0, world.room_width / 2.0);
    let mut game = Game::new(world);
    game.players.push(Player::new(Character::Tails, x, below, &GameplayConfig::default()));
    let events = hold_each(&mut game, &[0], 1);
    let tails = &game.players[0];
    assert_eq!(tails.hp, GameplayConfig::default().hurt.max_hp, "the forest's fall costs no health: {tails:?}");
    assert!(tails.y < game.world.room_height, "back inside the room: {tails:?}");
    assert!(events.contains(&crate::core::events::SimEvent::TeleportFlash));
    assert!(
        events.contains(&crate::core::events::SimEvent::LocalSound { sound: crate::core::resources::names::sound::SND_NPTELEPORT }),
        "its own sound, not the hurt one: {events:?}"
    );
}
