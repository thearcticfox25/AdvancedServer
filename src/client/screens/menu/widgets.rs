//! The obj_menu_* instances of room_menu: what each one remembers, built from its
//! placement in the map with the object's variable defaults.

use super::skins::{Pet, Skin};
use crate::client::room::PlacedInstance;
use crate::core::resources::names::SPRITE_FILES;
use crate::core::resources::SpriteId;
use crate::core::objects::ids::ObjectId;
use crate::core::world::C_WHITE;
use crate::core::config::step;

pub struct Widget {
    /// Index of the room layer the instance was placed on.
    pub layer: usize,
    pub builtins: Builtins,
    pub kind: Kind,
}

/// The built-in instance variables draw_self uses.
pub struct Builtins {
    pub x: f64,
    pub y: f64,
    pub xscale: f64,
    pub yscale: f64,
    pub angle: f64,
    pub sprite: Option<SpriteId>,
    pub image_index: f64,
    pub image_speed: f64,
    pub image_blend: u32,
    pub image_alpha: f64,
}

pub enum Kind {
    /// obj_menu_button and its children.
    Button(Button),
    Textbox(Textbox),
    Textpanel { tid: String, text: String },
    AboutText { text: String },
    /// obj_menu_gif: the animation of a mechanics page.
    Gif,
    /// obj_menu_link1 and obj_menu_link2.
    Link { url: String },
    AchievementArrow,
    Version,
    /// obj_menu_warning: the "!" of the join page and the note it opens.
    Warning { popup: bool },
    Credits,
    /// Draws its sprite and nothing else (obj_menu_characters).
    Picture,
    /// obj_menu_character: the picture of the character page being shown.
    CharacterPicture,
    AchievementList { scroll: f64, target_scroll: f64 },
}

pub struct Button {
    pub bid: i32,
    pub bcolumn: i32,
    pub submenu: i32,
    /// The shared Extras row the button belongs to (its page is set when a page opens).
    pub shared_row: Option<i32>,
    pub tid: String,
    /// Words of its own, for a button this port adds (spr_menu_buttons has no frame
    /// for it). Without them the frame's words are shown.
    pub label: Option<String>,
    /// How far the button is raised by the selection animation.
    pub offset: f64,
    /// sY: where the button rests.
    pub rest_y: f64,
    pub kind: ButtonKind,
}

pub enum ButtonKind {
    Plain,
    /// obj_menu_togglebutton: the frame after its first one shows "on".
    Toggle { first_frame: f64, on: bool },
    Skin(Skin),
    /// obj_menu_icon: a lobby icon.
    Icon { index: i32 },
    Pet(Pet),
    /// A Singleplayer map button: it shows the map's name and picks it.
    Level { index: usize },
    /// A Singleplayer character button: global.character and global.exeCharacter,
    /// and whether that survivor is played demonized (revivalTimes 2).
    Character { character: i32, exe_character: i32, demonized: bool },
    /// The two Singleplayer picks that are not a character of their own: one of the
    /// others at random, and watching the level with a free camera and no player.
    RandomCharacter,
    Freecam,
}

pub struct Textbox {
    pub bid: i32,
    pub submenu: i32,
    pub tid: String,
    pub placeholder: String,
    pub limit: usize,
    pub text: String,
    /// prevText: keyboard_string when the text was last taken from it.
    pub previous_keyboard_string: String,
}

impl Button {
    /// One step of the selection animation (obj_menu_button Step_0).
    pub fn step(&mut self, y: &mut f64) {
        if self.offset > 0.0 {
            *y = self.rest_y - self.offset;
            self.offset -= SELECTION_RAISE_FALL_PER_STEP * step();
        } else {
            *y = self.rest_y;
        }
    }
}

/// The words of spr_menu_buttons, by frame. Buttons show them in the menu font instead
/// of the picture, so they can be clicked where the words are.
pub const BUTTON_LABELS: [&str; 63] = [
    "join game", "options", "extra", "exit", "back", "<screen size>", "enter ip address", "general", "controls",
    "hide ping", "show ping", "hide fps", "show fps", "nickname", "up", "down", "left", "right",
    "jump", "special 1", "special 2", "enter", "emote 1", "emote 2", "emote 3", "hide gui", "opacity",
    "reset", "about", "game mechanics", "character info", "credits", "spin", "hit", "parrying", "rebirth",
    "stealth", "rings", "red rings", "demonization", "sudden death", "escape time", "difficulty", "tails", "knuckles",
    "eggman", "amy rose", "cream", "sally", "sonic.exe", "exetior", "exeller", "chaos", "unlockable",
    "achievements", "icons", "skins", "pets", "page 1", "page 2", "page 3", "player list", "idle",
];

/// The "pick" of the two Singleplayer buttons that choose no character.
const RANDOM_PICK: &str = "random";
const FREECAM_PICK: &str = "freecam";

pub const SELECTION_RAISE: f64 = 3.0;
const SELECTION_RAISE_FALL_PER_STEP: f64 = 0.2;

/// The kind of widget an object becomes, or None for objects the menu does not use:
/// the controller itself, the network client, the Android controls editor, and the
/// secret code check (secrets are plain choices in this port).
pub fn build(layer: usize, placed: &PlacedInstance, unlockables: &crate::client::unlockables::Unlockables) -> Option<Widget> {
    use ObjectId::*;
    let number = |name: &str, default: f64| placed.number(name).unwrap_or(default);
    let text = |name: &str, default: &str| placed.string(name).unwrap_or(default).to_string();
    let button = |kind: ButtonKind| {
        let submenu = number("submenu", 0.0) as i32;
        let shared_rows = [super::submenus::EXTRA_TABS, super::submenus::EXTRA_PAGE_ROW, super::submenus::EXTRA_CHARACTER_ROW];
        Kind::Button(Button {
            bid: number("bid", 0.0) as i32,
            bcolumn: number("bcolumn", -1.0) as i32,
            submenu,
            shared_row: shared_rows.contains(&submenu).then_some(submenu),
            tid: text("tid", "0"),
            label: placed.string("label").map(str::to_string),
            offset: 0.0,
            rest_y: placed.y,
            kind,
        })
    };

    let kind = match placed.object {
        // The Singleplayer buttons are plain buttons with the map or character they pick.
        MenuButton if placed.number("level").is_some() => button(ButtonKind::Level { index: number("level", 0.0) as usize }),
        MenuButton if placed.number("character").is_some() => button(ButtonKind::Character {
            character: number("character", 1.0) as i32,
            exe_character: number("exechar", -1.0) as i32,
            demonized: placed.boolean("demon").unwrap_or(false),
        }),
        MenuButton if placed.string("pick") == Some(RANDOM_PICK) => button(ButtonKind::RandomCharacter),
        MenuButton if placed.string("pick") == Some(FREECAM_PICK) => button(ButtonKind::Freecam),
        MenuButton => button(ButtonKind::Plain),
        MenuTogglebutton => button(ButtonKind::Toggle { first_frame: number("image_index", 0.0), on: false }),
        MenuSkin => button(ButtonKind::Skin(Skin::new(placed, unlockables))),
        MenuIcon => button(ButtonKind::Icon { index: number("index", 0.0) as i32 }),
        MenuPet => button(ButtonKind::Pet(Pet::new(number("index", 0.0) as i32, unlockables))),
        MenuTextbox => Kind::Textbox(Textbox {
            bid: number("bid", 0.0) as i32,
            submenu: number("submenu", 0.0) as i32,
            tid: text("tid", ""),
            placeholder: text("placeholder", "enter text..."),
            limit: number("limit", 15.0) as usize,
            text: String::new(),
            previous_keyboard_string: String::new(),
        }),
        MenuTextpanel => Kind::Textpanel { tid: text("tid", ""), text: String::new() },
        MenuAbouttext => Kind::AboutText { text: text("text", "") },
        MenuGif => Kind::Gif,
        MenuLink1 | MenuLink2 => Kind::Link { url: text("open_url", "") },
        MenuAchvarrow => Kind::AchievementArrow,
        MenuVersion => Kind::Version,
        MenuWarning => Kind::Warning { popup: false },
        MenuCredits => Kind::Credits,
        MenuCharacter => Kind::CharacterPicture,
        MenuCharacters => Kind::Picture,
        MenuAchivements => Kind::AchievementList { scroll: 0.0, target_scroll: 0.0 },
        _ => return None,
    };

    // obj_menu_gif shows the sprite named by its "spr" variable.
    let sprite = match placed.object {
        MenuGif => placed.string("spr").and_then(sprite_by_gamemaker_name),
        // These two carry the chat lettering in the project because the original drew
        // their words with it letter by letter; they have no picture of their own.
        MenuVersion | MenuAbouttext => None,
        object => object.info().sprite,
    };
    // Buttons do not animate on their own (image_speed = 0 in their Create events).
    let image_speed = if matches!(kind, Kind::Button(_)) { 0.0 } else { number("image_speed", 1.0) };
    let (image_blend, image_alpha) = placed.colour("image_blend").unwrap_or((C_WHITE, 1.0));
    Some(Widget {
        layer,
        builtins: Builtins {
            x: placed.x,
            y: placed.y,
            xscale: placed.xscale,
            yscale: placed.yscale,
            angle: placed.angle,
            sprite,
            image_index: number("image_index", 0.0),
            image_speed,
            image_blend,
            image_alpha,
        },
        kind,
    })
}

/// A sprite by its GameMaker name ("spr_menu_spin"); files drop the first prefix.
fn sprite_by_gamemaker_name(name: &str) -> Option<SpriteId> {
    let file_name = name.split_once('_').map_or(name, |(_, rest)| rest);
    SPRITE_FILES.iter().position(|file| *file == file_name).map(SpriteId)
}
