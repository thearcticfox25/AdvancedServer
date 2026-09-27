//! DotDotDot's ladders.

use super::common::{hold_each, started_room};
use crate::core::config::GameplayConfig;
use crate::core::game::Game;
use crate::core::objects::ids::ObjectId;
use crate::core::player::Player;
use crate::core::rooms::ids::RoomId;

#[test]
fn a_ladder_caps_the_speed_of_whoever_is_on_it() {
    let mut game = Game::new(started_room(RoomId::Dotdotdot));
    let ladder = game.world.ids_of(ObjectId::DotdotdotShitladder).next().unwrap();
    let bbox = game.world.bbox(ladder).unwrap();
    let mut sonic = Player::new_exe(crate::core::player::ExeCharacter::Original, bbox.left + 2.0, (bbox.top + bbox.bottom) / 2.0, &GameplayConfig::default());
    sonic.sensor_l.x = bbox.left + 2.0;
    sonic.sensor_l.y = (bbox.top + bbox.bottom) / 2.0;
    sonic.xspd = 12.0;
    game.players.push(sonic);
    hold_each(&mut game, &[0], 1);
    let rules = game.world.config.levels.dotdotdot.clone();
    assert!(game.players[0].on_ladder, "on the ladder");
    assert!(game.players[0].max_h_speed <= rules.ladder_max_speed, "capped: {}", game.players[0].max_h_speed);
}
