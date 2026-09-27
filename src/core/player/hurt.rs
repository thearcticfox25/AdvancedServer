//! scr_player_hurt, scr_player_instakill and falling out of the room.

use super::{gm_sign, Character, Player};
use crate::core::resources::names::{sound, sprite};
use crate::core::resources::{SoundId, SpriteId};
use crate::core::config::{self, GameplayConfig};
use crate::core::events::{SimEvent, EFFECT_SPEED};
use crate::core::objects::ids::ObjectId;
use crate::core::world::{InstanceId, World};

/// Arguments of scr_player_hurt. `Hit::damage(cfg, n)` gives the GML defaults.
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub damage: i32,
    pub xpw: f64,
    pub ypw: f64,
    pub sound: SoundId,
    pub blood: SpriteId,
    /// Knock back even during post-damage invincibility.
    pub ignore: bool,
}

impl Hit {
    pub fn damage(cfg: &GameplayConfig, damage: i32) -> Hit {
        Hit {
            damage,
            xpw: cfg.hurt.default_knockback_x,
            ypw: cfg.hurt.default_knockback_y,
            sound: sound::SND_HURT,
            blood: sprite::SPR_BLOOD1,
            ignore: false,
        }
    }
}

impl Player {
    pub fn hurt(&mut self, world: &World, cfg: &GameplayConfig, hit: Hit, events: &mut Vec<SimEvent>) {
        if world.game_ends || self.is_dead {
            return;
        }
        if self.hurttime > 0 {
            if hit.ignore {
                self.knock_back(hit);
            }
            return;
        }

        // EXE takes no damage; invisible or after the round it is not even knocked back.
        if self.character == Character::Exe && (self.invis_timer > 0 || self.won || self.lost) {
            return;
        }
        if self.character == Character::Sally && self.sally_shield_absorbs(cfg, hit.damage, events) {
            return;
        }
        if self.character != Character::Exe && !self.is_demonized() {
            self.hp -= hit.damage;
            if self.hp <= 0 {
                self.instakill(world, cfg, events);
                return;
            }
        }

        if self.rings > 0 && hit.damage > 0 {
            self.rings = 0;
            events.push(SimEvent::Sound { sound: sound::SND_RINGLOSE, x: self.x, y: self.y });
            events.push(SimEvent::Effect { sprite: sprite::SPR_RINGLOSE, x: self.x, y: self.y, xscale: 1.0, image_speed: EFFECT_SPEED, yspd: 0.0 });
            events.push(SimEvent::CameraShake);
        } else if hit.damage > 0 || hit.sound == sound::SND_BUBLE {
            events.push(SimEvent::Sound { sound: hit.sound, x: self.x, y: self.y });
            events.push(SimEvent::CameraShake);
        }

        if hit.damage > 0 {
            self.is_hurt = true;
            self.is_jumping = false;
            events.push(SimEvent::Effect { sprite: hit.blood, x: self.x, y: self.y, xscale: gm_sign(hit.xpw), image_speed: 1.0, yspd: 0.0 });
        }
        self.knock_back(hit);
        self.hurttime = if hit.damage == 0 { 0 } else { invincibility_ticks(world, cfg) };
    }

    fn knock_back(&mut self, hit: Hit) {
        self.is_grounded = false;
        self.is_spinning = false;
        self.is_looking_down = false;
        self.is_looking_up = false;
        self.xspd = hit.xpw;
        self.yspd = hit.ypw;
    }

    pub fn instakill(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        if self.is_dead || world.game_ends {
            return;
        }
        events.push(SimEvent::PlayerDied { revival_times: self.revival_times });
        self.hp = 0;
        self.dead_timer = cfg.hurt.death_countdown_start;
        self.rings = 0;
        self.xspd = self.image_xscale * cfg.hurt.death_push_x;
        self.is_grounded = false;
        self.is_spinning = false;
        self.is_looking_down = false;
        self.is_looking_up = false;
        self.is_dead = true;
        self.shards = 0;
        self.chunk = (super::CHUNK_UNSET, super::CHUNK_UNSET);
        self.chunk_ticks = 0;
        // The achievements' shard counter restarts on death: client achievements.rs died.
        events.push(SimEvent::Sound { sound: sound::SND_DEAD, x: self.x, y: self.y });
        events.push(SimEvent::Effect { sprite: sprite::SPR_BLOOD2, x: self.x, y: self.y, xscale: 1.0, image_speed: 1.0, yspd: 0.0 });
    }

    /// SERVER_GAME_DEATHTIMER_END: nobody revived the survivor in time. Some come
    /// back as demons, playing for EXE; the others stay down for good.
    pub fn death_timer_end(&mut self, cfg: &GameplayConfig, demonize: bool, events: &mut Vec<SimEvent>) {
        if !demonize {
            self.dead_timer = cfg.hurt.death_countdown_start;
            self.revival_times = 1;
            return;
        }
        events.push(SimEvent::Effect { sprite: sprite::SPR_RING_TELEPORT, x: self.x, y: self.y + 32.0, xscale: 1.0, image_speed: 2.0, yspd: 0.0 });
        events.push(SimEvent::LocalSound { sound: sound::SND_DEMONIZATION });
        // Abilities are ready at once.
        match self.character {
            Character::Tails => {
                self.attack_timer = 0;
                self.fly_timer = super::flight_ready_timer(cfg);
            }
            Character::Knux => {
                self.attack_timer = 0;
                self.glide_timer = 0;
            }
            Character::Eggman => {
                self.djump_recharge = 0;
                self.tracker_recharge = 0;
                self.shield_recharge = 0;
            }
            Character::Amy => {
                self.attack_timer = 0;
                self.hjump_timer = 0;
            }
            Character::Cream => {
                self.fly_timer = 0;
                self.dash_timer = 0;
                self.rings_timer = 0;
            }
            Character::Sally => {
                self.attack_timer = 0;
                self.shield_timer = 0;
                self.shield_recharge = 0;
            }
            Character::Exe => {}
        }
        self.hp = cfg.hurt.max_hp;
        self.is_dead = false;
        self.revival_times = 2;
        self.hurttime = config::ticks(cfg.hurt.demonized_invincibility_seconds);
    }

    /// SERVER_REVIVAL_REVIVED: teammates brought the survivor back.
    pub fn revive(&mut self, cfg: &GameplayConfig) {
        if self.revival_times >= 2 {
            return;
        }
        self.hp = cfg.hurt.revived_hp;
        self.is_dead = false;
        self.revival_times += 1;
        self.hurttime = config::ticks(cfg.hurt.revived_invincibility_seconds);
    }

    /// Fell below the room: back to the nearest abyss target (or death teleport point), with damage.
    pub(super) fn return_from_abyss(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        // Majin Forest catches its own falls: obj_majong_trigger teleports the player
        // to its nearest point, without damage and with its own sound, so the abyss
        // rule stays out of that map even when the fall passed the trigger by.
        if self.teleport_out_of_majin_forest(world, events) {
            return;
        }
        // A map may have neither point (a modder can remove them), and then the player used
        // to fall for ever; the spawn points are always there and are a place to stand.
        let own_spawn = if self.character == Character::Exe { ObjectId::Exespawn } else { ObjectId::Spawnpoint };
        let target = world
            .instance_nearest(self.x, self.y, ObjectId::AbyssTarget)
            .or_else(|| world.instance_nearest(self.x, self.y, ObjectId::DeathtpPoint))
            .or_else(|| world.instance_nearest(self.x, self.y, own_spawn))
            .or_else(|| world.instance_nearest(self.x, self.y, ObjectId::Spawnpoint));
        if let Some(target) = target {
            self.pull_out_of_abyss(world, cfg, target, true, events);
        }
    }

    /// Back up at `target` with damage; EXE and demonized players are also stunned when `stun`.
    pub(super) fn pull_out_of_abyss(&mut self, world: &World, cfg: &GameplayConfig, target: InstanceId, stun: bool, events: &mut Vec<SimEvent>) {
        self.x = world.instances[target].x;
        self.y = world.instances[target].y;
        self.xspd = 0.0;
        self.gspd = 0.0;
        self.yspd = 0.0;
        if self.character == Character::Sally {
            self.is_sliding = false;
        }
        if stun && (self.character == Character::Exe || self.is_demonized()) {
            self.shocked_timer += config::ticks(cfg.hazards.abyss_stun_seconds);
        }
        let hit = Hit { xpw: 0.0, ypw: 0.0, ..Hit::damage(cfg, cfg.hazards.abyss_damage) };
        self.hurt(world, cfg, hit, events);
    }
}

/// Post-damage invincibility, shorter in the round's last minute
/// (global.timeMinutes <= 0 && global.timeSeconds < 60 in the original).
fn invincibility_ticks(world: &World, cfg: &GameplayConfig) -> i32 {
    let last_minute = world.timer_ticks < config::ticks(cfg.hurt.last_minute_seconds) as i64;
    let seconds = if last_minute { cfg.hurt.last_minute_invincibility_seconds } else { cfg.hurt.invincibility_seconds };
    config::ticks(seconds)
}
