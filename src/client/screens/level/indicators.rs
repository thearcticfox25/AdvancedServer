//! The arrows around a player (obj_exe_indicator and the other indicator objects): what
//! each side is shown of the others. They are each client's own, as in the original.

use crate::client::canvas::Canvas;
use crate::core::resources::names::sprite;
use crate::core::resources::SpriteId;
use crate::core::collision::sprite_bbox;
use crate::core::contact::hunted;
use crate::core::config::{ticks, GameplayConfig};
use crate::core::player::{Character, ExeCharacter, Player};
use crate::core::snapshot::EntityView;
use crate::core::world::{World, C_WHITE};

/// The arrows sit at the player's place; the clone and spring ones this far from it.
const AT_DISTANCE: f64 = 32.0;
const ALPHA: f64 = 0.7;
/// Eggman's tracker shows its catch for three seconds, a lit lantern for twenty, and a
/// spring's echo fades over two.
const TRACKED_SECONDS: f64 = 3.0;
const LANTERN_SECONDS: f64 = 20.0;
const SPRING_SECONDS: f64 = 2.0;
/// obj_exetior_indicator: a black ring shows the survivors for seven seconds.
const BLACK_RING_SECONDS: f64 = 7.0;
/// obj_demon_indicator: the arrow hangs over the head, higher for the taller characters.
const DEMON_ABOVE: f64 = 25.0 + 9.0;
const DEMON_ABOVE_SMALL: f64 = 20.0 + 9.0;
const DEMON_ABOVE_EGGMAN: f64 = 36.0 + 9.0;

#[derive(Default)]
pub struct Indicators {
    /// The player Eggman's tracker caught, and the ticks left of it.
    tracked: Option<(u16, i32)>,
    /// A lit lantern of Weed Zone, and a spring's echo in Hide and Seek 2.
    lantern: Option<(f64, f64, i32)>,
    spring: Option<(f64, f64, i32)>,
    /// obj_exetior_indicator.showTimer
    black_ring: i32,
}

/// Everything the arrows are drawn from.
pub struct View<'a> {
    pub own: &'a Player,
    pub others: &'a [(u16, Player)],
    pub entities: &'a [EntityView],
    pub own_id: Option<u16>,
    pub view: (f64, f64),
    pub big_ring: Option<(f64, f64)>,
    pub config: &'a GameplayConfig,
}

impl Indicators {
    /// SimEvent::TrackerCaught: the tracker shows its catch to the tracker's own side.
    pub fn tracker_caught(&mut self, victim: u16) {
        self.tracked = Some((victim, ticks(TRACKED_SECONDS)));
    }

    /// SERVER_WDLATERN_ACTIVATE on a survivor's client.
    pub fn lantern_lit(&mut self, x: f64, y: f64) {
        self.lantern = Some((x, y, ticks(LANTERN_SECONDS)));
    }

    /// SimEvent::SpringEcho: a survivor's spring gives EXE their place.
    pub fn spring_echo(&mut self, x: f64, y: f64) {
        self.spring = Some((x, y, ticks(SPRING_SECONDS)));
    }

    /// Exetior placed a black ring.
    pub fn black_ring_placed(&mut self) {
        self.black_ring = ticks(BLACK_RING_SECONDS);
    }

    pub fn step(&mut self) {
        let count_down = |timer: &mut i32| {
            *timer -= 1;
            *timer > 0
        };
        self.tracked = self.tracked.and_then(|(id, mut left)| count_down(&mut left).then_some((id, left)));
        self.lantern = self.lantern.and_then(|(x, y, mut left)| count_down(&mut left).then_some((x, y, left)));
        self.spring = self.spring.and_then(|(x, y, mut left)| count_down(&mut left).then_some((x, y, left)));
        self.black_ring = (self.black_ring - 1).max(0);
    }

    /// Draw GUI of every indicator, in their depth order.
    pub fn draw(&self, canvas: &mut Canvas, world: &World, shown: &View) {
        let own = shown.own;
        let at_player = ((own.x - shown.view.0).ceil(), (own.y - shown.view.1).ceil());
        let angle_to = |x: f64, y: f64| (own.y - y).atan2(x - own.x).to_degrees();
        let arrow = |canvas: &mut Canvas, sprite: SpriteId, x: f64, y: f64, angle: f64, alpha: f64| {
            canvas.draw_sprite_ext(sprite, 0.0, x, y, 1.0, 1.0, angle, C_WHITE, alpha);
        };

        // obj_exeller_indicator_up and _down: where each clone sees a survivor.
        for entity in shown.entities {
            let EntityView::ExellerClone { owner, slot, x, y, .. } = *entity else { continue };
            if Some(owner) != shown.own_id || own.character != Character::Exe {
                continue;
            }
            let clone = sprite_bbox(world.sprites.get(sprite::SPR_EXELLER_CLONE), x as f64, y as f64, 1.0, 1.0, 0.0);
            let seen = shown.others.iter().filter(|(_, player)| hunted(player)).map(|(_, player)| (player, clone.distance(&body(world, player)))).filter(|(_, distance)| *distance < shown.config.exeller.clone_reveal_distance).min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((player, _)) = seen {
                let angle = angle_to(player.x, player.y).to_radians();
                let (x, y) = (at_player.0 + angle.cos() * AT_DISTANCE, at_player.1 - angle.sin() * AT_DISTANCE);
                let sprite = if slot == 0 { sprite::SPR_EXELLER_CLONETRACKER } else { sprite::SPR_EXELLER_CLONETRACKER2 };
                arrow(canvas, sprite, x, y, angle.to_degrees(), ALPHA);
            }
        }

        // obj_exe_indicator, obj_exetior_indicator and obj_exeller_indicator: the nearest
        // survivor, or whoever holds a red ring.
        if own.character == Character::Exe {
            let red_ring = (own.invis_timer <= 0).then(|| shown.others.iter().find(|(_, player)| hunted(player) && player.red_ring_timer > 0)).flatten();
            let nearest = shown.others.iter().filter(|(_, player)| hunted(player)).min_by(|a, b| distance(own, &a.1).total_cmp(&distance(own, &b.1)));
            let tracking = match own.exe_character {
                ExeCharacter::Original => own.invis_timer > 0,
                ExeCharacter::Chaos => own.slime_timer > 0,
                ExeCharacter::Exetior | ExeCharacter::Exeller => self.black_ring > 0,
            };
            if let Some((_, player)) = red_ring.or(nearest.filter(|_| tracking)) {
                arrow(canvas, sprite::SPR_INDICATOR, at_player.0, at_player.1, angle_to(player.x, player.y), 1.0);
            }
            // obj_exe_sprindicator: the spring's echo fades away.
            if let Some((x, y, left)) = self.spring {
                let angle = angle_to(x, y).to_radians();
                let (x, y) = (at_player.0 + angle.cos() * AT_DISTANCE, at_player.1 - angle.sin() * AT_DISTANCE);
                arrow(canvas, sprite::SPR_INDICATOR3, x, y, angle.to_degrees(), ALPHA * left as f64 / ticks(SPRING_SECONDS) as f64);
            }
        }

        // obj_demon_indicator: a demon is shown the nearest EXE.
        if own.is_demonized() {
            let exe = shown.others.iter().filter(|(_, player)| player.character == Character::Exe).min_by(|a, b| distance(own, &a.1).total_cmp(&distance(own, &b.1)));
            if let Some((_, player)) = exe {
                let above = match own.character {
                    Character::Tails | Character::Cream => DEMON_ABOVE_SMALL,
                    Character::Eggman => DEMON_ABOVE_EGGMAN,
                    _ => DEMON_ABOVE,
                };
                arrow(canvas, sprite::SPR_INDICATOR2, at_player.0, at_player.1 - above, angle_to(player.x, player.y), 1.0);
            }
        }

        if own.is_dead {
            return;
        }
        // obj_surv_indicator: whoever Eggman's tracker caught.
        if let Some((id, _)) = self.tracked {
            if let Some((_, player)) = shown.others.iter().find(|(other, _)| *other == id) {
                arrow(canvas, sprite::SPR_SINDICATOR, at_player.0, at_player.1, angle_to(player.x, player.y), 1.0);
            }
        }
        // obj_surv_ringindicator: the big ring, once Ravine Mist's shards are in and this
        // player carries one.
        let shards_in = world.ravine_mist.as_ref().is_some_and(|level| level.found as usize >= shown.config.levels.ravine_mist.shards_needed);
        let carrying = own.shards > 0 && own.hp > 0 && !own.is_demonized();
        if let (true, true, Some((x, y))) = (shards_in, carrying, shown.big_ring) {
            arrow(canvas, sprite::SPR_SINDICATOR2, at_player.0, at_player.1, angle_to(x, y), 1.0);
        }
        // obj_surv_lampindicator: Weed Zone's lit lantern.
        if let (Some((x, y, _)), false) = (self.lantern, own.is_demonized()) {
            arrow(canvas, sprite::SPR_SINDICATOR3, at_player.0, at_player.1, angle_to(x, y), 1.0);
        }
    }
}

fn body(world: &World, player: &Player) -> crate::core::collision::Bbox {
    sprite_bbox(world.sprites.get(player.sprite_index), player.x, player.y, player.image_xscale, 1.0, 0.0)
}

fn distance(own: &Player, other: &Player) -> f64 {
    (own.x - other.x).hypot(own.y - other.y)
}
