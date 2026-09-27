//! Behavior checks of Sonic.EXE (the original killer) in a real room.

use super::common::{greenhill_with_exe, hold};
use crate::core::resources::names::sprite;
use crate::core::events::SimEvent;
use crate::core::player::hurt::Hit;
use crate::core::player::{Buttons, ExeCharacter};

#[test]
fn c_turns_invisible_and_uses_invisible_poses() {
    let mut game = greenhill_with_exe(ExeCharacter::Original);
    hold(&mut game, 0, 120);
    assert_eq!(game.players[0].sprite_index, sprite::SPR_EXE_IDLE);
    let events = hold(&mut game, Buttons::C, 1);
    assert!(events.iter().any(|event| matches!(event, SimEvent::ExeVanished { .. })));
    let exe = &game.players[0];
    assert_eq!(exe.invis_timer, 15 * 60 - 1, "15 seconds, already counted this tick");
    assert_eq!(exe.sprite_index, sprite::SPR_EXE_INVIS_IDLE, "invisible idle pose");
}

#[test]
fn no_dash_while_invisible_and_dash_when_visible() {
    let mut game = greenhill_with_exe(ExeCharacter::Original);
    hold(&mut game, 0, 120);
    hold(&mut game, Buttons::C, 1);
    hold(&mut game, 0, 1);
    hold(&mut game, Buttons::B, 1);
    assert!(!game.players[0].is_attacking, "invisible EXE cannot attack");

    hold(&mut game, 0, 1);
    let events = hold(&mut game, Buttons::C, 1);
    assert!(events.iter().any(|event| matches!(event, SimEvent::ExeAppeared { .. })), "C again reappears");
    hold(&mut game, 0, 1);
    hold(&mut game, Buttons::B, 1);
    assert!(game.players[0].is_attacking, "visible EXE dashes");
}

#[test]
fn damage_never_lowers_hp_and_invisibility_blocks_knockback() {
    let mut game = greenhill_with_exe(ExeCharacter::Original);
    hold(&mut game, 0, 120);
    let cfg = game.world.config.clone();
    let mut events = Vec::new();

    game.players[0].hurt(&game.world, &cfg, Hit::damage(&cfg, 20), &mut events);
    assert_eq!(game.players[0].hp, 10000, "EXE hp is not reduced");
    assert!(game.players[0].is_hurt, "but the hit still knocks back");

    let mut game = greenhill_with_exe(ExeCharacter::Original);
    hold(&mut game, 0, 120);
    hold(&mut game, Buttons::C, 1);
    game.players[0].hurt(&game.world, &cfg, Hit::damage(&cfg, 20), &mut events);
    assert!(!game.players[0].is_hurt, "invisible EXE ignores hits completely");
}
