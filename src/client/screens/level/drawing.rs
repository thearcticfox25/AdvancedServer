//! Draw events of what a level shows: the placed instances of the simulation and
//! the players (obj_tails ... obj_exeller, obj_tails_tail).

use crate::client::canvas::Canvas;
use crate::client::palette::Colours;
use crate::core::resources::names::sprite;
use crate::core::resources::SpriteId;
use crate::core::objects::ids::ObjectId;
use crate::core::player::{Character, Player};
use crate::core::world::{is_a, InstanceId, ObjectVars, World, C_GREY, C_WHITE};

/// spr_electroshield and spr_sallyshield: (current_time / 30) % 41.
/// spr_aiz_zipline over a rider: 12 px to the left, 19 px above where they hang from.
const ZIPLINE_HANDLE_LEFT: f64 = 12.0;
const ZIPLINE_HANDLE_ABOVE: f64 = 19.0;
const SHIELD_FRAME_MILLISECONDS: f64 = 30.0;
const SHIELD_FRAMES: f64 = 41.0;
/// spr_tailscharge stands this far in front of Tails.
const TAILS_CHARGE_AHEAD: f64 = 24.0;
/// Seconds of charge the frames of spr_tailscharge cover: this player's Tails and
/// someone else's (obj_player_puppet) disagree in the original.
const OWN_TAILS_CHARGE_SECONDS: f64 = 3.5;
const OTHER_TAILS_CHARGE_SECONDS: f64 = 5.0;
const HALF_TRANSPARENT: f64 = 0.5;
/// obj_ghz_water Draw: a shock sprite every 97 px, a frame every 45 ms.
const WATER_SHOCK_STEP: f64 = 97.0;
const WATER_SHOCK_FRAME_MILLISECONDS: f64 = 45.0;
/// obj_ghz_wave Step: y = sY + sin(current_time / 250) * 2.
const WAVE_MILLISECONDS_PER_RADIAN: f64 = 250.0;
const WAVE_HEIGHT: f64 = 2.0;
/// obj_hd_crystal: y = sY + sin(current_time / 400) * 2, the hint a frame every 400 ms.
const CRYSTAL_MILLISECONDS: f64 = 400.0;
const CRYSTAL_BOB: f64 = 2.0;
/// obj_tails_tail: image_speed = 0.2.
pub const TAIL_IMAGE_SPEED: f64 = 0.2;

/// What an object's Draw event does.
enum DrawEvent {
    /// No Draw event, or draw_self().
    DrawSelf,
    /// An empty Draw event: collision shapes and indicators whose drawing lives elsewhere.
    Nothing,
    /// A Draw event of its own that is not ported yet.
    NotPorted,
}

/// obj_limpcity_echain1 flickers every 50 ms; the eye's hint changes every 400 ms.
const CHAIN_FLICKER_MILLISECONDS: f64 = 50.0;
const EYE_HINT_MILLISECONDS: f64 = 400.0;

/// obj_marijuna_crystal: the glow's frame moves on every 100 ms.
const CRYSTAL_GLOW_FRAME_MILLISECONDS: f64 = 100.0;

/// obj_fart_dummy's name, 10 px over its top; the statues shiver 3 px either way.
const DUMMY_NAME: &str = "\\im @dumb~";
const DUMMY_NAME_ABOVE: f64 = 10.0;
const STATUE_SHIVER: f64 = 3.0;

/// obj_weed_lantern2 and 3: the lantern hangs this tall, and its light's frame moves on this often.
pub const LANTERN_HEIGHT: f64 = 33.0;
const LANTERN_LIGHT_MILLISECONDS: f64 = 500.0;

/// targetX and targetY of a hanging lantern: the end of its swinging chain.
pub fn lantern_end(world: &World, id: InstanceId) -> (f64, f64) {
    let instance = &world.instances[id];
    let length = instance.sprite_index.map_or(0.0, |sprite| world.sprites.get(sprite).height as f64);
    let angle = instance.image_angle.to_radians();
    (instance.x + angle.sin() * length, instance.y + angle.cos() * length)
}

/// The objects whose Draw event (their own or inherited) is not draw_self().
fn draw_event(object: ObjectId) -> DrawEvent {
    use ObjectId::*;
    match object {
        AbandonMine | AmCollision | Blackfadein | Blackfadeout | DemonIndicator | DotdotdotShitladder | ExeIndicator | ExeSprindicator
        | ExellerIndicator | ExellerIndicatorDown | ExellerIndicatorUp | ExetiorIndicator | FartAss | Flash | FloorParent | GhzSlope | GhzSlope2
        | GhzSlope3 | GhzSlope4 | GhzSlope5 | HdCollision | HealProgress | KafSlope1 | KafSlope2 | MajongSlope | MarijunaSlops | NpWhite
        | PlatformJumptrough | SolidBlock | SolidBlock2 | SolidParent | SurvIndicator | SurvLampindicator | SurvRingindicator | WeedLight | WeedSlope
        | WeedSlopeJumpthrough | YcrSlope | YcrSlope2 => DrawEvent::Nothing,
        // ponytail: the level objects with Draw events of their own draw nothing until each is ported
        Blackring | BluespheresSphere | ChaosLiquid | ExellerClone | QuickeffectFade | RedringScreen | Tile => DrawEvent::NotPorted,
        _ => DrawEvent::DrawSelf,
    }
}

/// The Draw event of a placed instance, as this player's client shows it.
pub fn draw_instance(canvas: &mut Canvas, world: &World, id: InstanceId, own: Option<&Player>, current_time_ms: f64) {
    let instance = &world.instances[id];
    if !instance.visible {
        return;
    }
    let Some(sprite) = instance.sprite_index else { return };
    let (mut x, mut image_index, mut blend) = (instance.x, instance.image_index, instance.image_blend);
    let y = match instance.object {
        ObjectId::GhzWater => {
            let (Some(bbox), ObjectVars::Water { electro: true }) = (world.bbox(id), &instance.vars) else { return };
            // GameMaker's bbox_right is the last pixel inside; the copies start at x = 0, not at the water.
            let copies = (bbox.right - 1.0 - bbox.left) / WATER_SHOCK_STEP + 1.0;
            let frame = current_time_ms / WATER_SHOCK_FRAME_MILLISECONDS;
            let mut index = 0.0;
            while index < copies {
                canvas.draw_sprite(sprite::SPR_GHZ_ELECTROSHOCK, frame, index * WATER_SHOCK_STEP, bbox.top);
                index += 1.0;
            }
            return;
        }
        ObjectId::GhzWave => instance.y + (current_time_ms / WAVE_MILLISECONDS_PER_RADIAN).sin() * WAVE_HEIGHT,
        ObjectId::HdCrystal => {
            let y = instance.y + (current_time_ms / CRYSTAL_MILLISECONDS).sin() * CRYSTAL_BOB;
            let (lit, usable) = match instance.vars {
                ObjectVars::Crystal { lit, usable } => (lit, usable),
                _ => (false, false),
            };
            image_index = if lit { 1.0 } else { 0.0 };
            if usable && own.is_some_and(|own| own.meeting_at(world, own.x, own.y, ObjectId::HdCrystal)) {
                canvas.draw_sprite(sprite::SPR_LIMPCITY_EYE_HINT, current_time_ms / CRYSTAL_MILLISECONDS, instance.x, y);
            }
            y
        }
        // Springs and zipline handles were each client's own copies: these are this player's.
        ObjectId::AizZipline => match own.and_then(|own| own.ziplines.iter().find(|handle| handle.zipline == id)) {
            Some(handle) => {
                x = handle.x;
                handle.y
            }
            None => instance.y,
        },
        // obj_weed_lantern2 and 3: the chain, the lantern at its end (turned by image_alpha,
        // as the original passes it), and its light while lit.
        ObjectId::WeedLantern2 | ObjectId::WeedLantern3 => {
            let (target_x, target_y) = lantern_end(world, id);
            canvas.draw_sprite_ext(sprite, 0.0, x, instance.y, 1.0, 1.0, instance.image_angle, blend, 1.0);
            canvas.draw_sprite_ext(sprite::SPR_WEED_LATERN, image_index, target_x, target_y, 1.0, 1.0, instance.image_alpha, C_WHITE, 1.0);
            if matches!(instance.vars, ObjectVars::Lantern { lit: true, .. }) {
                canvas.draw_sprite(sprite::SPR_WEED_LIGHT, current_time_ms / LANTERN_LIGHT_MILLISECONDS, target_x, target_y + LANTERN_HEIGHT / 2.0);
            }
            return;
        }
        // obj_marijuna_crystal: the stone, and its glow over it.
        ObjectId::MarijunaCrystal => {
            canvas.draw_sprite(sprite, 0.0, x, instance.y);
            if let ObjectVars::MjCrystal { glow, .. } = instance.vars {
                canvas.draw_sprite_ext(sprite::SPR_MARIJUNA_CRYSTAL2, current_time_ms / CRYSTAL_GLOW_FRAME_MILLISECONDS, x, instance.y, 1.0, 1.0, 0.0, C_WHITE, glow);
            }
            return;
        }
        // obj_marijuna_statue: its other look shows as it fades.
        ObjectId::MarijunaStatue => {
            canvas.draw_sprite_ext(sprite, image_index, x, instance.y, 1.0, 1.0, 0.0, blend, instance.image_alpha);
            canvas.draw_sprite_ext(sprite::SPR_MARIJUNA_STATUE2, image_index, x, instance.y, 1.0, 1.0, 0.0, C_WHITE, 1.0 - instance.image_alpha);
            return;
        }
        // Drawn by the level screen over the view.
        ObjectId::MarjiunaStatic | ObjectId::RedringScreen2 => return,
        // obj_limpcity_echain1: flickers while it shocks.
        object if is_a(object, ObjectId::LimpcityEchain1) => {
            let shocking = matches!(instance.vars, ObjectVars::ElectricChain { state } if state == crate::core::level::ChainState::Shocking as u8);
            image_index = if shocking { (current_time_ms / CHAIN_FLICKER_MILLISECONDS).floor() % 2.0 } else { 0.0 };
            instance.y
        }
        // obj_abandon_face: it turns to this player.
        ObjectId::AbandonFace => {
            let angle = own.map_or(instance.image_angle, |own| (instance.y - own.y).atan2(own.x - instance.x).to_degrees());
            canvas.draw_sprite_ext(sprite, image_index, x, instance.y, instance.image_xscale, instance.image_yscale, angle, blend, instance.image_alpha);
            return;
        }
        // obj_limpcity_eyeB: drawn untilted; its pupil comes from the level screen.
        ObjectId::LimpcityEyeb => {
            canvas.draw_sprite(sprite, image_index, x, instance.y);
            return;
        }
        // obj_limpcity_eyeA: the hint while this player is at it.
        ObjectId::LimpcityEyea => {
            if own.is_some_and(|own| own.meeting_at(world, own.x, own.y, ObjectId::LimpcityEyea) && !own.is_dead) {
                canvas.draw_sprite_ext(sprite, image_index, x, instance.y, 1.0, 1.0, instance.image_angle, blend, 1.0);
                canvas.draw_sprite(sprite::SPR_LIMPCITY_EYE_HINT, (current_time_ms / EYE_HINT_MILLISECONDS).floor() % 4.0, x, instance.y);
                return;
            }
            instance.y
        }
        // obj_fart_dummy: awake for survivors, with its name over it.
        ObjectId::FartDummy => {
            let demon = own.is_some_and(|own| own.character == Character::Exe || own.revival_times >= 2);
            let frame = if demon { 0.0 } else { 1.0 };
            canvas.draw_sprite_ext(sprite, frame, x, instance.y, instance.image_xscale, instance.image_yscale, 0.0, blend, instance.image_alpha);
            let height = world.sprites.get(sprite).height as f64;
            let name_x = x - crate::client::text::text_width(canvas, DUMMY_NAME) / 2.0;
            crate::client::text::draw_text(canvas, name_x, instance.y - height - DUMMY_NAME_ABOVE, DUMMY_NAME, C_WHITE, 1.0);
            return;
        }
        // obj_fart_mermer Step: the statues shiver.
        ObjectId::FartMermer => {
            x += macroquad::rand::gen_range(-STATUE_SHIVER, STATUE_SHIVER);
            instance.y + macroquad::rand::gen_range(-STATUE_SHIVER, STATUE_SHIVER)
        }
        object if object == ObjectId::HdSpring || is_a(object, ObjectId::SpringParent) => {
            let (pressed, recharging) = own.map_or((false, false), |own| own.spring_look(id));
            image_index = if pressed { 1.0 } else { 0.0 };
            blend = if recharging { C_GREY } else { C_WHITE };
            instance.y
        }
        _ => instance.y,
    };
    if let DrawEvent::DrawSelf = draw_event(instance.object) {
        canvas.draw_sprite_ext(sprite, image_index, x, y, instance.image_xscale, instance.image_yscale, instance.image_angle, blend, instance.image_alpha);
    }
}

/// A player's skin: the colours its sprites have and the ones drawn instead.
pub struct Skin<'a> {
    pub from: &'a Colours,
    pub to: &'a Colours,
}

/// Draw_0 of the character objects, and obj_tails_tail behind Tails; `blend` is image_blend.
pub fn draw_player(canvas: &mut Canvas, player: &Player, skin: &Skin, blend: u32, tail_frame: f64, is_own: bool, current_time_ms: f64) {
    let (x, y) = (player.x.floor(), player.y.floor());
    let alpha = body_alpha(player, is_own);
    canvas.set_palette_swap(skin.from, skin.to);
    if let Some(tail) = tail_pose(player, is_own) {
        canvas.draw_sprite_ext(tail.sprite, tail_frame, tail.x, tail.y, tail.xscale, 1.0, tail.angle, blend, alpha);
    }
    let upright = player.is_spinning || (player.character == Character::Tails && player.attack_charge > 0);
    let angle = if upright { 0.0 } else { player.angle.to_degrees() };
    canvas.draw_sprite_ext(player.sprite_index, player.image_index, x, y, player.image_xscale, 1.0, angle, blend, alpha);
    canvas.reset_shader();
    // obj_player_puppet Draw: another player riding a zipline holds its handle; this
    // player's own handles are the level's zipline objects.
    if !is_own && player.state == player.zipline_state() {
        let hang = crate::core::player::hang_below_zipline(player.character);
        canvas.draw_sprite(sprite::SPR_AIZ_ZIPLINE, 0.0, x - ZIPLINE_HANDLE_LEFT, y - hang - ZIPLINE_HANDLE_ABOVE);
    }
    // obj_fart_controller and obj_player_puppet Draw: the curse's seconds over its holder.
    if player.potato_ticks > 0 {
        canvas.draw_sprite(sprite::SPR_GOODPERSON, (player.potato_ticks / 60) as f64, x, y);
    }

    let shield_frame = (current_time_ms / SHIELD_FRAME_MILLISECONDS) % SHIELD_FRAMES;
    match player.character {
        Character::Tails if player.attack_charge > 0 => {
            let charge_seconds = if is_own { OWN_TAILS_CHARGE_SECONDS } else { OTHER_TAILS_CHARGE_SECONDS };
            let frame = ((player.attack_charge as f64 / 60.0) / charge_seconds * 100.0).floor();
            let charge_x = x + player.image_xscale * TAILS_CHARGE_AHEAD;
            canvas.draw_sprite_ext(sprite::SPR_TAILSCHARGE, frame, charge_x, y, player.image_xscale, 1.0, 0.0, C_WHITE, 1.0);
        }
        Character::Eggman if player.is_attacking => canvas.draw_sprite(sprite::SPR_ELECTROSHIELD, shield_frame, x, y),
        Character::Sally if player.shield_timer > 0 => {
            let shield = if player.is_demonized() { sprite::SPR_SALLYSHIELD2 } else { sprite::SPR_SALLYSHIELD };
            canvas.draw_sprite(shield, shield_frame, x, y);
        }
        _ => {}
    }
}

/// Half transparent while hiding, and after a hit: while invincible (EXE also while
/// stunned) for this player, while hurt for someone else (obj_player_puppet).
fn body_alpha(player: &Player, is_own: bool) -> f64 {
    let hit = if is_own { player.hurttime > 0 || (player.character == Character::Exe && player.shocked_timer > 0) } else { player.is_hurt };
    if hit || player.is_hiding {
        HALF_TRANSPARENT
    } else {
        1.0
    }
}

struct TailPose {
    sprite: SpriteId,
    x: f64,
    y: f64,
    xscale: f64,
    angle: f64,
}

/// End Step of obj_tails_tail (this player) or obj_puppet_tail (someone else's
/// Tails): where the tail shows, if it shows at all. It is placed from the same whole
/// pixel the body is drawn at: from the exact position, which is fractional for
/// someone moving between snapshots, the two rounded apart and the tail twitched.
fn tail_pose(player: &Player, is_own: bool) -> Option<TailPose> {
    use crate::core::player::animation::{BALANCING, EMOTION1, EMOTION2, EMOTION3, IDLE, JUMP, LOOKDOWN, LOOKUP, SPIN};
    if player.character != Character::Tails {
        return None;
    }
    let hurt_sprite = matches!(player.sprite_index, sprite::SPR_TAILS_HURT | sprite::SPR_ETAILS_HURT);
    if (is_own && hurt_sprite) || (!is_own && player.hp <= 0) {
        return None;
    }
    let (tail1, tail2) = if player.is_demonized() {
        (sprite::SPR_ETAILS_TAIL1, sprite::SPR_ETAILS_TAIL2)
    } else {
        (sprite::SPR_TAILS_TAIL1, sprite::SPR_TAILS_TAIL2)
    };
    let (x, y) = (player.x.floor(), player.y.floor());
    let state = player.state;
    if state == BALANCING {
        let xscale = if is_own { player.edge_dir } else { player.image_xscale };
        return Some(TailPose { sprite: tail1, x: x + xscale * 4.0, y: y + 8.0, xscale, angle: 0.0 });
    }
    let emotes = if is_own { player.emotion } else { matches!(state, EMOTION1 | EMOTION2 | EMOTION3) };
    if state == IDLE || emotes || state == LOOKDOWN || state == LOOKUP {
        let xscale = player.image_xscale;
        return Some(TailPose { sprite: tail2, x: x + xscale * 2.0, y: y + 4.0, xscale, angle: 0.0 });
    }
    if state == JUMP || state == SPIN {
        // Only this player's tail turns with the flight direction.
        let horizontal = if player.is_grounded { player.gspd } else { player.xspd };
        let angle = if is_own { -player.yspd.atan2(horizontal).to_degrees() } else { 0.0 };
        return Some(TailPose { sprite: tail1, x, y: y + 6.0, xscale: 1.0, angle });
    }
    None
}
