//! The pages of the main menu (SUBMENU_* in obj_menu Create) and what each page
//! shows (submenu_data). Buttons in the map name their page by these numbers.
//! They run on without gaps: the pages this mod cut out (17, and 45 for the taunts)
//! leave no holes, so from ABOUT_CREDITS on they differ from the original's numbers.

use crate::core::resources::names::sprite;
use crate::core::resources::SpriteId;

pub const NONE: i32 = 0;
pub const JOIN: i32 = 1;

pub const OPTIONS_GENERAL: i32 = 2;
pub const OPTIONS_CONTROLS: i32 = 3;
pub const OPTIONS_BACK: i32 = 4;
pub const OPTIONS_CONTROLS_MOBILE: i32 = 5;

pub const MECH_SPIN: i32 = 6;
pub const MECH_HIT: i32 = 7;
pub const MECH_PAR: i32 = 8;
pub const MECH_REB: i32 = 9;
pub const MECH_STEAL: i32 = 10;
pub const MECH_RINGS: i32 = 11;
pub const MECH_REDRINGS: i32 = 12;
pub const MECH_DEMON: i32 = 13;
pub const MECH_SUDDEN: i32 = 14;
pub const MECH_ESCAPE: i32 = 15;
pub const MECH_DIFF: i32 = 16;

pub const ABOUT_CREDITS: i32 = 17;
pub const ABOUT_BACK: i32 = 18;

pub const CHAR_TAILS: i32 = 19;
pub const CHAR_KNUX: i32 = 20;
pub const CHAR_EGG: i32 = 21;
pub const CHAR_AMY: i32 = 22;
pub const CHAR_CREAM: i32 = 23;
pub const CHAR_SALLY: i32 = 24;
pub const CHAR_EXE: i32 = 25;
pub const CHAR_CHAOS: i32 = 26;
pub const CHAR_EXETIOR: i32 = 27;
pub const CHAR_EXELLER: i32 = 28;

pub const EXTRA_ACHIEVEMENTS: i32 = 29;
pub const EXTRA_TAILS: i32 = 30;
pub const EXTRA_KNUX: i32 = 31;
pub const EXTRA_EGG: i32 = 32;
pub const EXTRA_AMY: i32 = 33;
pub const EXTRA_CREAM: i32 = 34;
pub const EXTRA_SALLY: i32 = 35;
pub const EXTRA_EXE: i32 = 36;
pub const EXTRA_CHAOS: i32 = 37;
pub const EXTRA_EXETIOR: i32 = 38;
pub const EXTRA_EXELLER: i32 = 39;

pub const EXTRA_SKINS1: i32 = 40;
pub const EXTRA_SKINS2: i32 = 41;
pub const EXTRA_SKINS3: i32 = 42;

pub const EXTRA_BACK: i32 = 43;
/// The pets page (SUBMENU_EXTRA_PENIS in the original).
pub const EXTRA_PETS: i32 = 44;
/// Pages with the pets that were secret codes in the credits of the original.
pub const EXTRA_PETS2: i32 = 45;
pub const EXTRA_PETS3: i32 = 46;

/// Rows of buttons the Extras pages share, one set in the map instead of a copy per page
/// (see Menu::arrange_shared_rows): the section tabs at the bottom, the page numbers of
/// the icon and pet pages, the character names of the skin pages.
pub const EXTRA_TABS: i32 = 47;
pub const EXTRA_PAGE_ROW: i32 = 48;
pub const EXTRA_CHARACTER_ROW: i32 = 49;

/// Singleplayer, which the original has no page for: the map first, then the character.
pub const SINGLE_MAPS: i32 = 50;
pub const SINGLE_CHARACTERS: i32 = 51;

/// Buttons that belong to no page (drawn greyed out).
pub const DISABLED: i32 = 52;

/// Every layer a page may show; changing pages hides them all first.
pub const ALL_MENU_LAYERS: [&str; 57] = [
    "Main", "Main2", "Main3",
    "JoinGame",
    "OptionsGeneral", "OptionsControls", "OptionsControlsMobile", "OptionsBack",
    "AboutSpin", "AboutHit", "AboutPar", "AboutReb", "AboutSteal",
    "AboutRings", "AboutRedRings", "AboutDemon", "AboutSudden", "AboutEscape", "AboutDiff",
    "AboutCredits", "AboutCreditsText", "AboutBack",
    "AboutTails", "AboutKnux", "AboutEgg", "AboutAmy", "AboutCream", "AboutSally",
    "AboutExe", "AboutChaos", "AboutExetior", "AboutExeller", "AboutCharacter",
    "ExtraAchivements", "ExtraAchivementsList", "ExtraTabs", "ExtraPageRow", "ExtraCharacterRow",
    "ExtraTails", "ExtraKnux", "ExtraEgg", "ExtraAmy", "ExtraCream", "ExtraSally",
    "ExtraExe", "ExtraChaos", "ExtraExetior", "ExtraExeller", "ExtraBack",
    "ExtraSkins1", "ExtraSkins2", "ExtraSkins3",
    "ExtraPenis", "ExtraPets2", "ExtraPets3",
    "SingleMaps", "SingleCharacters",
];

pub struct Layout {
    /// Frame of spr_menu_bars behind the page.
    pub bars_frame: f64,
    /// Rows (bCount).
    pub rows: i32,
    /// Columns (bColumnCount).
    pub columns: i32,
    /// true: up and down move between rows, left and right between columns.
    /// false: left and right move between the buttons of one row.
    pub grid: bool,
    pub layers: &'static [&'static str],
    /// The character picture of a character page (obj_menu_character).
    pub character_sprite: Option<SpriteId>,
}

const fn page(bars_frame: f64, rows: i32, columns: i32, grid: bool, layers: &'static [&'static str]) -> Layout {
    Layout { bars_frame, rows, columns, grid, layers, character_sprite: None }
}

const fn character_page(layers: &'static [&'static str], character_sprite: SpriteId) -> Layout {
    Layout { bars_frame: 4.0, rows: 2, columns: 0, grid: true, layers, character_sprite: Some(character_sprite) }
}

/// submenu_data[submenu]
pub fn layout(submenu: i32) -> Layout {
    match submenu {
        NONE => page(0.0, 5, 0, false, &["Main", "Main2", "Main3"]),
        // The address row, then back and Singleplayer side by side (see JOIN_BOTTOM_ROW).
        JOIN => page(1.0, 2, 2, true, &["JoinGame"]),
        OPTIONS_GENERAL => page(2.0, 5, 0, true, &["OptionsGeneral"]),
        OPTIONS_CONTROLS => page(2.0, 11, 2, true, &["OptionsControls"]),
        OPTIONS_BACK => page(2.0, 1, 0, true, &["OptionsBack"]),
        OPTIONS_CONTROLS_MOBILE => page(3.0, 3, 0, false, &["OptionsControlsMobile"]),
        MECH_SPIN => page(4.0, 2, 0, true, &["AboutSpin"]),
        MECH_HIT => page(4.0, 2, 0, true, &["AboutHit"]),
        MECH_PAR => page(4.0, 2, 0, true, &["AboutPar"]),
        MECH_REB => page(4.0, 2, 0, true, &["AboutReb"]),
        MECH_STEAL => page(4.0, 2, 0, true, &["AboutSteal"]),
        MECH_RINGS => page(4.0, 2, 0, true, &["AboutRings"]),
        MECH_REDRINGS => page(4.0, 2, 0, true, &["AboutRedRings"]),
        MECH_DEMON => page(4.0, 2, 0, true, &["AboutDemon"]),
        MECH_SUDDEN => page(4.0, 2, 0, true, &["AboutSudden"]),
        MECH_ESCAPE => page(4.0, 2, 0, true, &["AboutEscape"]),
        MECH_DIFF => page(4.0, 2, 0, true, &["AboutDiff"]),
        ABOUT_CREDITS => page(5.0, 1, 0, true, &["AboutCredits", "AboutCreditsText"]),
        ABOUT_BACK => page(0.0, 1, 0, true, &["AboutBack"]),
        CHAR_TAILS => character_page(&["AboutTails", "AboutCharacter"], sprite::SPR_MENU_TAILS),
        CHAR_KNUX => character_page(&["AboutKnux", "AboutCharacter"], sprite::SPR_MENU_KNUX),
        CHAR_EGG => character_page(&["AboutEgg", "AboutCharacter"], sprite::SPR_MENU_EGG),
        CHAR_AMY => character_page(&["AboutAmy", "AboutCharacter"], sprite::SPR_MENU_AMY),
        CHAR_CREAM => character_page(&["AboutCream", "AboutCharacter"], sprite::SPR_MENU_CREAM),
        CHAR_SALLY => character_page(&["AboutSally", "AboutCharacter"], sprite::SPR_MENU_SALLY),
        CHAR_EXE => character_page(&["AboutExe", "AboutCharacter"], sprite::SPR_MENU_EXE),
        CHAR_CHAOS => character_page(&["AboutChaos", "AboutCharacter"], sprite::SPR_MENU_CHAOS),
        CHAR_EXETIOR => character_page(&["AboutExetior", "AboutCharacter"], sprite::SPR_MENU_EXETIOR),
        CHAR_EXELLER => character_page(&["AboutExeller", "AboutCharacter"], sprite::SPR_MENU_EXELLER),
        EXTRA_ACHIEVEMENTS => page(6.0, 1, 0, true, &["ExtraTabs", "ExtraAchivements", "ExtraAchivementsList"]),
        EXTRA_TAILS => page(0.0, 4, 4, true, &["ExtraTabs", "ExtraCharacterRow", "ExtraTails"]),
        EXTRA_KNUX => page(0.0, 4, 4, true, &["ExtraTabs", "ExtraCharacterRow", "ExtraKnux"]),
        EXTRA_EGG => page(0.0, 4, 4, true, &["ExtraTabs", "ExtraCharacterRow", "ExtraEgg"]),
        EXTRA_AMY => page(0.0, 4, 4, true, &["ExtraTabs", "ExtraCharacterRow", "ExtraAmy"]),
        EXTRA_CREAM => page(0.0, 4, 4, true, &["ExtraTabs", "ExtraCharacterRow", "ExtraCream"]),
        EXTRA_SALLY => page(0.0, 4, 4, true, &["ExtraTabs", "ExtraCharacterRow", "ExtraSally"]),
        EXTRA_EXE => page(0.0, 4, 4, true, &["ExtraTabs", "ExtraCharacterRow", "ExtraExe"]),
        EXTRA_CHAOS => page(0.0, 4, 4, true, &["ExtraTabs", "ExtraCharacterRow", "ExtraChaos"]),
        EXTRA_EXETIOR => page(0.0, 4, 4, true, &["ExtraTabs", "ExtraCharacterRow", "ExtraExetior"]),
        EXTRA_EXELLER => page(0.0, 4, 4, true, &["ExtraTabs", "ExtraCharacterRow", "ExtraExeller"]),
        EXTRA_SKINS1 => page(0.0, 4, 5, true, &["ExtraTabs", "ExtraPageRow", "ExtraSkins1"]),
        EXTRA_SKINS2 => page(0.0, 4, 5, true, &["ExtraTabs", "ExtraPageRow", "ExtraSkins2"]),
        EXTRA_SKINS3 => page(0.0, 3, 5, true, &["ExtraTabs", "ExtraPageRow", "ExtraSkins3"]),
        EXTRA_BACK => page(0.0, 1, 0, true, &["ExtraTabs", "ExtraBack"]),
        // The pet pages have a page row (bid 3) and no title, like the icon pages.
        EXTRA_PETS => page(0.0, 4, 4, true, &["ExtraTabs", "ExtraPageRow", "ExtraPenis"]),
        EXTRA_PETS2 => page(0.0, 4, 6, true, &["ExtraTabs", "ExtraPageRow", "ExtraPets2"]),
        EXTRA_PETS3 => page(0.0, 4, 6, true, &["ExtraTabs", "ExtraPageRow", "ExtraPets3"]),
        // 21 maps in the vote's three columns (seven rows, scrolled), then back.
        SINGLE_MAPS => page(0.0, 8, 3, true, &["SingleMaps"]),
        // Three rows of six: the survivors, the same six demonized, then the four EXE
        // with the random pick and the free camera beside them; then back.
        SINGLE_CHARACTERS => page(0.0, 4, 6, true, &["SingleCharacters"]),
        other => panic!("menu page {other} does not exist"),
    }
}
