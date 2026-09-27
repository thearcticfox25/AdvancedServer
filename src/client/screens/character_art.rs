//! The pictures of the character select stage (obj_lobby states 2 and 3): each
//! character's big picture standing at the left, its description strip at the bottom,
//! and its lobby icon. The lobby's round intro and the Singleplayer character page
//! both draw them.

use crate::client::canvas::Canvas;
use crate::client::net;
use crate::core::resources::names::sprite;
use crate::core::resources::SpriteId;
use crate::core::config::ticks_per_second;

/// spr_menu_* art and spr_char_info frames in this order: survivors tails ... sally, then exe ... exeller.
pub const CHARACTER_ART: [SpriteId; 10] = [
    sprite::SPR_MENU_TAILS,
    sprite::SPR_MENU_KNUX,
    sprite::SPR_MENU_EGG,
    sprite::SPR_MENU_AMY,
    sprite::SPR_MENU_CREAM,
    sprite::SPR_MENU_SALLY,
    sprite::SPR_MENU_EXE,
    sprite::SPR_MENU_CHAOS,
    sprite::SPR_MENU_EXETIOR,
    sprite::SPR_MENU_EXELLER,
];
pub const SURVIVOR_ART_COUNT: usize = 6;
pub const EXE_ART_COUNT: usize = 4;

/// obj_lobby Draw: the picture stands on (132, 230), the strip is drawn at (1, 227).
pub const ART_POSITION: (f64, f64) = (132.0, 230.0);
pub const DESCRIPTION_POSITION: (f64, f64) = (1.0, 227.0);

/// obj_lobby_icon: the EXE icons, by EXE character, which animate on their own.
const EXE_ICONS: [SpriteId; 4] = [sprite::SPR_LOBBY_EXEICON, sprite::SPR_LOBBY_EXEICON2, sprite::SPR_LOBBY_EXEICON3, sprite::SPR_LOBBY_EXEICON4];
const EXE_ICON_FRAME_MILLISECONDS: f64 = 100.0;
/// spr_lobby_icon frames: "?", the EXE ring, then survivor n (1-based) on frame n + 1.
const SURVIVOR_FRAME_OFFSET: f64 = 1.0;
/// spr_lobby_icon's first frame, the "?" of a character nobody has chosen yet.
pub const UNKNOWN_ICON_FRAME: f64 = 0.0;

/// spr_playerhealth: seven frames per survivor - the five health steps, then downed
/// and dead. The health row at the bottom of a round draws them (level/hud.rs).
pub const HEALTH_FRAMES_PER_CHARACTER: f64 = 7.0;

/// The health row's icon of a survivor (0-based) at full health, or of its demon.
/// The demon sheet has one frame per survivor and no health steps: a demon's health
/// is nobody's business but its own.
pub fn health_row_icon(survivor: usize, demonized: bool) -> (SpriteId, f64) {
    if demonized {
        (sprite::SPR_PLAYERHEALTH_DEMON, survivor as f64)
    } else {
        (sprite::SPR_PLAYERHEALTH, survivor as f64 * HEALTH_FRAMES_PER_CHARACTER)
    }
}

/// spr_lobby_icon_arrow around the icon being chosen: current_time / 150 % 3.
pub fn icon_arrow_frame(current_time_ms: f64) -> f64 {
    (current_time_ms / ICON_ARROW_FRAME_MILLISECONDS) % ICON_ARROW_FRAMES
}
const ICON_ARROW_FRAME_MILLISECONDS: f64 = 150.0;
const ICON_ARROW_FRAMES: f64 = 3.0;

/// Index into CHARACTER_ART of a character: survivors 1..6, or EXE with its EXE character.
pub fn art_index(character: i32, exe_character: i32) -> Option<usize> {
    if character == net::EXE_CHARACTER {
        return usize::try_from(exe_character).ok().filter(|&exe| exe < EXE_ART_COUNT).map(|exe| SURVIVOR_ART_COUNT + exe);
    }
    usize::try_from(character - 1).ok().filter(|&survivor| survivor < SURVIVOR_ART_COUNT)
}

/// The frame of a big picture `ticks` steps after it started moving (charIndex).
pub fn art_frame(canvas: &Canvas, art: usize, ticks: f64) -> f64 {
    ticks * canvas.sprites.get(CHARACTER_ART[art]).fps as f64 / ticks_per_second()
}

/// The lobby icon of a character, by its index into CHARACTER_ART: its sprite and frame.
pub fn icon(art: usize, current_time_ms: f64) -> (SpriteId, f64) {
    match art.checked_sub(SURVIVOR_ART_COUNT) {
        Some(exe) => (EXE_ICONS[exe], current_time_ms / EXE_ICON_FRAME_MILLISECONDS),
        None => (sprite::SPR_LOBBY_ICON, (art + 1) as f64 + SURVIVOR_FRAME_OFFSET),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn characters_find_their_pictures_and_icons() {
        assert_eq!(art_index(1, -1), Some(0), "tails");
        assert_eq!(art_index(6, -1), Some(5), "sally");
        assert_eq!(art_index(net::EXE_CHARACTER, 3), Some(9), "exeller");
        assert_eq!(art_index(net::EXE_CHARACTER, -1), None, "EXE whose character is not known yet");
        assert_eq!(art_index(-1, -1), None, "not in the round");
        // Tails is survivor 1: frame 2 of spr_lobby_icon.
        assert_eq!(icon(0, 0.0), (sprite::SPR_LOBBY_ICON, 2.0));
        assert_eq!(icon(9, 0.0).0, sprite::SPR_LOBBY_EXEICON4);
    }
}
