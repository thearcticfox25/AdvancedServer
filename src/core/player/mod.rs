//! A player character: obj_tails / obj_knux / ... of the original, plus the
//! scripts they all share (scr_move_basic, scr_collision_basic,
//! scr_collision_objects, scr_player_hurt).
//!
//! The original reads the keyboard inside these scripts. Here the same logic
//! reads `Buttons` for the current tick instead, so the server (authority)
//! and the client (prediction) run exactly this code.

pub(crate) mod amy;
pub mod animation;
mod act9;
mod animation_tables;
mod chaos;
mod cream;
mod dark_tower;
mod doors;
mod echidna_ruins;
mod dotdotdot;
mod egg;
pub(crate) mod exe;
mod exeller;
mod exetior;
mod fart_zone;
pub mod hurt;
mod knux;
mod limp_city;
mod movement;
mod objects;
mod slugs;
mod snow;
mod stages;
pub(crate) mod sally;
pub(crate) mod tails;
mod terrain;
mod volcano;
mod zipline;

pub use animation::Character;
pub use animation::IDLE;
pub use exe::ExeCharacter;
pub use exeller::{CloneSlot, NO_CLONE};
pub use zipline::{hang_below_zipline, ZiplineHandle};

use crate::core::resources::names::sprite;
use crate::core::resources::SpriteId;
use crate::core::config::{self, GameplayConfig, Movement};
use crate::core::events::SimEvent;
use crate::core::world::World;

/// The chunk a player has not been put in yet (obj_player Create: chunkX = -1000).
const CHUNK_UNSET: f64 = -1000.0;

/// One tick of input. Only buttons travel to the server, never positions.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Buttons(pub u16);

impl Buttons {
    pub const LEFT: u16 = 1 << 0;
    pub const RIGHT: u16 = 1 << 1;
    pub const UP: u16 = 1 << 2;
    pub const DOWN: u16 = 1 << 3;
    /// global.KeyA: jump.
    pub const A: u16 = 1 << 4;
    /// global.KeyB: first ability (Tails: charged shot).
    pub const B: u16 = 1 << 5;
    /// global.KeyC: second ability (Eggman: tracker).
    pub const C: u16 = 1 << 6;
    /// global.KeyEm1, KeyEm2, KeyEm3: the taunts.
    pub const EMOTION1: u16 = 1 << 7;
    pub const EMOTION2: u16 = 1 << 8;
    pub const EMOTION3: u16 = 1 << 9;
    /// global.KeyIdle: the idle pose jumps to its fidget.
    pub const IDLE_POSE: u16 = 1 << 10;

    pub fn held(self, button: u16) -> bool {
        self.0 & button != 0
    }
}

/// A spring this player used. The original's springs were each client's own copies,
/// so one player's use does not hold a spring back for the others.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UsedSpring {
    pub spring: crate::core::world::InstanceId,
    /// alarm[0]: the pressed image shows until this runs out.
    pub frame_ticks: i32,
    /// alarm[1]: the spring launches this player again once this runs out.
    pub recharge_ticks: i32,
}

/// Where the eight obj_player_sensor* instances of the original stand. In the
/// original they are room-wide singletons; with several simulated players
/// each player needs its own.
#[derive(Clone, Copy, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Sensor {
    pub x: f64,
    pub y: f64,
    pub coll: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Player {
    pub character: Character,
    /// Which killer, when `character` is EXE.
    pub exe_character: ExeCharacter,
    /// Animation state number of the original (IDLE, WALK, ... per character).
    pub state: usize,
    pub sprite_index: SpriteId,
    pub image_index: f64,
    pub image_speed: f64,

    pub x: f64,
    pub y: f64,
    pub gspd: f64,
    pub xspd: f64,
    pub yspd: f64,
    /// Radians.
    pub angle: f64,
    pub image_xscale: f64,

    pub hp: i32,
    pub rings: i32,
    /// Ticks of post-damage invincibility left.
    pub hurttime: i32,
    /// 2 means demonized (corrupted after death).
    pub revival_times: i32,
    pub shocked_timer: i32,
    pub bounce_timer: i32,
    pub red_ring_timer: i32,
    pub dead_timer: i32,
    /// Ticks of the speed trail effect left.
    pub effect_time: i32,
    pub shards: i32,

    pub is_grounded: bool,
    pub is_jumping: bool,
    pub is_spinning: bool,
    pub is_looking_up: bool,
    pub is_looking_down: bool,
    pub just_jumped: bool,
    pub is_on_edge: bool,
    pub edge_dir: f64,
    pub is_boosting: bool,
    pub is_hurt: bool,
    pub is_dead: bool,
    pub emotion: bool,
    pub is_zipline: bool,
    pub is_attacking: bool,
    pub is_hiding: bool,
    /// Tails: flying, charging a shot, and the pose held after shooting.
    pub is_flying: bool,
    pub attack_charge: i32,
    pub attack_after: i32,
    pub attack_timer: i32,
    pub attack_k_dir: f64,
    pub recoil: f64,
    /// Counts down during flight, then down to -420 on the ground (flight recharge).
    pub fly_timer: i32,
    pub fly_grv: f64,
    /// Knuckles: gliding, stuck to a wall, glide recharge, glide duration, glide speed.
    pub is_gliding: bool,
    pub is_stuck: bool,
    pub glide_timer: i32,
    pub glide_timeout: i32,
    pub glide_xspd: f64,
    /// Eggman: double jump, shield and tracker timers (negative = ability in use).
    pub djump_recharge: i32,
    pub shield_recharge: i32,
    pub tracker_recharge: i32,
    /// Eggman: flat floor in front, a tracker can be placed.
    pub is_colliding: bool,
    /// Amy: big jump recharge and whether the big jump pose is running.
    pub hjump_timer: i32,
    pub is_hj: bool,
    /// Cream: rings recharge, rings spawn countdown, dash time left, flight time used, dash recharge.
    pub rings_timer: i32,
    pub rings_spawn: i32,
    pub dashing: i32,
    pub fly_timeout: i32,
    pub dash_timer: i32,
    /// Sally: shield time left, sliding and the slide speed.
    pub shield_timer: i32,
    pub is_sliding: bool,
    pub slide_speed: f64,
    /// EXE: invisibility timer (positive = invisible, negative = recharging), round won or lost.
    pub invis_timer: i32,
    pub won: bool,
    pub lost: bool,
    /// Chaos: dash direction, window in which a dash sticks to surfaces, stuck state,
    /// the burst after leaving a surface, arrows held on impact, liquid form.
    pub dash_dir_x: f64,
    pub dash_dir_y: f64,
    pub dash_wall_window: i32,
    pub stuck_timer: i32,
    pub stuck_dir: f64,
    pub stuck_dash_timer: i32,
    pub left_pressed: bool,
    pub right_pressed: bool,
    pub up_pressed: bool,
    /// Positive = liquid, negative = recharging.
    pub slime_timer: i32,
    /// The transformation animation has played; liquid poses are shown.
    pub slime_anim: bool,
    pub prev_grounded: bool,
    /// Exetior: black ring recharge, stomping, stomp landing handled.
    pub bring_timer: i32,
    pub is_stomping: bool,
    pub just_landed: bool,
    /// Exeller: clone ids by slot (NO_CLONE when empty), clone cooldown, clones placed.
    pub clones: [i32; 2],
    pub clone_timer: i32,
    pub clone_count: i32,
    /// Chaos and Exeller: pose after losing; the server picks one of two at random when spawning.
    pub lost_state: usize,
    /// Slowed by a tracker or a snowball; alarm[4] of the original ends it.
    pub is_slow: bool,
    pub slow_ticks: i32,

    pub can_move: bool,
    pub can_look_down: bool,
    pub can_look_up: bool,
    pub can_spin: bool,
    /// global.playerControls: false while the round has not started yet.
    pub controls_enabled: bool,
    /// instance_destroy(): the player left the level (escaped through the big ring, or
    /// left the server) and takes no further part in it.
    pub removed: bool,
    pub used_springs: Vec<UsedSpring>,
    /// inWater of obj_ghz_water, made on the player the first time it is read.
    pub in_water: bool,
    /// The zipline handles this player moved from their starts.
    pub ziplines: Vec<ZiplineHandle>,
    /// keyboard_key_release(KeyA): jump counts as up until it really is.
    pub ignore_jump_until_released: bool,
    /// obj_ycr_smokearea timer: ticks of breathing gas, negative before its first effect.
    pub gas_timer: i32,
    /// Fart Zone: this client's dummy timer, and the ticks left of the statue's curse (0: none).
    pub dummy_rest: i32,
    pub potato_ticks: i32,
    /// scr_move_basic: the screenful the player has been sitting in, and for how long.
    pub chunk: (f64, f64),
    pub chunk_ticks: i32,
    /// obj_player_warning is on screen and keeps slowing this player down.
    pub hiding_warning: bool,
    /// Ravine Mist: this client's bite timer of each slug it knows.
    pub slug_bites: Vec<(u16, i32)>,
    /// Echidna Ruins: ticks the controls stay reversed.
    pub controls_reversed: i32,
    /// Limp City: the eye this player looks through.
    pub watching_eye: Option<u8>,
    /// Torture Cave: whether a burning cloud touched this player this tick (collision) and
    /// the time to its next bite (timer).
    pub in_acid: bool,
    pub acid_timer: i32,
    /// DotDotDot: a side sensor is on a ladder (coll).
    pub on_ladder: bool,
    /// The Priceless Freedom lift carrying the player (pid of obj_pf_lift).
    pub riding_lift: Option<crate::core::world::InstanceId>,
    /// The player opened the pause menu (CLIENT_ROUND_PAUSE). The simulation ignores it:
    /// a paused player keeps standing in the round, others are shown that they are away.
    pub paused: bool,
    /// isInactive: the server has heard nothing from this player's client for a while
    /// (states/round.rs INACTIVE_AFTER_SECONDS).
    pub inactive: bool,

    /// Current acceleration and top speed: the character's values, lowered while slowed.
    pub acc: f64,
    pub max_h_speed: f64,

    pub sensor_bl: Sensor,
    pub sensor_br: Sensor,
    pub sensor_l: Sensor,
    pub sensor_r: Sensor,
    pub sensor_tl: Sensor,
    pub sensor_tr: Sensor,
    pub sensor_al: Sensor,
    pub sensor_ar: Sensor,

    buttons: Buttons,
    previous_buttons: Buttons,
}

/// Tails' flight is ready from the start: flyTimer begins at the recharge bottom.
fn flight_ready_timer(cfg: &GameplayConfig) -> i32 {
    -config::ticks(cfg.tails.flight_recharge_seconds)
}

/// obj_exe and its kin: hp = 10000. EXE takes no damage (scr_player_hurt leaves it out,
/// it can only be stunned); the number only keeps every "still alive" check (hp > 0)
/// true for it, so it is not a setting.
const EXE_HP: i32 = 10000;

impl Player {
    /// Create event values of the character's object.
    pub fn new(character: Character, x: f64, y: f64, cfg: &GameplayConfig) -> Player {
        let movement = movement_config(character, cfg);
        let mut player = Player::with_speeds(x, y, movement.acceleration_per_tick, movement.max_speed_per_tick);
        player.character = character;
        player.hp = cfg.hurt.max_hp;
        player.can_spin = !matches!(character, Character::Eggman | Character::Amy | Character::Sally);
        // Tails starts with flight ready (bottom of its recharge); Cream's timer starts at 0.
        player.fly_timer = if character == Character::Tails { flight_ready_timer(cfg) } else { 0 };
        player.sprite_index = player.animation_idle_sprite();
        player
    }

    /// obj_chaos and obj_exeller Create: lostState = choose(LOST, LOST2). The caller
    /// rolls the dice (`second`), so the simulation itself stays repeatable.
    pub fn pick_lost_pose(&mut self, second: bool) {
        self.lost_state = match (self.exe_character, second) {
            (ExeCharacter::Chaos, true) => chaos::CHAOS_LOST2,
            (ExeCharacter::Chaos, false) => chaos::CHAOS_LOST,
            (ExeCharacter::Exeller, true) => exeller::EXELLER_LOST2,
            (ExeCharacter::Exeller, false) => exeller::EXELLER_LOST,
            _ => self.lost_state,
        };
    }

    /// Create event of an EXE object (obj_exe, obj_chaos, ...).
    pub fn new_exe(exe_character: ExeCharacter, x: f64, y: f64, cfg: &GameplayConfig) -> Player {
        let mut player = Player::new(Character::Exe, x, y, cfg);
        player.exe_character = exe_character;
        // Movement values depend on which EXE it is, so they are read again.
        let movement = player.movement(cfg);
        player.acc = movement.acceleration_per_tick;
        player.max_h_speed = movement.max_speed_per_tick;
        player.hp = EXE_HP;
        match exe_character {
            ExeCharacter::Chaos => {
                player.slime_timer = -config::ticks(cfg.chaos.liquid_recharge_seconds);
            }
            ExeCharacter::Exetior => {}
            ExeCharacter::Exeller => {
                player.lost_state = exeller::EXELLER_LOST;
            }
            ExeCharacter::Original => {
                player.invis_timer = -config::ticks(cfg.exe.invisibility_recharge_seconds);
            }
        }
        player.sprite_index = player.animation_idle_sprite();
        player
    }

    /// The character's movement numbers from the gameplay config.
    pub fn movement<'a>(&self, cfg: &'a GameplayConfig) -> &'a Movement {
        match (self.character, self.exe_character) {
            (Character::Exe, ExeCharacter::Chaos) => &cfg.chaos.movement,
            (Character::Exe, ExeCharacter::Exetior) => &cfg.exetior.movement,
            (Character::Exe, ExeCharacter::Exeller) => &cfg.exeller.movement,
            (Character::Exe, ExeCharacter::Original) => &cfg.exe.movement,
            _ => movement_config(self.character, cfg),
        }
    }

    fn with_speeds(x: f64, y: f64, acc: f64, max_h_speed: f64) -> Player {
        Player {
            character: Character::Tails,
            exe_character: ExeCharacter::Original,
            state: animation::IDLE,
            sprite_index: sprite::SPR_TAILS_IDLE,
            image_index: 0.0,
            image_speed: 1.0,
            x,
            y,
            gspd: 0.0,
            xspd: 0.0,
            yspd: 0.0,
            angle: 0.0,
            image_xscale: 1.0,
            // Player::new sets the health the config gives a survivor; EXE its own.
            hp: 0,
            rings: 0,
            hurttime: 0,
            revival_times: 0,
            shocked_timer: 0,
            bounce_timer: 0,
            red_ring_timer: 0,
            // The Create event assigns 31 and then 0; the second assignment wins.
            dead_timer: 0,
            effect_time: 0,
            shards: 0,
            is_grounded: false,
            is_jumping: false,
            is_spinning: false,
            is_looking_up: false,
            is_looking_down: false,
            just_jumped: false,
            is_on_edge: false,
            edge_dir: 1.0,
            is_boosting: false,
            is_hurt: false,
            is_dead: false,
            emotion: false,
            is_zipline: false,
            is_attacking: false,
            is_hiding: false,
            is_flying: false,
            attack_charge: 0,
            attack_after: 0,
            attack_timer: 0,
            attack_k_dir: 0.0,
            recoil: 0.0,
            fly_timer: -420,
            fly_grv: 0.0,
            is_gliding: false,
            is_stuck: false,
            glide_timer: 0,
            glide_timeout: 0,
            glide_xspd: 0.0,
            djump_recharge: 0,
            shield_recharge: 0,
            tracker_recharge: 0,
            is_colliding: false,
            hjump_timer: 0,
            is_hj: false,
            rings_timer: 0,
            rings_spawn: 0,
            dashing: 0,
            fly_timeout: 0,
            dash_timer: 0,
            shield_timer: 0,
            is_sliding: false,
            slide_speed: 5.0,
            invis_timer: 0,
            won: false,
            lost: false,
            dash_dir_x: 0.0,
            dash_dir_y: 0.0,
            dash_wall_window: 0,
            stuck_timer: 0,
            stuck_dir: 0.0,
            stuck_dash_timer: 0,
            left_pressed: false,
            right_pressed: false,
            up_pressed: false,
            slime_timer: 0,
            slime_anim: false,
            prev_grounded: true,
            bring_timer: 0,
            is_stomping: false,
            just_landed: false,
            clones: [NO_CLONE; 2],
            clone_timer: 0,
            clone_count: 0,
            lost_state: exe::EXE_LOST,
            is_slow: false,
            slow_ticks: -1,
            can_move: true,
            can_look_down: true,
            can_look_up: true,
            can_spin: true,
            controls_enabled: true,
            removed: false,
            used_springs: Vec::new(),
            in_water: false,
            ziplines: Vec::new(),
            ignore_jump_until_released: false,
            gas_timer: 0,
            dummy_rest: 0,
            potato_ticks: 0,
            chunk: (CHUNK_UNSET, CHUNK_UNSET),
            chunk_ticks: 0,
            hiding_warning: false,
            slug_bites: Vec::new(),
            controls_reversed: 0,
            watching_eye: None,
            in_acid: false,
            acid_timer: 0,
            on_ladder: false,
            riding_lift: None,
            paused: false,
            inactive: false,
            acc,
            max_h_speed,
            sensor_bl: Sensor::default(),
            sensor_br: Sensor::default(),
            sensor_l: Sensor::default(),
            sensor_r: Sensor::default(),
            sensor_tl: Sensor::default(),
            sensor_tr: Sensor::default(),
            sensor_al: Sensor::default(),
            sensor_ar: Sensor::default(),
            buttons: Buttons::default(),
            previous_buttons: Buttons::default(),
        }
    }

    /// One game tick in GameMaker event order: image update (with Animation End),
    /// Step, End Step, then the Pre-Draw/Draw logic that affects the game state.
    pub fn tick(&mut self, world: &World, buttons: Buttons, events: &mut Vec<SimEvent>) {
        let sprites = world.sprites.clone();
        let cfg = world.config.clone();
        self.advance_animation(&sprites, &cfg);
        self.step(world, &cfg, buttons, events);
        // Room instances' End Step comes before the character's.
        self.zipline_end_steps(world, &cfg, events);
        self.touch_falling_stalactites(world, &cfg, events);
        self.leave_walls_after_switch(world);
        self.bitten_by_acid(world, &cfg, events);
        self.crush_under_doors(world, &cfg, events);
        self.ride_lift(world, &cfg);
        self.choose_state(&cfg, events);
        // obj_pf_lift Draw events: a rider shows its hurt pose.
        if self.riding_lift.is_some() {
            self.state = animation::HURT;
            self.is_hurt = false;
        }
        self.apply_state_animation(&sprites, &cfg, world.room);
        // Room instances' Draw events that act on the player.
        self.use_crystals(world, events);
        self.hold_the_curse(events);
        self.slow_ladder_walk(&cfg);
        // Draw_0 "hakc" of obj_chaos, obj_exetior and obj_exeller: a dead one stops sliding.
        // They never die; kept for parity.
        let has_dead_freeze = matches!(self.exe_character, ExeCharacter::Chaos | ExeCharacter::Exetior | ExeCharacter::Exeller);
        if self.character == Character::Exe && has_dead_freeze && self.is_dead {
            self.xspd = 0.0;
            self.gspd = 0.0;
        }
        self.update_trail(events);
    }

    /// Step event of obj_tails (the survivor part shared by every character).
    fn step(&mut self, world: &World, cfg: &GameplayConfig, buttons: Buttons, events: &mut Vec<SimEvent>) {
        self.previous_buttons = self.buttons;
        let buttons = self.reverse_controls(buttons);
        self.buttons = buttons;
        if self.ignore_jump_until_released {
            if buttons.held(Buttons::A) {
                self.buttons.0 &= !Buttons::A;
            } else {
                self.ignore_jump_until_released = false;
            }
        }
        // Begin Step of obj_pf_lift and obj_act9_wall.
        self.ride_lift(world, cfg);
        if world.room == Some(crate::core::rooms::ids::RoomId::Act9) {
            self.meet_act9_walls(world, cfg, events);
        }
        // Alarms run before Step events.
        self.count_down_springs();
        self.room_object_steps(world, cfg, events);

        let held_in_place = self.ability_holds_in_place();
        if held_in_place {
            // While an ability holds the character in place, only the ability runs.
            self.special(world, cfg, events);
        } else if self.shocked_timer <= 0 {
            self.move_basic(world, cfg, events);
        } else {
            self.stay_shocked(cfg);
        }
        self.count_down_slowdown(cfg);
        // obj_exe Step_0: after winning or losing EXE stands still.
        if self.character == Character::Exe && (self.won || self.lost) {
            self.gspd = 0.0;
            self.xspd = 0.0;
            self.angle = 0.0;
        }
        if self.is_exe(ExeCharacter::Chaos) || self.is_exe(ExeCharacter::Exetior) {
            self.prev_grounded = self.is_grounded;
        }
        let interrupted = self.is_hurt || self.hp <= 0 || self.is_dead || self.shocked_timer > 0 || self.is_zipline;
        if self.character == Character::Tails && interrupted {
            self.recoil = 0.0;
            self.attack_charge = 0;
            self.attack_after = 0;
        }

        self.collision_basic(world, cfg, events);
        self.count_down_timers(events);
    }

    /// specialFunc of the original: the character's own abilities.
    fn special(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        match self.character {
            Character::Tails => self.tails_special(cfg, events),
            Character::Knux => self.knux_special(cfg, events),
            Character::Eggman => self.egg_special(world, cfg, events),
            Character::Amy => self.amy_special(cfg, events),
            Character::Cream => self.cream_special(world, cfg, events),
            Character::Sally => self.sally_special(world, cfg, events),
            Character::Exe => match self.exe_character {
                ExeCharacter::Chaos => self.chaos_special(cfg, events),
                ExeCharacter::Exetior => self.exetior_special(world, cfg, events),
                ExeCharacter::Exeller => self.exeller_special(cfg, events),
                ExeCharacter::Original => self.exe_special(cfg, events),
            },
        }
    }

    /// scr_player_slow: lower acceleration and top speed by `percent` for a while.
    pub fn slow_down(&mut self, cfg: &GameplayConfig, percent: f64, seconds: f64) {
        let movement = self.movement(cfg);
        self.is_slow = true;
        self.acc = movement.acceleration_per_tick * (100.0 - percent) / 100.0;
        self.max_h_speed = movement.max_speed_per_tick * (100.0 - percent) / 100.0;
        self.slow_ticks = config::ticks(seconds);
    }

    /// alarm[4] of every character object: back to normal speed.
    // ponytail: alarm counted inside Step instead of GameMaker's alarm phase (before Step), one tick later at most
    fn count_down_slowdown(&mut self, cfg: &GameplayConfig) {
        if self.slow_ticks <= 0 {
            return;
        }
        self.slow_ticks -= 1;
        if self.slow_ticks == 0 {
            self.slow_ticks = -1;
            if self.is_exe(ExeCharacter::Chaos) {
                self.chaos_end_slowdown(cfg);
                return;
            }
            let movement = self.movement(cfg);
            self.acc = movement.acceleration_per_tick;
            self.max_h_speed = movement.max_speed_per_tick;
            self.is_slow = false;
        }
    }

    pub fn is_exe(&self, exe_character: ExeCharacter) -> bool {
        self.character == Character::Exe && self.exe_character == exe_character
    }

    /// Step_0 "if(canMove) ... else specialFunc()": while an ability holds the character
    /// (Tails charging, Chaos stuck to a wall) only the ability runs. Chaos still takes
    /// the stun branch when stunned. The other characters never check canMove here.
    fn ability_holds_in_place(&self) -> bool {
        if self.can_move {
            return false;
        }
        match self.character {
            Character::Tails => true,
            Character::Exe if matches!(self.exe_character, ExeCharacter::Chaos | ExeCharacter::Exetior) => self.shocked_timer <= 0,
            _ => false,
        }
    }

    /// Abilities cannot be ready sooner than this while hiding or under a red ring.
    /// The original applies both checks one after another; the larger delay always wins.
    pub(super) fn ability_delay_ticks(&self, cfg: &GameplayConfig) -> Option<i32> {
        let hiding = self.is_hiding.then(|| config::ticks(cfg.physics.hiding_ability_delay_seconds));
        let red_ring = (self.red_ring_timer > 0).then_some(crate::core::config::original_ticks(cfg.physics.red_ring_ability_delay_ticks));
        hiding.max(red_ring)
    }

    /// place_meeting(x, y, object) with the player's current sprite as mask.
    pub fn meeting_at(&self, world: &World, x: f64, y: f64, object: crate::core::objects::ids::ObjectId) -> bool {
        self.instance_place(world, x, y, object).is_some()
    }

    /// instance_place(x, y, object) with the player's current sprite as mask.
    // ponytail: bounding boxes only, precise masks of the other object ignored
    pub fn instance_place(&self, world: &World, x: f64, y: f64, object: crate::core::objects::ids::ObjectId) -> Option<crate::core::world::InstanceId> {
        let body = crate::core::collision::sprite_bbox(world.sprites.get(self.sprite_index), x, y, self.image_xscale, 1.0, 0.0);
        world.ids_of(object).find(|&id| world.bbox(id).is_some_and(|other| other.overlaps(&body)))
    }

    /// Stunned: slide to a stop, ignore input, stay invincible.
    fn stay_shocked(&mut self, cfg: &GameplayConfig) {
        match self.character {
            Character::Tails => self.can_move = true,
            Character::Cream => self.cream_stop_abilities(),
            Character::Exe if self.exe_character == ExeCharacter::Chaos => self.chaos_stop_abilities(),
            Character::Exe if self.exe_character == ExeCharacter::Exetior => self.exetior_stop_stomp(),
            _ => {}
        }
        self.is_boosting = false;
        self.is_looking_down = false;
        self.is_looking_up = false;
        self.is_attacking = false;
        self.is_jumping = false;
        self.gspd -= self.gspd.abs().min(self.acc * config::step()) * gm_sign(self.gspd);
        self.xspd -= self.xspd.abs().min(self.acc * config::step()) * gm_sign(self.xspd);
        self.shocked_timer -= 1;
        self.hurttime = config::ticks(cfg.physics.shocked_invincibility_seconds);
    }

    fn count_down_timers(&mut self, events: &mut Vec<SimEvent>) {
        if self.red_ring_timer > 0 {
            if self.red_ring_timer == 1 {
                events.push(SimEvent::RedRingEnded);
            }
            self.red_ring_timer -= 1;
        }
        if self.hurttime > 0 {
            self.hurttime -= 1;
        }
    }

    /// revivalTimes of 2: died once and came back corrupted, playing for the EXE side.
    pub fn is_demonized(&self) -> bool {
        self.revival_times >= 2
    }

    pub fn is_killers_side(&self) -> bool {
        on_killers_side(self.character, self.revival_times)
    }

    /// keyboard_check, gated by global.playerControls like every call in the original.
    fn held(&self, button: u16) -> bool {
        self.controls_enabled && self.buttons.held(button)
    }

    /// keyboard_check_pressed: down this tick, up the previous one.
    fn pressed(&self, button: u16) -> bool {
        self.held(button) && !self.previous_buttons.held(button)
    }

    /// keyboard_check_pressed used without the global.playerControls gate.
    fn pressed_raw(&self, button: u16) -> bool {
        self.buttons.held(button) && !self.previous_buttons.held(button)
    }

    /// keyboard_check_released (never gated in the original).
    fn released(&self, button: u16) -> bool {
        !self.buttons.held(button) && self.previous_buttons.held(button)
    }
}

fn movement_config(character: Character, cfg: &GameplayConfig) -> &Movement {
    match character {
        Character::Knux => &cfg.knuckles.movement,
        Character::Eggman => &cfg.eggman.movement,
        Character::Amy => &cfg.amy.movement,
        Character::Cream => &cfg.cream.movement,
        Character::Sally => &cfg.sally.movement,
        // Player::movement picks the section of the specific EXE.
        Character::Exe => &cfg.exe.movement,
        // ponytail: other characters' sections come with their objects
        _ => &cfg.tails.movement,
    }
}

/// GameMaker sign(): -1, 0 or 1.
pub fn gm_sign(value: f64) -> f64 {
    if value > 0.0 {
        1.0
    } else if value < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// GameMaker point_direction: degrees, counter-clockwise, y axis pointing down.
pub fn point_direction(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    (-(y2 - y1)).atan2(x2 - x1).to_degrees().rem_euclid(360.0)
}

/// The EXE and the demons play against the survivors. A free function, because a
/// snapshot's health row (HealthView) knows the same two things about a player.
pub fn on_killers_side(character: Character, revival_times: i32) -> bool {
    character == Character::Exe || revival_times >= 2
}
