//! The preference cards of getting ready, which replace the character select
//! stage of the original: Z opens two cards beside the player's icon, an EXE and
//! a survivor. Up and down pick a card, left and right its choice, Z gets ready
//! with them, X closes the cards. Either card may say "any"; the EXE card may
//! also say "not me".

use super::super::character_art::{icon, icon_arrow_frame, SURVIVOR_ART_COUNT};
use crate::client::canvas::Canvas;
use crate::client::net::NetClient;
use crate::client::options::Options;
use crate::client::text::{draw_text, text_width};
use crate::client::Context;
use crate::packet::{PREFERENCE_ANY, PREFERENCE_REFUSES_EXE};
use crate::core::resources::names::{sound, sprite};
use crate::core::world::{C_GREY, C_WHITE};

pub const EXE_NAMES: [&str; 4] = ["exe", "chaos", "exetior", "exeller"];
pub const SURVIVOR_NAMES: [&str; 6] = ["tails", "knuckles", "eggman", "amy", "cream", "sally"];
/// The choices in the order left and right go through them.
const EXE_CHOICES: [u8; 6] = [PREFERENCE_ANY, 1, 2, 3, 4, PREFERENCE_REFUSES_EXE];
const SURVIVOR_CHOICES: [u8; 7] = [PREFERENCE_ANY, 1, 2, 3, 4, 5, 6];
/// spr_lobby_icon frames: "?" and the EXE ring (the survivors follow, see character_art).
const QUESTION_FRAME: f64 = 0.0;
const EXE_RING_FRAME: f64 = 1.0;
const ANY_NAME: &str = "any";
const REFUSES_EXE_NAME: &str = "not me";
/// The cards stand this far left and right of the player's icon.
const CARD_DISTANCE: f64 = 64.0;
/// Names stand beside the cards, on the outer side, clear of the chat below.
const NAME_BESIDE_CARD: f64 = 34.0;
const NAME_RAISE: f64 = 4.0;
const USED_MARK_OFFSET: f64 = -12.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Card {
    Exe,
    Survivor,
}

pub struct PreferenceCards {
    /// PREFERENCE_ANY, 1-based exe ... exeller, or PREFERENCE_REFUSES_EXE.
    pub exe: u8,
    /// PREFERENCE_ANY or 1-based tails ... sally.
    pub survivor: u8,
    pub is_open: bool,
    active: Card,
}

/// What the player did with the open cards this step.
pub enum CardsInput {
    None,
    Confirm,
}

impl PreferenceCards {
    /// Starts from the wishes the player made last time.
    pub fn new(options: &Options) -> PreferenceCards {
        let known = |value: u8, choices: &[u8]| if choices.contains(&value) { value } else { PREFERENCE_ANY };
        PreferenceCards {
            exe: known(options.preferred_exe, &EXE_CHOICES),
            survivor: known(options.preferred_survivor, &SURVIVOR_CHOICES),
            is_open: false,
            active: Card::Survivor,
        }
    }

    pub fn step(&mut self, context: &mut Context) -> CardsInput {
        let keys = &context.options.keys;
        let input = &context.input;
        if input.pressed(keys.special1.0) {
            self.is_open = false;
            return CardsInput::None;
        }
        if input.pressed(keys.jump.0) {
            return CardsInput::Confirm;
        }
        let mut moved = false;
        if input.pressed(keys.up.0) || input.pressed(keys.down.0) {
            self.active = if self.active == Card::Exe { Card::Survivor } else { Card::Exe };
            moved = true;
        }
        let step = if input.pressed(keys.left.0) {
            -1
        } else if input.pressed(keys.right.0) {
            1
        } else {
            0
        };
        if step != 0 {
            match self.active {
                Card::Exe => self.exe = next_choice(self.exe, step, &EXE_CHOICES),
                Card::Survivor => self.survivor = next_choice(self.survivor, step, &SURVIVOR_CHOICES),
            }
            moved = true;
        }
        if moved {
            context.audio.play(sound::SND_MENU_SELECT, false);
        }
        CardsInput::None
    }

    /// The name of a card's character another ready player already claimed, if any.
    /// Survivors are claimed unless the server shares characters; EXE characters
    /// only when a round has more than one EXE.
    pub fn taken_character(&self, net: &NetClient) -> Option<&'static str> {
        if net.duplicate_characters_allowed {
            return None;
        }
        let others_ready = || net.players.values().filter(|player| player.is_ready);
        if let Some(name) = survivor_name(self.survivor) {
            if others_ready().any(|player| player.preferred_survivor == self.survivor) {
                return Some(name);
            }
        }
        if let Some(name) = exe_name(self.exe).filter(|_| net.exe_count > 1) {
            if others_ready().any(|player| player.preferred_exe == self.exe) {
                return Some(name);
            }
        }
        None
    }

    /// The cards beside the icon at (`icon_x`, `icon_y`): shown while open and while
    /// ready, unless the server lets nobody pick a character.
    pub fn draw(&self, canvas: &mut Canvas, icon_x: f64, icon_y: f64, net: &NetClient, is_ready: bool, current_time_ms: f64) {
        if !net.character_selection || (!self.is_open && !is_ready) {
            return;
        }
        let blend = |card: Card| if !self.is_open || self.active == card { C_WHITE } else { C_GREY };
        let exe_x = icon_x - CARD_DISTANCE;
        let survivor_x = icon_x + CARD_DISTANCE;

        let (exe_sprite, exe_frame) = match self.exe {
            PREFERENCE_ANY => (sprite::SPR_LOBBY_ICON, QUESTION_FRAME),
            PREFERENCE_REFUSES_EXE => (sprite::SPR_LOBBY_ICON, EXE_RING_FRAME),
            exe => icon(SURVIVOR_ART_COUNT + exe as usize - 1, current_time_ms),
        };
        canvas.draw_sprite_ext(exe_sprite, exe_frame, exe_x, icon_y, 1.0, 1.0, 0.0, blend(Card::Exe), 1.0);
        let (survivor_sprite, survivor_frame) = match self.survivor {
            PREFERENCE_ANY => (sprite::SPR_LOBBY_ICON, QUESTION_FRAME),
            survivor => icon(survivor as usize - 1, current_time_ms),
        };
        canvas.draw_sprite_ext(survivor_sprite, survivor_frame, survivor_x, icon_y, 1.0, 1.0, 0.0, blend(Card::Survivor), 1.0);

        // A crossed card: "not me", or a character someone else claimed.
        let taken = self.taken_character(net);
        let exe_crossed = self.exe == PREFERENCE_REFUSES_EXE || (taken.is_some() && taken == exe_name(self.exe));
        let survivor_crossed = taken.is_some() && taken == survivor_name(self.survivor);
        for (crossed, x) in [(exe_crossed, exe_x), (survivor_crossed, survivor_x)] {
            if crossed {
                canvas.draw_sprite(sprite::SPR_LOBBY_ICON_USED, 0.0, x + USED_MARK_OFFSET, icon_y + USED_MARK_OFFSET);
            }
        }

        let exe_label = match self.exe {
            PREFERENCE_REFUSES_EXE => REFUSES_EXE_NAME,
            exe => exe_name(exe).unwrap_or(ANY_NAME),
        };
        draw_text(canvas, exe_x - NAME_BESIDE_CARD - text_width(canvas, exe_label), icon_y - NAME_RAISE, exe_label, C_WHITE, 1.0);
        let survivor_label = survivor_name(self.survivor).unwrap_or(ANY_NAME);
        draw_text(canvas, survivor_x + NAME_BESIDE_CARD, icon_y - NAME_RAISE, survivor_label, C_WHITE, 1.0);

        // spr_lobby_icon_arrow is a pair of arrows around the card, left and right,
        // as it stands around the chosen icon of the menu.
        if self.is_open {
            let arrow_x = if self.active == Card::Exe { exe_x } else { survivor_x };
            let arrow_frame = icon_arrow_frame(current_time_ms);
            canvas.draw_sprite(sprite::SPR_LOBBY_ICON_ARROW, arrow_frame, arrow_x, icon_y);
        }
    }
}

fn exe_name(choice: u8) -> Option<&'static str> {
    EXE_NAMES.get((choice as usize).wrapping_sub(1)).copied()
}

fn survivor_name(choice: u8) -> Option<&'static str> {
    SURVIVOR_NAMES.get((choice as usize).wrapping_sub(1)).copied()
}

/// The next or previous of `choices`, wrapping around.
fn next_choice(current: u8, step: i32, choices: &[u8]) -> u8 {
    let position = choices.iter().position(|&choice| choice == current).unwrap_or(0) as i32;
    choices[(position + step).rem_euclid(choices.len() as i32) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_wrap_around_through_any_and_not_me() {
        assert_eq!(next_choice(PREFERENCE_ANY, -1, &EXE_CHOICES), PREFERENCE_REFUSES_EXE);
        assert_eq!(next_choice(PREFERENCE_REFUSES_EXE, 1, &EXE_CHOICES), PREFERENCE_ANY);
        assert_eq!(next_choice(6, 1, &SURVIVOR_CHOICES), PREFERENCE_ANY);
        assert_eq!(next_choice(2, 1, &SURVIVOR_CHOICES), 3);
    }

    #[test]
    fn exe_characters_are_claimed_only_with_several_exe() {
        let mut net = NetClient::new();
        let mut other = crate::client::net::Player::new("fox".to_string(), 0, false);
        other.is_ready = true;
        other.preferred_exe = 2;
        net.players.insert(7, other);
        let cards = PreferenceCards { exe: 2, survivor: PREFERENCE_ANY, is_open: true, active: Card::Exe };
        assert_eq!(cards.taken_character(&net), None, "one EXE per round: nobody else can hold its character");
        net.exe_count = 2;
        assert_eq!(cards.taken_character(&net), Some("chaos"));
        net.duplicate_characters_allowed = true;
        assert_eq!(cards.taken_character(&net), None);
    }
}
