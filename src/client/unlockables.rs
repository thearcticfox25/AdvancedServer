//! PersistentData/Game/Palettes.toml: the chosen skin of every character look, the
//! chosen pet with the colours of each pet, and the lobby icon (obj_unlockables).

use crate::client::palette::{self, Colours};
use serde::{Deserialize, Serialize};
use std::path::Path;
use crate::core::palettes::PALETTE_TABLES;

const PALETTES_FILE: &str = "PersistentData/Game/Palettes.toml";
/// obj_unlockables.pet when no pet follows the player.
pub const NO_PET: i32 = -1;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Unlockables {
    /// Frame of spr_menu_lobbyicon shown for the player in the lobby.
    pub lobby_icon: i32,
    /// Index into the pet list, or NO_PET.
    pub pet: i32,
    /// Colours of the recolourable pets, by pet index.
    pub pet_palettes: Vec<PetPalette>,
    /// One entry per palette table (crate::core::palettes).
    pub skins: Vec<ChosenSkin>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PetPalette {
    pub from: Colours,
    pub to: Colours,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ChosenSkin {
    /// Character number, plus PALETTE_DEMON or PALETTE_EXE.
    pub table: usize,
    /// The skin's name in the menu ("default", "classic", ...).
    pub name: String,
    /// The custom skin was picked: `to` holds its shifted colours.
    pub custom: bool,
    pub from: Colours,
    pub to: Colours,
}

/// global.petPallettes for the pets that can be recoloured (flicky ... eggor).
const PET_COLOURS: [&[u32]; 7] = [
    &[0x8484e7, 0x4242a5, 0x212184],
    &[0xaddef8, 0x2ebee9, 0x0e6d89, 0xecde2f, 0xdcb936, 0x985000],
    &[0xe00000, 0x800000],
    &[0xe00000, 0x800000],
    &[0x000022, 0x000090, 0x482490, 0x6c48b4],
    &[0xff0000, 0x700000, 0xba0000],
    &[0xfc0000, 0x900000],
];

impl Default for Unlockables {
    fn default() -> Unlockables {
        Unlockables {
            lobby_icon: 0,
            pet: NO_PET,
            pet_palettes: PET_COLOURS
                .iter()
                .map(|colours| PetPalette { from: palette::from_rgb(colours), to: palette::from_rgb(colours) })
                .collect(),
            skins: PALETTE_TABLES
                .iter()
                .map(|table| ChosenSkin {
                    table: table.table,
                    name: "default".to_string(),
                    custom: false,
                    from: palette::default_colours(),
                    to: palette::default_colours(),
                })
                .collect(),
        }
    }
}

impl Unlockables {
    /// The saved choices, or the defaults (written to disk) when there are none.
    /// A broken file is reported and left alone until the next change saves over it.
    pub fn load() -> Unlockables {
        let Ok(text) = std::fs::read_to_string(PALETTES_FILE) else {
            let unlockables = Unlockables::default();
            unlockables.save();
            return unlockables;
        };
        match toml::from_str::<Unlockables>(&text) {
            Ok(mut unlockables) => {
                unlockables.add_missing_entries();
                unlockables
            }
            Err(error) => {
                eprintln!("{PALETTES_FILE} is broken, using the defaults: {error}");
                Unlockables::default()
            }
        }
    }

    pub fn save(&self) {
        let written = Path::new(PALETTES_FILE)
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|_| std::fs::write(PALETTES_FILE, toml::to_string(self).expect("palettes always serialize")));
        if let Err(error) = written {
            eprintln!("failed to save {PALETTES_FILE}: {error}");
        }
    }

    pub fn skin_mut(&mut self, table: usize) -> &mut ChosenSkin {
        let index = self.skins.iter().position(|skin| skin.table == table).expect("every palette table has a chosen skin");
        &mut self.skins[index]
    }

    pub fn skin(&self, table: usize) -> &ChosenSkin {
        self.skins.iter().find(|skin| skin.table == table).expect("every palette table has a chosen skin")
    }

    /// A file written before a table or pet existed gets the defaults for it.
    fn add_missing_entries(&mut self) {
        let defaults = Unlockables::default();
        for skin in defaults.skins {
            if !self.skins.iter().any(|existing| existing.table == skin.table) {
                self.skins.push(skin);
            }
        }
        let known_pets = self.pet_palettes.len();
        self.pet_palettes.extend(defaults.pet_palettes.into_iter().skip(known_pets));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palettes_survive_a_save_and_load() {
        let mut unlockables = Unlockables::default();
        unlockables.pet = 3;
        unlockables.skin_mut(41).custom = true;
        let text = toml::to_string(&unlockables).unwrap();
        let loaded: Unlockables = toml::from_str(&text).unwrap();
        assert_eq!(loaded.pet, 3);
        assert!(loaded.skin(41).custom);
        assert_eq!(loaded.pet_palettes, unlockables.pet_palettes);
    }
}
