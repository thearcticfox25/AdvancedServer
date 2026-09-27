//! The customization cards of the menu: character skins (obj_menu_skin) and pets
//! (obj_menu_pet). A card shows the character or pet in its colours, its name and
//! whether it is equipped.

use crate::client::canvas::Canvas;
use crate::client::palette::{self, Colours};
use crate::client::room::PlacedInstance;
use crate::client::text::{draw_text, text_width};
use crate::client::unlockables::{Unlockables, NO_PET};
use macroquad::rand;
use crate::core::resources::names::sprite;
use crate::core::resources::SpriteId;
use crate::core::palettes::{palette_table, PALETTE_DEMON, PALETTE_EXE};
use crate::core::player::{Character, ExeCharacter};
use crate::core::world::C_WHITE;

/// palette variable of a skin card.
const DEFAULT_SKIN: i32 = -1;
const CUSTOM_SKIN: i32 = 2;
/// Moving the custom colours: at creation, and on each click after it was picked.
const CUSTOM_START_HUE_SHIFT: f64 = 60.0;
const CUSTOM_START_BRIGHTNESS_SHIFT: f64 = -60.0;
const SURVIVOR_CUSTOM_CLICK_HUE_SHIFT: f64 = 10.0;
const EXE_CUSTOM_CLICK_HUE_SHIFT: f64 = 15.0;
/// Exetior and Exeller get brighter while darker than this, then darker in small steps.
const CUSTOM_BRIGHTNESS_LIMIT: f64 = 155.0;
const CUSTOM_BRIGHTER_SHIFT: f64 = 75.0;
const CUSTOM_DARKER_SHIFT: f64 = -15.0;
const PET_CLICK_HUE_SHIFT: f64 = 15.0;

/// Where a card draws its picture and texts, from its top-left corner.
const CARD_CENTER_X: f64 = 32.0;
const SKIN_PICTURE_Y: f64 = 44.0;
const PET_PICTURE_Y: f64 = 32.0;
const CARD_TITLE_Y: f64 = 6.0;
const CARD_STATE_FROM_BOTTOM: f64 = 12.0;

pub struct Skin {
    pub character: i32,
    /// -1 default, 0 classic, 1 eccentric, 2 custom.
    pub palette: i32,
    pub demon: bool,
    pub exe: bool,
    /// tableIndex: the palette table of this character look.
    pub table: usize,
    /// skinName: the names of the four skins of this look.
    pub names: [&'static str; 4],
    pub from: Colours,
    pub to: Colours,
    /// percent: the hue line under the name of a custom skin.
    pub hue_text: String,
}

impl Skin {
    pub fn new(placed: &PlacedInstance, unlockables: &Unlockables) -> Skin {
        let character = placed.number("character").unwrap_or(0.0) as i32;
        let demon = placed.boolean("demon").unwrap_or(false);
        let exe = placed.boolean("exe").unwrap_or(false);
        let mut table = character as usize;
        if demon {
            table += PALETTE_DEMON;
        }
        if exe {
            table += PALETTE_EXE;
        }
        let mut skin = Skin {
            character,
            palette: placed.number("palette").unwrap_or(DEFAULT_SKIN as f64) as i32,
            demon,
            exe,
            table,
            names: skin_names(exe, character),
            from: palette::default_colours(),
            to: palette::default_colours(),
            hue_text: String::new(),
        };
        match skin.palette {
            DEFAULT_SKIN => {}
            CUSTOM_SKIN => skin.start_custom_colours(unlockables),
            classic_or_eccentric => {
                let chosen = &known_table(table).skins[classic_or_eccentric as usize];
                skin.from = palette::from_rgb(chosen.from);
                skin.to = palette::from_rgb(chosen.to);
            }
        }
        skin
    }

    pub fn name(&self) -> &'static str {
        self.names[(self.palette + 1) as usize]
    }

    pub fn is_equipped(&self, unlockables: &Unlockables) -> bool {
        unlockables.skin(self.table).name == self.name()
    }

    /// click: equips the skin. Clicking the equipped custom skin moves its colours further.
    pub fn click(&mut self, unlockables: &mut Unlockables) {
        if self.palette == CUSTOM_SKIN {
            if !unlockables.skin(self.table).custom {
                self.save_to(unlockables, true);
                return;
            }
            self.move_custom_colours();
        } else {
            unlockables.skin_mut(self.table).custom = false;
        }
        let still_custom = unlockables.skin(self.table).custom;
        self.save_to(unlockables, still_custom);
    }

    pub fn draw(&self, canvas: &mut Canvas, x: f64, y: f64, card_height: f64, unlockables: &Unlockables) {
        let (picture, frame, offset_x, offset_y) = self.character_picture();
        canvas.set_palette_swap(&self.from, &self.to);
        if !self.exe && self.character == Character::Tails as i32 {
            let tail = if self.demon { sprite::SPR_ETAILS_TAIL2 } else { sprite::SPR_TAILS_TAIL2 };
            canvas.draw_sprite(tail, 0.0, x + CARD_CENTER_X + 2.0 + offset_x, y + SKIN_PICTURE_Y + 4.0);
        }
        canvas.draw_sprite(picture, frame, x + CARD_CENTER_X + offset_x, y + SKIN_PICTURE_Y + offset_y);
        canvas.reset_shader();
        draw_card_title(canvas, x, y, &format!("{}{}", self.name(), self.hue_text));
        draw_card_state(canvas, x, y, card_height, self.is_equipped(unlockables));
    }

    /// The custom skin starts from the saved colours, or from the look's own
    /// colours moved once.
    fn start_custom_colours(&mut self, unlockables: &Unlockables) {
        let saved = unlockables.skin(self.table);
        if saved.custom {
            self.from = saved.from.clone();
            self.to = saved.to.clone();
            return;
        }
        self.from = palette::from_rgb(known_table(self.table).custom_colours);
        self.to = if self.changes_brightness() {
            palette::shift(&self.from, 0.0, 0.0, CUSTOM_START_BRIGHTNESS_SHIFT)
        } else {
            palette::shift(&self.from, CUSTOM_START_HUE_SHIFT, 0.0, 0.0)
        };
        // The original also works out a brightness text for Exetior and Exeller
        // here, but then overwrites it with this hue text for every custom skin.
        self.hue_text = hue_change_text(&self.from, &self.to);
    }

    fn move_custom_colours(&mut self) {
        if self.changes_brightness() {
            let brightness_shift = if palette::first_colour_value(&self.to) < CUSTOM_BRIGHTNESS_LIMIT {
                CUSTOM_BRIGHTER_SHIFT
            } else {
                CUSTOM_DARKER_SHIFT
            };
            self.to = palette::shift(&self.to, 0.0, 0.0, brightness_shift);
            self.hue_text = format!("\n({})", palette::first_colour_value(&self.to).floor());
            return;
        }
        let hue_shift = if self.exe { EXE_CUSTOM_CLICK_HUE_SHIFT } else { SURVIVOR_CUSTOM_CLICK_HUE_SHIFT };
        self.to = palette::shift(&self.to, hue_shift, 0.0, 0.0);
        self.hue_text = hue_change_text(&self.from, &self.to);
    }

    /// Exetior and Exeller are mostly dark, so their custom skin changes brightness instead of hue.
    fn changes_brightness(&self) -> bool {
        self.exe && (self.character == ExeCharacter::Exetior as i32 || self.character == ExeCharacter::Exeller as i32)
    }

    fn save_to(&self, unlockables: &mut Unlockables, custom: bool) {
        let saved = unlockables.skin_mut(self.table);
        saved.custom = custom;
        saved.from = self.from.clone();
        saved.to = self.to.clone();
        saved.name = self.name().to_string();
        unlockables.save();
    }

    /// Sprite, frame and offset of the character on the card.
    fn character_picture(&self) -> (SpriteId, f64, f64, f64) {
        if self.exe {
            return match self.character {
                0 => (sprite::SPR_EXE_IDLE, 0.0, 0.0, 0.0),
                1 => (sprite::SPR_CHAOS_WON, 3.0, 0.0, 0.0),
                2 => (sprite::SPR_EXETIOR_IDLE, 0.0, 0.0, 0.0),
                3 => (sprite::SPR_EXELLER_IDLE, 0.0, 0.0, 0.0),
                _ => (sprite::SPR_MERFURMU, 0.0, 0.0, 0.0),
            };
        }
        let pick = |normal, demonized| if self.demon { demonized } else { normal };
        match self.character {
            1 => (pick(sprite::SPR_TAILS_IDLE, sprite::SPR_ETAILS_IDLE), 0.0, 4.0, 0.0),
            2 => (pick(sprite::SPR_KNUX_IDLE, sprite::SPR_EKNUX_IDLE), 0.0, 0.0, 0.0),
            3 => (pick(sprite::SPR_EGG_IDLE, sprite::SPR_EEGG_IDLE), 0.0, 0.0, 7.0),
            4 => (pick(sprite::SPR_AMY_IDLE, sprite::SPR_EAMY_IDLE), 0.0, 0.0, 0.0),
            5 => (pick(sprite::SPR_CREAM_IDLE, sprite::SPR_ECREAM_IDLE), 0.0, 2.0, 0.0),
            6 => (pick(sprite::SPR_SALLY_IDLE, sprite::SPR_ESALLY_IDLE), 0.0, 0.0, 4.0),
            _ => (sprite::SPR_MERFURMU, 0.0, 0.0, 0.0),
        }
    }
}

fn skin_names(exe: bool, character: i32) -> [&'static str; 4] {
    let mut names = ["default", "classic", "eccentric", "custom"];
    if !exe {
        return names;
    }
    match character {
        0 => names[2] = "coldblood",
        1 => {
            names[1] = "prismarine";
            names[2] = "vermillion";
        }
        2 => names[2] = "menace",
        3 => names[2] = "oldschool",
        _ => {}
    }
    names
}

fn known_table(table: usize) -> &'static crate::core::palettes::PaletteTable {
    palette_table(table).unwrap_or_else(|| panic!("a skin card uses palette table {table}, which does not exist"))
}

/// "(def.)" while the hue has not moved, the new hue otherwise.
fn hue_change_text(from: &Colours, to: &Colours) -> String {
    let (old_hue, new_hue) = (palette::first_colour_hue(from), palette::first_colour_hue(to));
    if old_hue == new_hue {
        "\n(def.)".to_string()
    } else {
        format!("\n({})", new_hue.floor())
    }
}

/// The name at the top of a card.
fn draw_card_title(canvas: &mut Canvas, x: f64, y: f64, title: &str) {
    draw_text(canvas, x - text_width(canvas, title) / 2.0 + CARD_CENTER_X, y + CARD_TITLE_Y, title, C_WHITE, 1.0);
}

/// "equip" or "equipped" at the bottom of a card.
pub fn draw_card_state(canvas: &mut Canvas, x: f64, y: f64, card_height: f64, equipped: bool) {
    let state = if equipped { "@equipped~" } else { "|equip~" };
    draw_text(canvas, x - text_width(canvas, state) / 2.0 + CARD_CENTER_X, y + card_height - CARD_STATE_FROM_BOTTOM, state, C_WHITE, 1.0);
}

pub struct Pet {
    /// Index in the pet list, or NO_PET for the "none" card.
    pub index: i32,
    pub sprite: SpriteId,
    pub name: &'static str,
    pub from: Colours,
    pub to: Colours,
    pub hue_text: String,
    /// offff: a random phase, so the pets do not bob in step.
    pub bob_phase: f64,
}

/// Pet sprite and name by pet index: obj_menu_pet Create for the first seven, then
/// the pets the original unlocked with codes typed in the credits (obj_pet_secret),
/// named after their sprites.
const PETS: [(SpriteId, &str); 29] = [
    (sprite::SPR_PET_FLICKY, "flicky"),
    (sprite::SPR_PET_CHAO, "chao"),
    (sprite::SPR_PET_METAL, "metal"),
    (sprite::SPR_PET_DALDOL_B, "t. doll"),
    (sprite::SPR_PET_MAJIN, "majong"),
    (sprite::SPR_PET_MKNUX, "m. knux"),
    (sprite::SPR_PET_EGG, "eggor"),
    (sprite::SPR_PET_PATOS, "patos"),
    (sprite::SPR_PET_TITS, "tits"),
    (sprite::SPR_PET_SEWERS, "sewers"),
    (sprite::SPR_PET_STOR, "stor"),
    (sprite::SPR_PET_SELFINJ, "selfinj"),
    (sprite::SPR_PET_UNCLE, "uncle"),
    (sprite::SPR_PET_HAMTER, "hamter"),
    (sprite::SPR_PET_CATWE, "catwe"),
    (sprite::SPR_PET_DANA, "dana"),
    (sprite::SPR_PET_PERDIS, "perdis"),
    (sprite::SPR_PET_MOONWATER, "moonwater"),
    (sprite::SPR_PET_SNIC, "snic"),
    (sprite::SPR_PET_PHILNUX, "philnux"),
    (sprite::SPR_PET_BALS, "bals"),
    (sprite::SPR_PET_TRIZ1, "triz1"),
    (sprite::SPR_PET_TRIZ2, "triz2"),
    (sprite::SPR_PET_TRIZ3, "triz3"),
    (sprite::SPR_PET_ANNETTE, "annette"),
    (sprite::SPR_PET_MERJONG, "merjong"),
    (sprite::SPR_PET_WHISPER, "whisper"),
    (sprite::SPR_PET_MRPIXEL, "mrpixel"),
    (sprite::SPR_PET_SKULL, "skull"),
];
const CHAO_PET: i32 = 1;
/// current_time milliseconds per bob and per animation frame.
const PET_BOB_MILLISECONDS: f64 = 400.0;
const PET_BOB_HEIGHT: f64 = 3.0;
const PET_FRAME_MILLISECONDS: f64 = 150.0;
/// The chao animates ten times slower than the other pets.
const CHAO_FRAME_SLOWDOWN: f64 = 10.0;

impl Pet {
    pub fn new(index: i32, unlockables: &Unlockables) -> Pet {
        let (sprite, name) = usize::try_from(index).ok().and_then(|index| PETS.get(index)).copied().unwrap_or((sprite::SPR_PET_NONE, ""));
        let saved = usize::try_from(index).ok().and_then(|index| unlockables.pet_palettes.get(index));
        let (from, to) = saved.map_or((palette::default_colours(), palette::default_colours()), |saved| (saved.from.clone(), saved.to.clone()));
        // Former secret pets have no colours to shift, so they show no hue.
        let hue_text = if saved.is_some() { format!("\n({})", palette::first_colour_hue(&to).floor()) } else { String::new() };
        Pet {
            index,
            sprite,
            name,
            hue_text,
            from,
            to,
            bob_phase: rand::gen_range(0.0, 99999.0),
        }
    }

    pub fn is_equipped(&self, unlockables: &Unlockables) -> bool {
        unlockables.pet == self.index
    }

    /// click: equips the pet. Clicking the equipped pet moves its colours in hue.
    pub fn click(&mut self, unlockables: &mut Unlockables) {
        let was_equipped = self.is_equipped(unlockables);
        let saved = usize::try_from(self.index).ok().and_then(|index| unlockables.pet_palettes.get_mut(index));
        if let Some(saved) = saved.filter(|_| was_equipped) {
            self.to = palette::shift(&self.to, PET_CLICK_HUE_SHIFT, 0.0, 0.0);
            self.hue_text = format!("\n({})", palette::first_colour_hue(&self.to).floor());
            saved.to = self.to.clone();
        }
        unlockables.pet = self.index;
        unlockables.save();
    }

    pub fn draw(&self, canvas: &mut Canvas, x: f64, y: f64, card_height: f64, unlockables: &Unlockables, current_time_ms: f64) {
        let bob = if self.sprite == sprite::SPR_PET_NONE { 0.0 } else { (current_time_ms / PET_BOB_MILLISECONDS + self.bob_phase).sin() };
        let mut frame = current_time_ms / PET_FRAME_MILLISECONDS;
        if self.index == CHAO_PET {
            frame /= CHAO_FRAME_SLOWDOWN;
        }
        canvas.set_palette_swap(&self.from, &self.to);
        canvas.draw_sprite(self.sprite, frame, x + CARD_CENTER_X, y + PET_PICTURE_Y + bob * PET_BOB_HEIGHT);
        canvas.reset_shader();
        let title = if self.index == NO_PET { "none".to_string() } else { format!("{}{}", self.name, self.hue_text) };
        draw_card_title(canvas, x, y, &title);
        draw_card_state(canvas, x, y, card_height, self.is_equipped(unlockables));
    }
}
