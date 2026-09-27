//! The pet that follows a player (obj_pet_flicky and its kin): it trails behind, bobs,
//! wears the colours chosen in the Extras menu, and hides when its owner hides.

use crate::client::canvas::Canvas;
use crate::client::palette::Colours;
use crate::client::unlockables::{PetPalette, NO_PET};
use crate::core::resources::names::sprite;
use crate::core::resources::SpriteId;
use crate::core::player::{Character, Player};
use crate::core::rooms::ids::RoomId;
use crate::core::world::C_WHITE;

/// obj_spawnpoint: the pets, in the order of global.pets; from the seventh on, the
/// secret ones, which are the same pet with another sprite.
const PETS: [Pet; 7] = [
    // spr_pet_flicky: its wings beat faster the further behind it is.
    Pet { sprite: sprite::SPR_PET_FLICKY, palette: 0, follow: 0.21, bob: 5.0, bob_milliseconds: 500.0, parts: false, flapping: true, demon_frame: false },
    Pet { sprite: sprite::SPR_PET_CHAO, palette: 1, follow: 0.22, bob: 4.0, bob_milliseconds: 500.0, parts: false, flapping: false, demon_frame: true },
    Pet { sprite: sprite::SPR_PET_METAL, palette: 2, follow: 0.23, bob: 3.5, bob_milliseconds: 500.0, parts: false, flapping: false, demon_frame: false },
    // spr_pet_daldol: its tail, body, legs and arms lag behind one another.
    Pet { sprite: sprite::SPR_PET_DALDOL, palette: 3, follow: 0.25, bob: 4.0, bob_milliseconds: 500.0, parts: true, flapping: false, demon_frame: false },
    Pet { sprite: sprite::SPR_PET_MAJIN, palette: 4, follow: 0.24, bob: 4.0, bob_milliseconds: 500.0, parts: false, flapping: false, demon_frame: false },
    Pet { sprite: sprite::SPR_PET_MKNUX, palette: 5, follow: 0.21, bob: 3.5, bob_milliseconds: 500.0, parts: false, flapping: false, demon_frame: false },
    Pet { sprite: sprite::SPR_PET_EGG, palette: 6, follow: 0.2, bob: 4.0, bob_milliseconds: 400.0, parts: false, flapping: false, demon_frame: false },
];

/// obj_pet_secret: the sprites of the secret pets, in its own order.
const SECRET_PETS: [SpriteId; 22] = [
    sprite::SPR_PET_PATOS,
    sprite::SPR_PET_TITS,
    sprite::SPR_PET_SEWERS,
    sprite::SPR_PET_STOR,
    sprite::SPR_PET_SELFINJ,
    sprite::SPR_PET_UNCLE,
    sprite::SPR_PET_HAMTER,
    sprite::SPR_PET_CATWE,
    sprite::SPR_PET_DANA,
    sprite::SPR_PET_PERDIS,
    sprite::SPR_PET_MOONWATER,
    sprite::SPR_PET_SNIC,
    sprite::SPR_PET_PHILNUX,
    sprite::SPR_PET_BALS,
    sprite::SPR_PET_TRIZ1,
    sprite::SPR_PET_TRIZ2,
    sprite::SPR_PET_TRIZ3,
    sprite::SPR_PET_ANNETTE,
    sprite::SPR_PET_MERJONG,
    sprite::SPR_PET_WHISPER,
    sprite::SPR_PET_MRPIXEL,
    sprite::SPR_PET_SKULL,
];

/// obj_pet_secret keeps the chao's ways, with its own sprite.
const SECRET: Pet = Pet { sprite: sprite::SPR_PET_CHAO, palette: 1, follow: 0.22, bob: 4.0, bob_milliseconds: 500.0, parts: false, flapping: false, demon_frame: false };

/// How one kind of pet follows and looks.
struct Pet {
    sprite: SpriteId,
    /// Which of the pet palettes it wears.
    palette: usize,
    follow: f64,
    bob: f64,
    bob_milliseconds: f64,
    /// Drawn in parts, each lagging further behind (obj_pet_dadol).
    parts: bool,
    /// Its frames run faster the further behind it is (obj_pet_flicky).
    flapping: bool,
    /// Its second frame is for a killer's pet (obj_pet_chao).
    demon_frame: bool,
}

/// It walks this far behind its owner, half as far while they hide, and catches up
/// from further away than this in one step.
const BEHIND: f64 = 32.0;
const BEHIND_HIDDEN: f64 = 16.0;
const ABOVE: f64 = 10.0;
const CATCH_UP_FROM: f64 = 200.0;
/// obj_pet_flicky: image_speed = 0.2 + distance / 150.
const FLAP_SPEED: f64 = 0.2;
const FLAP_DISTANCE: f64 = 150.0;
/// The parts of obj_pet_dadol trail by these parts of the way.
const PART_LAG: [(f64, f64); 4] = [(4.0, 0.25), (1.0, 0.15), (3.0, 0.26), (2.0, 0.26)];

pub struct Follower {
    kind: usize,
    secret: Option<SpriteId>,
    x: f64,
    y: f64,
    frame: f64,
    /// Black in Act 9, as everything else there.
    blend: u32,
}

impl Follower {
    /// obj_spawnpoint (this player's) and obj_netclient Other_4 (everyone else's):
    /// the pet numbered `chosen` following `player`, if they chose one.
    pub fn new(chosen: i32, room: RoomId, player: &Player) -> Option<Follower> {
        if chosen < 0 || chosen == NO_PET {
            return None;
        }
        let kind = (chosen as usize).min(PETS.len());
        let secret = (chosen as usize >= PETS.len()).then(|| SECRET_PETS.get(chosen as usize - PETS.len()).copied()).flatten();
        let blend = if room == RoomId::Act9 { 0x000000 } else { C_WHITE };
        Some(Follower { kind, secret, x: player.x, y: player.y, frame: 0.0, blend })
    }

    /// Which of the pet palettes (Unlockables::pet_palettes) this pet wears.
    pub fn palette(&self) -> usize {
        self.look().palette
    }

    fn look(&self) -> &'static Pet {
        PETS.get(self.kind).unwrap_or(&SECRET)
    }

    fn sprite(&self) -> SpriteId {
        self.secret.unwrap_or(self.look().sprite)
    }

    /// Draw Begin of the pets: it trails its owner and bobs.
    pub fn step(&mut self, canvas: &Canvas, player: &Player, current_time_ms: f64) {
        let pet = self.look();
        let behind = if player.is_hiding { BEHIND_HIDDEN } else { BEHIND };
        if (self.x - player.x).hypot(self.y - player.y) > CATCH_UP_FROM {
            self.x = player.x - player.image_xscale * BEHIND;
            self.y = player.y - ABOVE;
        }
        let wanted_x = player.x - player.image_xscale * behind;
        let wanted_y = player.y - ABOVE + (current_time_ms / pet.bob_milliseconds).sin() * pet.bob;
        self.x += (wanted_x - self.x) * crate::core::config::eased_share(pet.follow);
        self.y += (wanted_y - self.y) * crate::core::config::eased_share(pet.follow);

        let frames = canvas.sprites.get(self.sprite());
        let (count, fps) = (frames.frame_count as f64, frames.fps as f64);
        let image_speed = if pet.flapping { FLAP_SPEED + (self.x - player.x).hypot(self.y - player.y) / FLAP_DISTANCE } else { 1.0 };
        if count > 0.0 && fps > 0.0 {
            self.frame = (self.frame + image_speed * fps / crate::core::config::ticks_per_second()) % count;
        }
    }

    /// Draw of the pets: in `colours` (its owner's choice), see-through while they hide.
    pub fn draw(&self, canvas: &mut Canvas, colours: Option<&PetPalette>, player: &Player) {
        let pet = self.look();
        let alpha = if player.is_hiding { 0.5 } else { 1.0 };
        let killer = player.character == Character::Exe || player.revival_times >= 2;
        let frame = if pet.demon_frame { f64::from(killer) } else { self.frame };
        let none = Colours::default();
        let (from, to) = colours.map_or((&none, &none), |palette| (&palette.from, &palette.to));
        canvas.set_palette_swap(from, to);
        let sprite = self.sprite();
        if pet.parts && self.secret.is_none() {
            // The parts follow the same way they trail the owner.
            let (xspd, yspd) = (player.x - self.x - player.image_xscale * BEHIND, player.y - self.y);
            for &(part, lag) in &PART_LAG {
                canvas.draw_sprite_ext(sprite, part, self.x - xspd * lag, self.y - yspd * lag, player.image_xscale, 1.0, 0.0, self.blend, alpha);
            }
        }
        canvas.draw_sprite_ext(sprite, frame, self.x, self.y, player.image_xscale, 1.0, 0.0, self.blend, alpha);
        canvas.reset_shader();
    }
}
