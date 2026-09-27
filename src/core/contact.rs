//! Players touching players: Step_0 of obj_player_puppet, where every client
//! checked its own player against everyone else's player as it last heard of them.
//! Here every player is checked against every other one after all of them moved,
//! against the others as they were before any contact of this tick: two attacks
//! meeting bounce both players back, as they did on both clients.

use crate::core::resources::names::{sound, sprite};
use crate::core::config::{self, GameplayConfig};
use crate::core::events::{SimEvent, EFFECT_SPEED};
use crate::core::player::amy::AMY_HJUMP;
use crate::core::player::animation::{JUMP, SPIN};
use crate::core::player::exe::{EXE_AIR_ATTACK, EXE_ATTACK};
use crate::core::player::hurt::Hit;
use crate::core::player::{gm_sign, Character, ExeCharacter, Player};
use crate::core::world::World;

/// Mercoin bonus numbers of CLIENT_MERCOIN_BONUS.
const BONUS_STUNNED_EXE: u8 = 1;
const BONUS_AMY_HIT: u8 = 3;
pub const BONUS_DEMON_KILL: u8 = 4;
const BONUS_DEMON_KNUCKLES_UPPERCUT: u8 = 8;
const BONUS_KNUCKLES_UPPERCUT: u8 = 9;
const BONUS_CHAOS_SECOND_ATTACK: u8 = 10;
const BONUS_AMY_STUN: u8 = 11;
/// Amy's big jump hits for its first frames: image_index < 3 in obj_player_puppet.
const AMY_BIG_JUMP_HITTING_FRAMES: f64 = 3.0;
/// Amy is out of reach of a jumping demon while her big jump is this far: image_index <= 3 in obj_amy.
const AMY_BIG_JUMP_SAFE_FRAMES: f64 = 3.0;
/// The knock back of a jump that meets a player: 3 across, 4 up (or down) in obj_* Step_0.
const JUMP_PUSH_X: f64 = 3.0;
const JUMP_PUSH_Y: f64 = 4.0;
/// bounceTimer = 6 (60 Hz steps): EXE bounces off one jumping survivor at a time.
const EXE_BOUNCE_TICKS: i32 = 6;

#[cfg(test)]
pub fn resolve(world: &World, cfg: &GameplayConfig, players: &mut [Player], events: &mut Vec<SimEvent>) {
    let before_contact = players.to_vec();
    for (victim_index, victim) in players.iter_mut().enumerate() {
        resolve_victim(world, cfg, victim, victim_index, &before_contact, events);
    }
}

/// Everyone in `before_contact` (the players before this tick's contacts) against one victim.
pub fn resolve_victim(world: &World, cfg: &GameplayConfig, victim: &mut Player, victim_index: usize, before_contact: &[Player], events: &mut Vec<SimEvent>) {
    for (attacker_index, attacker) in before_contact.iter().enumerate() {
        if attacker_index != victim_index && !attacker.removed {
            touch(world, cfg, victim, victim_index, attacker, attacker_index, events);
            jump_into(world, cfg, victim, victim_index, attacker, attacker_index, events);
        }
    }
}

fn touch(world: &World, cfg: &GameplayConfig, victim: &mut Player, victim_index: usize, attacker: &Player, attacker_index: usize, events: &mut Vec<SimEvent>) {
    if !seen_attacking(attacker) || !bodies_touch(world, attacker, victim) {
        return;
    }
    let attacker_is_demon = attacker.revival_times >= 2 || attacker.character == Character::Exe;
    let victim_is_demon = victim.character == Character::Exe || victim.revival_times >= 2;
    let rules = &cfg.contact;

    if attacker_is_demon && seen_visible(attacker) {
        if victim.character == Character::Exe {
            return;
        }
        let attack_bounces = !matches!(attacker.character, Character::Amy | Character::Eggman)
            && matches!(victim.character, Character::Knux | Character::Sally)
            && victim.is_attacking
            && victim.revival_times < 2;
        if attack_bounces {
            bounce(cfg, victim, attacker, rules.clash_push_from_demon_x, events);
            return;
        }
        if victim.hurttime > 0 || victim.hp <= 0 || victim.revival_times >= 2 {
            return;
        }
        let blocked = (attacker.character != Character::Amy && victim.character == Character::Eggman && victim.is_attacking)
            || (victim.character == Character::Amy && victim.is_attacking);
        if blocked {
            return;
        }
        if victim.character == Character::Sally && victim.shield_timer > 0 {
            break_shield(cfg, victim, attacker, events);
            return;
        }
        hit_survivor(world, cfg, victim, victim_index, attacker, attacker_index, events);
        return;
    }

    if !attacker_is_demon && victim_is_demon {
        if victim.character == Character::Exe && victim.invis_timer > 0 {
            return;
        }
        let attack_bounces = !matches!(victim.character, Character::Amy | Character::Eggman)
            && victim.is_attacking
            && matches!(attacker.character, Character::Knux | Character::Sally);
        if attack_bounces {
            bounce(cfg, victim, attacker, rules.clash_push_from_survivor_x, events);
            return;
        }
        if victim.shocked_timer > 0 || victim.hurttime > 0 {
            return;
        }
        if victim.character == Character::Sally && victim.shield_timer > 0 {
            break_shield(cfg, victim, attacker, events);
            return;
        }
        if victim.character == Character::Amy && victim.is_attacking {
            return;
        }
        stun_demon(cfg, victim, victim_index, attacker, attacker_index, events);
    }
}

/// Players meeting while jumping, not attacking: Step_0 of each character object.
/// A jumping or spinning demon (EXE included) hurts a survivor it runs into; Tails and
/// Knuckles jumping themselves only bounce off it. EXE jumping into a jumping survivor
/// bounces off, unhurt. Eggman, Amy and Sally jump into nobody: their jumps are attacks.
fn jump_into(world: &World, cfg: &GameplayConfig, victim: &mut Player, victim_index: usize, other: &Player, other_index: usize, events: &mut Vec<SimEvent>) {
    let in_air = |player: &Player| player.state == JUMP || player.state == SPIN;
    let jumps_into_others = !matches!(other.character, Character::Eggman | Character::Amy | Character::Sally);
    let victim_jumping = victim.is_jumping || victim.is_spinning;
    if !jumps_into_others || !in_air(other) || !bodies_touch(world, other, victim) {
        return;
    }
    let dir_x = gm_sign(other.x - victim.x);
    let dir_y = gm_sign(other.y - victim.y);

    if victim.character == Character::Exe {
        let free = victim.invis_timer <= 0 && victim.bounce_timer <= 0 && victim.slime_timer <= 0;
        if free && !other.is_demonized() && other.character != Character::Exe && !other.is_hurt && !other.inactive && victim_jumping {
            let hit = Hit { damage: 0, xpw: -dir_x * JUMP_PUSH_X, ypw: -dir_y * JUMP_PUSH_Y, sound: sound::SND_BUBLE, blood: sprite::SPR_BLOOD2, ignore: true };
            victim.hurt(world, cfg, hit, events);
            victim.bounce_timer = crate::core::config::original_ticks(EXE_BOUNCE_TICKS);
        }
        return;
    }

    let other_is_demon = other.is_demonized() || other.character == Character::Exe;
    let slime = other.character == Character::Exe && other.exe_character == ExeCharacter::Chaos && other.slime_timer > 0;
    if !other_is_demon || !seen_visible(other) || slime || victim.is_demonized() || victim.hp <= 0 {
        return;
    }
    let busy = match victim.character {
        Character::Amy => victim.is_attacking || (victim.state == AMY_HJUMP && victim.image_index <= AMY_BIG_JUMP_SAFE_FRAMES),
        Character::Tails => false,
        _ => victim.is_attacking,
    };
    if busy {
        return;
    }
    // Tails and Knuckles in the air only bounce off; everyone else takes the hit.
    let only_bounces = matches!(victim.character, Character::Tails | Character::Knux) && victim_jumping;
    let hit = Hit {
        damage: if only_bounces { 0 } else { cfg.contact.damage },
        xpw: -dir_x * JUMP_PUSH_X,
        ypw: if only_bounces { -dir_y * JUMP_PUSH_Y } else { -JUMP_PUSH_Y },
        sound: if only_bounces { sound::SND_BUBLE } else { sound::SND_HURT },
        blood: sprite::SPR_BLOOD2,
        ignore: matches!(victim.character, Character::Tails | Character::Knux),
    };
    let hp_before = victim.hp;
    victim.hurt(world, cfg, hit, events);
    if victim.hp == hp_before {
        return;
    }
    events.push(SimEvent::BloodScreen { victim: victim_index });
    events.push(SimEvent::PlayerHit { victim: victim_index, attacker: other_index, damage: hp_before - victim.hp, stun_seconds: 0.0 });
    if victim.hp <= 0 {
        if other.character == Character::Exe {
            events.push(SimEvent::KilledByExe { victim: victim_index, killer: other_index });
        } else {
            events.push(SimEvent::MercoinBonus { player: other_index, bonus: BONUS_DEMON_KILL });
        }
    }
}

/// An EXE or demonized attack lands on a survivor.
fn hit_survivor(world: &World, cfg: &GameplayConfig, victim: &mut Player, victim_index: usize, attacker: &Player, attacker_index: usize, events: &mut Vec<SimEvent>) {
    let rules = &cfg.contact;
    let damage = if attacker.character == Character::Exe && attacker.exe_character != ExeCharacter::Chaos { rules.exe_damage } else { rules.damage };
    if attacker.character == Character::Eggman {
        victim.slow_down(cfg, cfg.physics.slowdown_percent, rules.eggman_hit_slow_seconds);
    }
    if attacker.character == Character::Amy {
        events.push(SimEvent::MercoinBonus { player: attacker_index, bonus: BONUS_AMY_HIT });
    }
    let hit = Hit {
        damage,
        xpw: attacker.image_xscale * rules.hit_knockback_x,
        ypw: rules.hit_knockback_y,
        sound: sound::SND_HURT,
        blood: sprite::SPR_BLOOD3,
        ignore: false,
    };
    victim.hurt(world, cfg, hit, events);
    if attacker.character == Character::Exe {
        if attacker.exe_character == ExeCharacter::Chaos && attacker.sprite_index == sprite::SPR_CHAOS_ATTACK2 {
            events.push(SimEvent::MercoinBonus { player: attacker_index, bonus: BONUS_CHAOS_SECOND_ATTACK });
        }
        if victim.hp <= 0 {
            events.push(SimEvent::KilledByExe { victim: victim_index, killer: attacker_index });
        }
    }
    events.push(SimEvent::BloodScreen { victim: victim_index });
    events.push(SimEvent::PlayerHit { victim: victim_index, attacker: attacker_index, damage, stun_seconds: 0.0 });
    if attacker.revival_times >= 2 && victim.hp <= 0 {
        events.push(SimEvent::MercoinBonus { player: attacker_index, bonus: BONUS_DEMON_KILL });
    }
    if attacker.revival_times >= 2 && attacker.character == Character::Knux && attacker.sprite_index == sprite::SPR_EKNUX_ATTACK2 {
        events.push(SimEvent::MercoinBonus { player: attacker_index, bonus: BONUS_DEMON_KNUCKLES_UPPERCUT });
    }
}

/// A survivor's attack stuns EXE or a demonized player.
fn stun_demon(cfg: &GameplayConfig, victim: &mut Player, victim_index: usize, attacker: &Player, attacker_index: usize, events: &mut Vec<SimEvent>) {
    let rules = &cfg.contact;
    let shock = attacker.character == Character::Eggman;
    let (stun_seconds, stun_sound, push) = if shock {
        events.push(SimEvent::Effect { sprite: sprite::SPR_SHOCKPARTICLE, x: victim.x, y: victim.y, xscale: 1.0, image_speed: EFFECT_SPEED, yspd: 0.0 });
        (rules.eggman_stun_seconds, sound::SND_ELECTROSHOCK, 0.0)
    } else {
        (rules.stun_seconds, sound::SND_EXE_STUN, -gm_sign(attacker.x - victim.x) * rules.stun_push_x)
    };
    events.push(SimEvent::PlayerHit { victim: victim_index, attacker: attacker_index, damage: 0, stun_seconds });
    if attacker.character == Character::Knux && attacker.sprite_index == sprite::SPR_KNUX_ATTACK2 {
        events.push(SimEvent::MercoinBonus { player: attacker_index, bonus: BONUS_KNUCKLES_UPPERCUT });
    }
    if victim.character == Character::Sally {
        victim.is_sliding = false;
    }
    victim.shocked_timer = config::ticks(stun_seconds);
    victim.xspd = push;
    victim.yspd = rules.stun_push_y;
    victim.is_grounded = false;
    events.push(SimEvent::Sound { sound: stun_sound, x: victim.x, y: victim.y });
    if attacker.character == Character::Amy {
        events.push(SimEvent::MercoinBonus { player: attacker_index, bonus: BONUS_AMY_STUN });
    }
    if victim.character == Character::Exe {
        events.push(SimEvent::MercoinBonus { player: attacker_index, bonus: BONUS_STUNNED_EXE });
        // scr_exe_stunsound
        let voice = match victim.exe_character {
            ExeCharacter::Original => sound::SND_EXE_STUN2,
            ExeCharacter::Chaos => sound::SND_CHAOS_STUN,
            ExeCharacter::Exeller => sound::SND_EXELLER_STUN,
            ExeCharacter::Exetior => sound::SND_EXETIOR_STUN,
        };
        events.push(SimEvent::Sound { sound: voice, x: victim.x, y: victim.y });
    }
}

/// Two attacks met: the victim's attack stops and it bounces back.
fn bounce(cfg: &GameplayConfig, victim: &mut Player, attacker: &Player, push_x: f64, events: &mut Vec<SimEvent>) {
    victim.is_attacking = false;
    victim.is_hurt = true;
    victim.hurttime = crate::core::config::original_ticks(cfg.contact.bounce_invincibility_ticks);
    victim.xspd = gm_sign(victim.x - attacker.x) * push_x;
    victim.yspd = cfg.contact.clash_push_y;
    victim.is_grounded = false;
    events.push(SimEvent::Sound { sound: sound::SND_BUBLE, x: victim.x, y: victim.y });
}

/// Sally's shield takes the hit and breaks.
fn break_shield(cfg: &GameplayConfig, victim: &mut Player, attacker: &Player, events: &mut Vec<SimEvent>) {
    let rules = &cfg.contact;
    events.push(SimEvent::ShieldBlocked { on_last_hit: !victim.is_demonized() && victim.hp <= rules.damage });
    victim.is_grounded = false;
    victim.is_hurt = true;
    victim.hurttime = crate::core::config::original_ticks(rules.bounce_invincibility_ticks);
    victim.xspd = gm_sign(victim.x - attacker.x) * rules.shield_break_push_x;
    victim.yspd = rules.shield_break_push_y;
    victim.shield_timer = 0;
    events.push(SimEvent::Sound { sound: sound::SND_SALLY_SHIELDBREAK, x: victim.x, y: victim.y });
    let effect = if victim.revival_times >= 2 { sprite::SPR_SHIELDBREAK2 } else { sprite::SPR_SHIELDBREAK };
    events.push(SimEvent::Effect { sprite: effect, x: victim.x, y: victim.y, xscale: 1.0, image_speed: 1.0, yspd: 0.0 });
}

/// Whether others see the player attacking: the attack flag of CLIENT_PLAYER_DATA
/// (obj_netclient End Step), plus the big jump hack of obj_player_puppet.
pub fn seen_attacking(player: &Player) -> bool {
    let frame = player.image_index.floor();
    match player.character {
        Character::Amy if player.state == AMY_HJUMP && frame < AMY_BIG_JUMP_HITTING_FRAMES => true,
        Character::Knux | Character::Amy | Character::Sally | Character::Eggman => player.is_attacking,
        Character::Exe if player.exe_character == ExeCharacter::Original => {
            player.is_attacking && ((player.state == EXE_ATTACK && frame > 0.0) || (player.state == EXE_AIR_ATTACK && frame >= 3.0))
        }
        Character::Exe => player.is_attacking,
        Character::Tails | Character::Cream => false,
    }
}

/// A survivor the killers look for: the one an indicator arrow points at, and the
/// one an Exeller's clone reveals (client/screens/level/indicators.rs, states/round.rs).
pub fn hunted(player: &Player) -> bool {
    player.character != Character::Exe && player.hp > 0 && !player.is_demonized()
}

/// An invisible Sonic.EXE is not drawn for others, and hits nobody.
pub fn seen_visible(player: &Player) -> bool {
    !(player.character == Character::Exe && player.exe_character == ExeCharacter::Original && player.invis_timer > 0)
}

/// place_meeting(attacker.x, attacker.y, victim) with both players' sprites as masks.
pub fn bodies_touch(world: &World, attacker: &Player, victim: &Player) -> bool {
    let body = |player: &Player| crate::core::collision::sprite_bbox(world.sprites.get(player.sprite_index), player.x, player.y, player.image_xscale, 1.0, 0.0);
    body(attacker).overlaps(&body(victim))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::resources::sprites::Sprites;
    use std::path::Path;
    use std::sync::Arc;

    fn world() -> World {
        let textures = Path::new(env!("CARGO_MANIFEST_DIR")).join("Resources/Textures");
        World::empty(Arc::new(Sprites::index(&textures).unwrap()), Arc::new(GameplayConfig::default()))
    }

    fn attacking_exe(cfg: &GameplayConfig) -> Player {
        let mut exe = Player::new_exe(ExeCharacter::Original, 100.0, 100.0, cfg);
        exe.is_attacking = true;
        exe.state = EXE_ATTACK;
        exe.image_index = 1.0;
        exe
    }

    fn jumping_exe(cfg: &GameplayConfig) -> Player {
        let mut exe = Player::new_exe(ExeCharacter::Original, 100.0, 100.0, cfg);
        exe.state = JUMP;
        exe.is_jumping = true;
        exe
    }

    #[test]
    fn a_jumping_demon_hurts_a_survivor_it_runs_into() {
        let (world, cfg) = (world(), GameplayConfig::default());
        let mut players = [jumping_exe(&cfg), Player::new(Character::Cream, 104.0, 100.0, &cfg)];
        let mut events = Vec::new();
        resolve(&world, &cfg, &mut players, &mut events);
        assert_eq!(players[1].hp, cfg.hurt.max_hp - cfg.contact.damage);
        assert!(events.contains(&SimEvent::PlayerHit { victim: 1, attacker: 0, damage: cfg.contact.damage, stun_seconds: 0.0 }));
    }

    #[test]
    fn tails_in_the_air_only_bounces_off_a_jumping_demon() {
        let (world, cfg) = (world(), GameplayConfig::default());
        let mut tails = Player::new(Character::Tails, 104.0, 100.0, &cfg);
        tails.is_jumping = true;
        let mut players = [jumping_exe(&cfg), tails];
        resolve(&world, &cfg, &mut players, &mut Vec::new());
        assert_eq!(players[1].hp, cfg.hurt.max_hp, "no damage");
        assert_eq!(players[1].xspd, JUMP_PUSH_X, "pushed away from EXE, who is on the left");
    }

    #[test]
    fn exe_bounces_off_a_jumping_survivor_unhurt() {
        let (world, cfg) = (world(), GameplayConfig::default());
        let mut knuckles = Player::new(Character::Knux, 104.0, 100.0, &cfg);
        knuckles.state = JUMP;
        knuckles.is_jumping = true;
        let mut players = [jumping_exe(&cfg), knuckles];
        resolve(&world, &cfg, &mut players, &mut Vec::new());
        assert_eq!(players[0].xspd, -JUMP_PUSH_X, "EXE is pushed back, away from Knuckles");
        assert_eq!(players[0].bounce_timer, crate::core::config::original_ticks(EXE_BOUNCE_TICKS));
        assert_eq!(players[1].hp, cfg.hurt.max_hp, "Knuckles, jumping too, only bounces");
    }

    #[test]
    fn exe_attack_hurts_a_touching_survivor_once() {
        let (world, cfg) = (world(), GameplayConfig::default());
        let mut players = [attacking_exe(&cfg), Player::new(Character::Tails, 104.0, 100.0, &cfg)];
        let mut events = Vec::new();
        resolve(&world, &cfg, &mut players, &mut events);
        assert_eq!(players[1].hp, cfg.hurt.max_hp - cfg.contact.exe_damage, "Sonic.EXE hits for his damage");
        assert!(events.contains(&SimEvent::PlayerHit { victim: 1, attacker: 0, damage: cfg.contact.exe_damage, stun_seconds: 0.0 }));

        resolve(&world, &cfg, &mut players, &mut Vec::new());
        assert_eq!(players[1].hp, cfg.hurt.max_hp - cfg.contact.exe_damage, "invincible right after the hit");
    }

    #[test]
    fn exe_is_harmless_while_invisible_or_between_attack_frames() {
        let (world, cfg) = (world(), GameplayConfig::default());
        let mut exe = attacking_exe(&cfg);
        exe.image_index = 0.5;
        let mut players = [exe, Player::new(Character::Tails, 104.0, 100.0, &cfg)];
        resolve(&world, &cfg, &mut players, &mut Vec::new());
        assert_eq!(players[1].hp, cfg.hurt.max_hp, "the first frame of the ground attack does not hit");

        players[0].image_index = 1.0;
        players[0].invis_timer = 10;
        resolve(&world, &cfg, &mut players, &mut Vec::new());
        assert_eq!(players[1].hp, cfg.hurt.max_hp, "invisible EXE hits nobody");
    }

    #[test]
    fn knuckles_attack_stuns_exe_and_meeting_attacks_bounce() {
        let (world, cfg) = (world(), GameplayConfig::default());
        let mut exe = Player::new_exe(ExeCharacter::Original, 100.0, 100.0, &cfg);
        exe.is_attacking = false;
        let mut knuckles = Player::new(Character::Knux, 110.0, 100.0, &cfg);
        knuckles.is_attacking = true;
        let mut players = [exe, knuckles];
        let mut events = Vec::new();
        resolve(&world, &cfg, &mut players, &mut events);
        assert_eq!(players[0].shocked_timer, 180, "stunned for 3 seconds");
        assert_eq!(players[0].xspd, -4.0, "pushed away from Knuckles");
        assert!(events.contains(&SimEvent::MercoinBonus { player: 1, bonus: BONUS_STUNNED_EXE }));

        let mut players = [attacking_exe(&cfg), Player::new(Character::Knux, 110.0, 100.0, &cfg)];
        players[1].is_attacking = true;
        players[0].shocked_timer = 0;
        resolve(&world, &cfg, &mut players, &mut Vec::new());
        assert_eq!(players[1].hp, cfg.hurt.max_hp, "Knuckles' attack meets EXE's and bounces instead of taking damage");
        assert!(!players[1].is_attacking && players[1].xspd == 3.0, "Knuckles bounces off the demon");
        assert!(!players[0].is_attacking && players[0].xspd == -2.0, "and EXE off the survivor");
    }
}
