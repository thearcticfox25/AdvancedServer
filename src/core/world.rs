//! Instances placed in a room and the GameMaker-style queries over them.

use crate::core::resources::sprites::Sprites;
use crate::core::config::{GameplayConfig, Springs};
use crate::core::resources::SpriteId;
use crate::core::objects::ids::{ObjectId, ALL_OBJECTS};
use crate::core::rooms::ids::RoomId;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use crate::format::placement::{self, GmPlacement, SpriteFrame, TiledPlacement};
use crate::format::tiled::{self, Layer};

pub type InstanceId = usize;

/// The built-in variables every GameMaker instance has, plus the few object
/// variables the physics reads from other instances.
#[derive(Clone, Debug)]
pub struct Instance {
    pub object: ObjectId,
    pub x: f64,
    pub y: f64,
    pub sprite_index: Option<SpriteId>,
    pub mask_index: Option<SpriteId>,
    pub image_index: f64,
    pub image_speed: f64,
    pub image_xscale: f64,
    pub image_yscale: f64,
    pub image_angle: f64,
    pub visible: bool,
    pub image_alpha: f64,
    /// GameMaker colour 0xBBGGRR.
    pub image_blend: u32,
    pub vars: ObjectVars,
    /// obj_floor_parent.canStuck: whether Knuckles and Chaos can stick to this wall.
    pub can_stuck: bool,
}

/// Variables an object declares in its Create event, for objects other code reads.
#[derive(Clone, Debug, PartialEq)]
pub enum ObjectVars {
    None,
    /// obj_spring_parent and children: launch speed.
    Spring { xdir: f64, ydir: f64 },
    /// obj_abyss: whether EXE and demonized players who fall in are stunned too.
    Abyss { should_stun: bool },
    /// obj_ghz_water: shocks whoever stands in it (the server's thunder sets it).
    Water { electro: bool },
    /// obj_aiz_zipline_start and _end, and a zipline before it found them: the line's gid.
    ZiplineGroup { gid: i64 },
    /// obj_aiz_zipline: the start and end of its line.
    Zipline { start: InstanceId, end: InstanceId },
    /// obj_soundemitter: stype, what sounds from it (SOUNDEMT_MOVINGSPIKE = 0, ...).
    SoundEmitter { kind: i64 },
    /// obj_ycr_smokearea: which gas fills it, and whether it does now.
    SmokeArea { nid: u8, gassed: bool },
    /// obj_kaf_speedbox: its number (by creation order) and whether it is broken.
    Monitor { nid: u8, broken: bool },
    /// obj_hd_door and obj_hd_door2: sX, sY (where the map placed it, closed), state, and
    /// whether it moved shut a pixel on the last change (it crushes then).
    Door { closed_x: f64, closed_y: f64, open: bool, closing: bool },
    /// obj_pf_lift: where it starts, where it stops (the map's "top"), whether it carries someone.
    Lift { start_y: f64, top_y: f64, carrying: bool },
    /// obj_hd_crystal: state (the doors are open) and canUse.
    Crystal { lit: bool, usable: bool },
    /// obj_nap_iceblock: its number.
    IceBlock { nid: u8 },
    /// obj_nap_snowball_waypoint: the snowball whose path it is on, its place on it, and
    /// how fast the snowball rolls the leg from here (None: the rules' speed).
    SnowballWaypoint { nid: u8, wid: u8, roll_speed: Option<f64>, roll_frames: Option<f64> },
    /// obj_vv_lavacolumn and obj_marijuna_lavaplatform: its number, where the map placed
    /// it, and state (2 rising, 3 up high, 4 falling).
    LavaColumn { nid: u8, start_y: f64, state: u8 },
    /// obj_marijuna_crystal: activated, and how much it glows (fade).
    MjCrystal { lit: bool, glow: f64 },
    /// obj_marijuna_judger: the server's state (0 waiting, 1 ready, 2 firing), which the
    /// client read as a bool.
    Judger { state: u8 },
    /// obj_vv_vase: its number.
    Vase { nid: u8 },
    /// obj_darktower_ball: sY, and how it swings (dir, dist).
    DarkTowerBall { start_y: f64, dir: f64, dist: f64 },
    /// obj_darktower_stalactite: its number, where it hangs, and whether it falls (damage).
    Stalactite { nid: u8, start_y: f64, falling: bool },
    /// obj_np_controller: tid, the stage everyone is on, whether the next one's shadow
    /// shows (the depth 90 tiles' isVis), and whether the stage switched this tick (the
    /// pushers' active).
    NotPerfectController { stage: u8, warning: bool, switched: bool },
    /// obj_np_teleporn: the stage whose area it is (tid), and where it puts whoever is
    /// outside it (pX, pY).
    StageArea { stage: u8, back_x: f64, back_y: f64 },
    /// obj_act9_wall: which wall it is (0 the ceiling, 1 the left side, 2 the right).
    Act9Wall { nid: u8 },
    /// obj_weed_lantern and its children: its number and whether it is lit (active).
    Lantern { nid: u8, lit: bool },
    /// obj_fart_dummy: where the map put it, and whether it ignores hits for now (timer).
    Dummy { start_x: f64, resting: bool },
    /// obj_limpcity_echain1 and 2: resting (0), warning (1) or shocking (2).
    ElectricChain { state: u8 },
    /// obj_limpcity_eyechain and 2: sid, the chain or eye hanging on it; tid, the chain it
    /// hangs on (-1: none); range; and where the map put it (oX, oY).
    EyeChain { sid: i64, tid: i64, range: f64, map_x: f64, map_y: f64 },
    /// obj_limpcity_eyeA (with the eyes it shows) and obj_limpcity_eyeB: nid, tid, and
    /// whether someone looks through it, and its charge.
    Eye { nid: u8, tid: i64, targets: Option<(u8, u8)>, used: bool, charge: u8, map_x: f64, map_y: f64 },
    /// obj_abadon_cloud: its acid group, and whether it burns (damage).
    AcidCloud { nid: u8, burning: bool },
    /// obj_nap_snowball: the path it rolls down.
    Snowball { nid: u8 },
}

/// A snowball stands at the bottom middle of its waypoints' 64 px squares.
pub const SNOWBALL_UNDER_WAYPOINT: (f64, f64) = (32.0, 64.0);

pub const C_WHITE: u32 = 0xFFFFFF;
pub const C_GREY: u32 = 0x808080;

pub struct World {
    pub sprites: Arc<Sprites>,
    /// Gameplay numbers of this round (the server's, on clients).
    pub config: Arc<GameplayConfig>,
    pub instances: Vec<Instance>,
    pub room_width: f64,
    pub room_height: f64,
    pub room: Option<RoomId>,
    /// Round timer in ticks, counting down. Set by the server's game rules.
    pub timer_ticks: i64,
    /// obj_netclient.isServerReady: players may move.
    pub round_started: bool,
    /// obj_netclient.gameEnds: nobody can be hurt any more.
    pub game_ends: bool,
    pub black_rings: Vec<crate::core::black_ring::BlackRing>,
    /// The rings of the round (obj_ring, obj_redring): put out by whoever owns the
    /// level (the server, or the game itself in Singleplayer) and drawn by everyone.
    pub rings: Vec<crate::core::rings::MapRing>,
    /// Ravine Mist's shards and slugs, as the level shows them.
    pub ravine_mist: Option<crate::core::level::RavineMistView>,
}

/// `object_is_ancestor` plus equality: true when `object` is `wanted` or inherits from it.
pub fn is_a(object: ObjectId, wanted: ObjectId) -> bool {
    let mut current = Some(object);
    while let Some(candidate) = current {
        if candidate == wanted {
            return true;
        }
        current = candidate.info().parent;
    }
    false
}

impl World {
    pub fn empty(sprites: Arc<Sprites>, config: Arc<GameplayConfig>) -> World {
        World {
            sprites,
            config,
            instances: Vec::new(),
            room_width: 0.0,
            room_height: 0.0,
            room: None,
            timer_ticks: 0,
            round_started: false,
            game_ends: false,
            black_rings: Vec::new(),
            rings: Vec::new(),
            ravine_mist: None,
        }
    }

    /// Creates an instance with the object's editor defaults and runs its Create event.
    pub fn instance_create(&mut self, x: f64, y: f64, object: ObjectId) -> InstanceId {
        let info = object.info();
        let instance = Instance {
            object,
            x,
            y,
            sprite_index: info.sprite,
            mask_index: info.mask,
            image_index: 0.0,
            image_speed: 1.0,
            image_xscale: 1.0,
            image_yscale: 1.0,
            image_angle: 0.0,
            visible: info.visible,
            image_alpha: 1.0,
            image_blend: C_WHITE,
            vars: ObjectVars::None,
            can_stuck: create_can_stuck(object),
        };
        self.instances.push(instance);
        let id = self.instances.len() - 1;
        run_create_event(&mut self.instances[id], &self.config.springs);
        id
    }

    /// UpdateImages of the runner, first thing in a step: image_index moves on by
    /// image_speed times the sprite's speed and wraps around.
    // ponytail: Animation End events of placed objects are not run; add them with the first object that has one
    pub fn animate(&mut self) {
        for instance in &mut self.instances {
            let Some(sprite) = instance.sprite_index else { continue };
            let meta = self.sprites.get(sprite);
            let frame_count = meta.frame_count as f64;
            instance.image_index += instance.image_speed * meta.fps as f64 / crate::core::config::ticks_per_second();
            if frame_count > 0.0 && !(0.0..frame_count).contains(&instance.image_index) {
                instance.image_index = instance.image_index.rem_euclid(frame_count);
            }
        }
    }

    /// instance_nearest(x, y, object): by distance to the instance position.
    pub fn instance_nearest(&self, x: f64, y: f64, object: ObjectId) -> Option<InstanceId> {
        let distance = |id: InstanceId| (self.instances[id].x - x).hypot(self.instances[id].y - y);
        self.ids_of(object).min_by(|&a, &b| distance(a).total_cmp(&distance(b)))
    }

    pub fn ids_of(&self, object: ObjectId) -> impl Iterator<Item = InstanceId> + '_ {
        (0..self.instances.len()).filter(move |&id| is_a(self.instances[id].object, object))
    }

    /// Loads the instances of the room's Tiled map (`<maps>/<name>.tmj`), in creation order.
    pub fn load_room(&mut self, maps_folder: &Path, room: RoomId) -> Result<()> {
        let map_path = &maps_folder.join(format!("{}.tmj", room.file_name()));
        let map = tiled::load_map(map_path)?;
        self.room = Some(room);
        let tiles = TileLookup::load(map_path, &map)?;
        self.room_width = map.pixel_width() as f64;
        self.room_height = map.pixel_height() as f64;

        let mut objects = Vec::new();
        collect_instance_objects(&map.layers, &mut objects);
        // Tiled object ids follow the GameMaker creation order (see gm_convert rooms.rs).
        objects.sort_by_key(|object| object.id);
        for object in objects {
            self.create_from_map(object, &tiles).with_context(|| format!("map object {}", object.id))?;
        }
        self.connect_ziplines();
        self.create_snowballs();
        crate::core::level::hang_eyes(self, 0.0);
        if room == RoomId::Dartower {
            // SERVER_DTTAILSDOLL_STATE made it on the first news of it.
            let doll = self.instance_create(0.0, 0.0, ObjectId::DarktowerTailsdoll);
            self.instances[doll].visible = false;
            self.instances[doll].image_speed = 0.0;
        }
        // SERVER_KAFMONITOR_STATE 0 numbers the monitors in instance order.
        let monitors: Vec<InstanceId> = self.ids_of(ObjectId::KafSpeedbox).collect();
        for (nid, monitor) in monitors.into_iter().enumerate() {
            self.instances[monitor].vars = ObjectVars::Monitor { nid: nid as u8, broken: false };
        }
        Ok(())
    }

    /// SERVER_NAPBALL_STATE 0 made a snowball at its path's first waypoint each time it
    /// started rolling; here each path has one from the start, hidden while it is not rolling.
    fn create_snowballs(&mut self) {
        let starts: Vec<(f64, f64, u8)> = self
            .ids_of(ObjectId::NapSnowballWaypoint)
            .filter_map(|id| match self.instances[id].vars {
                ObjectVars::SnowballWaypoint { nid, wid: 0, .. } => Some((self.instances[id].x, self.instances[id].y, nid)),
                _ => None,
            })
            .collect();
        for (x, y, nid) in starts {
            let snowball = self.instance_create(x + SNOWBALL_UNDER_WAYPOINT.0, y + SNOWBALL_UNDER_WAYPOINT.1, ObjectId::NapSnowball);
            let instance = &mut self.instances[snowball];
            instance.vars = ObjectVars::Snowball { nid };
            instance.visible = false;
            instance.image_speed = 0.0;
        }
    }

    /// obj_aiz_zipline End Step: the start and end with the zipline's gid (the last of each).
    fn connect_ziplines(&mut self) {
        let group = |world: &World, id: InstanceId| match world.instances[id].vars {
            ObjectVars::ZiplineGroup { gid } => Some(gid),
            _ => None,
        };
        let ziplines: Vec<InstanceId> = self.ids_of(ObjectId::AizZipline).collect();
        for zipline in ziplines {
            let gid = group(self, zipline);
            let start = self.ids_of(ObjectId::AizZiplineStart).filter(|&id| group(self, id) == gid).last();
            let end = self.ids_of(ObjectId::AizZiplineEnd).filter(|&id| group(self, id) == gid).last();
            if let (Some(start), Some(end)) = (start, end) {
                self.instances[zipline].vars = ObjectVars::Zipline { start, end };
            }
        }
    }

    fn create_from_map(&mut self, object: &tiled::Object, tiles: &TileLookup) -> Result<()> {
        let tile = object.tile_gid().and_then(|gid| tiles.get(gid));
        let object_id = map_object_class(object, tile)?;
        let origin = object_id.info().sprite.map_or((0.0, 0.0), |sprite| {
            let meta = self.sprites.get(sprite);
            (meta.origin_x, meta.origin_y)
        });
        let gm = map_object_placement(object, tile, origin);

        let id = self.instance_create(gm.x, gm.y, object_id);
        let instance = &mut self.instances[id];
        instance.image_xscale = gm.scale_x;
        instance.image_yscale = gm.scale_y;
        instance.image_angle = gm.rotation;
        if let Some(index) = tiled::find_property(&object.properties, "image_index").and_then(|value| value.as_f64()) {
            instance.image_index = index;
        }
        if let Some(speed) = tiled::find_property(&object.properties, "image_speed").and_then(|value| value.as_f64()) {
            instance.image_speed = speed;
        }
        if matches!(object_id, ObjectId::AizZipline | ObjectId::AizZiplineStart | ObjectId::AizZiplineEnd) {
            let gid = tiled::find_property(&object.properties, "gid").and_then(|value| value.as_f64());
            instance.vars = ObjectVars::ZiplineGroup { gid: gid.unwrap_or(0.0) as i64 };
        }
        if matches!(object_id, ObjectId::HdDoor | ObjectId::HdDoor2) {
            instance.vars = ObjectVars::Door { closed_x: instance.x, closed_y: instance.y, open: false, closing: false };
        }
        if object_id == ObjectId::PfLift {
            let top_y = tiled::find_property(&object.properties, "top").and_then(|value| value.as_f64()).unwrap_or(instance.y);
            instance.vars = ObjectVars::Lift { start_y: instance.y, top_y, carrying: false };
            // obj_pf_lift Create: faded out at first.
            instance.image_alpha = 0.0;
        }
        if object_id == ObjectId::VvLavacolumn {
            let nid = tiled::find_property(&object.properties, "nid").and_then(|value| value.as_f64());
            instance.vars = ObjectVars::LavaColumn { nid: nid.unwrap_or(0.0) as u8, start_y: instance.y, state: 0 };
            // obj_vv_lavacolumn Step: a high one is taller.
            if tiled::find_property(&object.properties, "isHigh").and_then(|value| value.as_bool()) == Some(true) {
                instance.sprite_index = Some(crate::core::resources::names::sprite::SPR_VV_LAVACOLUMN2);
            }
        }
        if is_a(object_id, ObjectId::WeedLantern) {
            let nid = tiled::find_property(&object.properties, "nid").and_then(|value| value.as_f64());
            instance.vars = ObjectVars::Lantern { nid: nid.unwrap_or(0.0) as u8, lit: false };
            // Its frame is chosen by its Step.
            instance.image_speed = 0.0;
        }
        if object_id == ObjectId::FartDummy {
            instance.vars = ObjectVars::Dummy { start_x: instance.x, resting: false };
        }
        if object_id == ObjectId::Act9Wall {
            let nid = tiled::find_property(&object.properties, "nid").and_then(|value| value.as_f64());
            instance.vars = ObjectVars::Act9Wall { nid: nid.unwrap_or(0.0) as u8 };
        }
        if object_id == ObjectId::NpController {
            instance.vars = ObjectVars::NotPerfectController { stage: 0, warning: false, switched: false };
        }
        if object_id == ObjectId::NpTeleporn {
            let number = |name| tiled::find_property(&object.properties, name).and_then(|value| value.as_f64()).unwrap_or(0.0);
            instance.vars = ObjectVars::StageArea { stage: number("tid") as u8, back_x: number("pX"), back_y: number("pY") };
        }
        if object_id == ObjectId::DarktowerBall {
            let number = |name| tiled::find_property(&object.properties, name).and_then(|value| value.as_f64());
            instance.vars = ObjectVars::DarkTowerBall { start_y: instance.y, dir: number("dir").unwrap_or(1.0), dist: number("dist").unwrap_or(100.0) };
        }
        if object_id == ObjectId::DarktowerStalactite {
            let nid = tiled::find_property(&object.properties, "nid").and_then(|value| value.as_f64());
            instance.vars = ObjectVars::Stalactite { nid: nid.unwrap_or(0.0) as u8, start_y: instance.y, falling: false };
        }
        if is_a(object_id, ObjectId::LimpcityEchain1) {
            instance.vars = ObjectVars::ElectricChain { state: 0 };
        }
        let number = |name: &str| tiled::find_property(&object.properties, name).and_then(|value| value.as_f64());
        if matches!(object_id, ObjectId::LimpcityEyechain | ObjectId::LimpcityEyechain2) {
            instance.vars = ObjectVars::EyeChain {
                sid: number("sid").unwrap_or(-1.0) as i64,
                tid: number("tid").unwrap_or(-1.0) as i64,
                range: number("range").unwrap_or(1.0),
                map_x: instance.x,
                map_y: instance.y,
            };
        }
        if matches!(object_id, ObjectId::LimpcityEyea | ObjectId::LimpcityEyeb) {
            let targets = (object_id == ObjectId::LimpcityEyea).then(|| (number("target1").unwrap_or(0.0) as u8, number("target2").unwrap_or(0.0) as u8));
            instance.vars = ObjectVars::Eye {
                nid: number("nid").unwrap_or(0.0) as u8,
                tid: number("tid").unwrap_or(-1.0) as i64,
                targets,
                used: false,
                charge: 100,
                map_x: instance.x,
                map_y: instance.y,
            };
        }
        if object_id == ObjectId::AbadonCloud {
            instance.vars = ObjectVars::AcidCloud { nid: number("nid").unwrap_or(0.0) as u8, burning: false };
            // Its frame comes from the acid's clock.
            instance.image_speed = 0.0;
        }
        if object_id == ObjectId::MarijunaLavaplatform {
            instance.vars = ObjectVars::LavaColumn { nid: 0, start_y: instance.y, state: 0 };
        }
        if object_id == ObjectId::MarijunaCrystal {
            instance.vars = ObjectVars::MjCrystal { lit: false, glow: 0.0 };
        }
        if object_id == ObjectId::MarijunaJudger {
            instance.vars = ObjectVars::Judger { state: 0 };
        }
        if object_id == ObjectId::VvVase {
            let nid = tiled::find_property(&object.properties, "nid").and_then(|value| value.as_f64());
            instance.vars = ObjectVars::Vase { nid: nid.unwrap_or(0.0) as u8 };
        }
        if object_id == ObjectId::NapIceblock {
            let nid = tiled::find_property(&object.properties, "nid").and_then(|value| value.as_f64());
            instance.vars = ObjectVars::IceBlock { nid: nid.unwrap_or(0.0) as u8 };
        }
        if object_id == ObjectId::NapSnowballWaypoint {
            let number = |name| tiled::find_property(&object.properties, name).and_then(|value| value.as_f64());
            instance.vars = ObjectVars::SnowballWaypoint {
                nid: number("nid").unwrap_or(0.0) as u8,
                wid: number("wid").unwrap_or(0.0) as u8,
                roll_speed: number("roll_speed"),
                roll_frames: number("roll_frames"),
            };
        }
        if object_id == ObjectId::HdCrystal {
            instance.vars = ObjectVars::Crystal { lit: false, usable: true };
        }
        if object_id == ObjectId::YcrSmokearea {
            let nid = tiled::find_property(&object.properties, "nid").and_then(|value| value.as_f64());
            instance.vars = ObjectVars::SmokeArea { nid: nid.unwrap_or(0.0) as u8, gassed: false };
        }
        if object_id == ObjectId::Soundemitter {
            let kind = tiled::find_property(&object.properties, "stype").and_then(|value| value.as_f64());
            instance.vars = ObjectVars::SoundEmitter { kind: kind.unwrap_or(-1.0) as i64 };
        }
        if object_id == ObjectId::Abyss {
            let should_stun = tiled::find_property(&object.properties, "shouldStun").and_then(|value| value.as_bool());
            instance.vars = ObjectVars::Abyss { should_stun: should_stun.unwrap_or(false) };
        }
        Ok(())
    }
}

fn run_create_event(instance: &mut Instance, springs: &Springs) {
    use ObjectId::*;
    let launch = match instance.object {
        SpringUp => Some(springs.spring_up),
        SpringLeft => Some(springs.spring_left),
        SpringRight => Some(springs.spring_right),
        BspringUp => Some(springs.big_spring_up),
        BspringLeft => Some(springs.big_spring_left),
        BspringRight => Some(springs.big_spring_right),
        YspringUp => Some(springs.yellow_spring_up),
        _ => None,
    };
    if let Some(launch) = launch {
        instance.vars = ObjectVars::Spring { xdir: launch.x, ydir: launch.y };
        instance.image_speed = 0.0;
    }
    // Objects whose frame is chosen, not animated.
    if matches!(instance.object, HdSpring | Deserttown | DeserttownMisc | DeserttownTiles | Movingspike | Act9Bodies | Eggstatue) {
        instance.image_speed = 0.0;
    }
    if instance.object == GhzWater {
        instance.vars = ObjectVars::Water { electro: false };
    }
}

/// Create events that set canStuck. Every obj_floor_parent child inherits
/// `canStuck = true`; these override it with false.
fn create_can_stuck(object: ObjectId) -> bool {
    use ObjectId::*;
    let never_sticks = matches!(
        object,
        GhzSlope | GhzSlope2 | GhzSlope3 | GhzSlope4 | GhzSlope5 | HdDoor | HdDoor2 | Hn2Slope1 | Hn2Slope2
            | MarijunaSlops | MarijunaLavaplatform | KafSlope1 | KafSlope2 | NapIceblock | SolidBlock2 | WeedSlope
            | WeedConveyor | WeedSlopeJumpthrough | Movingspike | YcrSlope | YcrSlope2 | Spike
    );
    is_a(object, FloorParent) && !never_sticks
}

/// Create events that move a placed object off its room layer to a depth of its own
/// (`depth = ...`); only drawing cares. Depths computed from other instances come with
/// their objects.
pub fn create_depth(object: ObjectId) -> Option<i32> {
    use ObjectId::*;
    Some(match object {
        AbadonCloud => 101,
        Act9Wall => -90,
        BlackringSpawner | RingSpawner | MarijunaCrystalcontroller => -1,
        DarktowerDarkness => -100,
        DarktowerJumpscare => -105,
        DotdotdotI => -999,
        HdCrystal => 10,
        LimpcityEchain1 => 150,
        LimpcityEyea | LimpcityEyeb => 90,
        MajongController => -900,
        NapIceblock | PfLift => -2,
        NapSnowball => 102,
        MarjiunaStatic => -500,
        RedringScreen2 => -500,
        DarktowerStalactite => 101,
        DarktowerTailsdoll => 0,
        // depth = global.player.depth + 2
        Npring => 2,
        NpBackground => 250,
        RavinemistController => -200,
        VvLavacolumn => 249,
        Weed => -400,
        _ => return None,
    })
}

pub fn object_by_name(name: &str) -> Option<ObjectId> {
    ALL_OBJECTS.into_iter().find(|object| object.info().name == name)
}

/// The GameMaker object a map object stands for: its own class, or its tile's.
pub fn map_object_class(object: &tiled::Object, tile: Option<&TileInfo>) -> Result<ObjectId> {
    let class = if object.class.is_empty() { tile.map_or("", |tile| tile.class.as_str()) } else { &object.class };
    object_by_name(class).with_context(|| format!("unknown object class {class:?}"))
}

/// Position, scale and rotation of a map object the GameMaker way, for a sprite with this origin.
pub fn map_object_placement(object: &tiled::Object, tile: Option<&TileInfo>, origin: (f64, f64)) -> GmPlacement {
    let (image_width, image_height) = tile.map_or((object.width, object.height), |tile| (tile.width, tile.height));
    placement::tiled_to_gm(
        TiledPlacement {
            x: object.x,
            y: object.y,
            width: object.width,
            height: object.height,
            rotation: object.rotation,
            flip_horizontal: object.flipped_horizontally(),
            flip_vertical: object.flipped_vertically(),
        },
        SpriteFrame { width: image_width, height: image_height, origin_x: origin.0, origin_y: origin.1 },
    )
}

/// Object layers that hold instances (asset layers only hold decoration sprites).
fn collect_instance_objects<'a>(layers: &'a [Layer], out: &mut Vec<&'a tiled::Object>) {
    for layer in layers {
        match layer {
            Layer::Group(group) => collect_instance_objects(&group.layers, out),
            Layer::ObjectGroup(group) => {
                let is_asset_layer = tiled::find_property(&group.properties, "asset_layer").and_then(|v| v.as_bool()) == Some(true);
                if !is_asset_layer {
                    out.extend(group.objects.iter());
                }
            }
            Layer::ImageLayer(_) => {}
        }
    }
}

pub struct TileInfo {
    pub class: String,
    pub width: f64,
    pub height: f64,
    /// Image path as written in the tileset, relative to the tileset file.
    pub image: String,
}

/// gid -> tile, across all tilesets referenced by a map.
pub struct TileLookup {
    /// (first gid, tiles by tile id), sorted by first gid.
    tilesets: Vec<(u32, HashMap<u32, TileInfo>)>,
}

impl TileLookup {
    pub fn load(map_path: &Path, map: &tiled::Map) -> Result<TileLookup> {
        let mut tilesets = Vec::new();
        for reference in &map.tilesets {
            let tileset = tiled::load_tileset(&tiled::relative_to(map_path, &reference.source))?;
            let tiles = tileset
                .tiles
                .into_iter()
                .map(|tile| {
                    let info = TileInfo { class: tile.class, width: tile.imagewidth as f64, height: tile.imageheight as f64, image: tile.image };
                    (tile.id, info)
                })
                .collect();
            tilesets.push((reference.firstgid, tiles));
        }
        tilesets.sort_by_key(|(first_gid, _)| *first_gid);
        Ok(TileLookup { tilesets })
    }

    pub fn get(&self, gid: u32) -> Option<&TileInfo> {
        let (first_gid, tiles) = self.tilesets.iter().rev().find(|(first_gid, _)| *first_gid <= gid)?;
        tiles.get(&(gid - first_gid))
    }
}
