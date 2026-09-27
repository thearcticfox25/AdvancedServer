//! scr_collision_objects_after: springs, spikes, damage zones, angle nullers.

use super::hurt::Hit;
use super::{Buttons, Character, Player};
use crate::core::resources::names::{sound, sprite};
use crate::core::config::{self, GameplayConfig, Rect};
use crate::core::events::SimEvent;
use crate::core::objects::ids::ObjectId;
use crate::core::rooms::ids::RoomId;
use super::UsedSpring;
use crate::core::world::{InstanceId, ObjectVars, World};

/// obj_deathtp: a body comes back this far above the point.
const DEATH_POINT_LIFT: f64 = 2.0;
/// obj_majong_controller: half a view from either end, the player comes out at the other.
const WRAP_MARGIN: f64 = 240.0;

impl Player {
    /// Step events of the room's instances that act on the player (obj_deathtp,
    /// obj_abyss, obj_ghz_water, Majin Forest's controller and trigger). Room
    /// instances run their Step before the player's.
    pub(super) fn room_object_steps(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        if world.ids_of(ObjectId::MajongController).next().is_some() {
            self.wrap_around(world);
        }
        self.fall_out_of_majin_forest(world, events);
        self.touch_pits(world, cfg, events);
        self.push_body_out_of_doors(world);
        self.hold_on_ziplines(world);
        self.touch_lifts(world, cfg, events);
        self.glide_through_ice(world, events);
        self.roll_over_by_snowballs(world, cfg, events);
        self.touch_lava(world, cfg, events);
        self.break_vases(world, events);
        self.touch_dark_tower_balls(world, cfg, events);
        self.stay_on_stage(world);
        self.ride_conveyors(world, cfg);
        self.hit_dummy(world, cfg, events);
        self.touch_statue(world, cfg);
        self.touch_slugs(world, cfg, events);
        self.meet_lava_platform(world, cfg, events);
        self.touch_crystals(world, cfg, events);
        self.meet_limp_city(world, cfg, events);
        self.touch_acid(world, cfg);
        self.climb_ladders(world, cfg);
        if world.ids_of(ObjectId::YcrSmokearea).next().is_some() {
            self.breathe_gas(world, cfg, events);
        }
        for water in world.ids_of(ObjectId::GhzWater) {
            self.wade(world, cfg, water, events);
        }
    }

    /// obj_pf_lift Step: a faded in lift is asked for when touched; its rider stays on it.
    fn touch_lifts(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        if self.hp > 0 && self.riding_lift.is_none() {
            let lift = self.instance_place(world, self.x, self.y, ObjectId::PfLift).filter(|&lift| world.instances[lift].image_alpha >= 1.0);
            if let Some(lift) = lift {
                events.push(SimEvent::LiftTouched { lift });
            }
        }
        self.ride_lift(world, cfg);
    }

    /// A rider is held on its lift, whatever it was doing.
    pub(super) fn ride_lift(&mut self, world: &World, cfg: &GameplayConfig) {
        let Some(lift) = self.riding_lift else { return };
        self.x = world.instances[lift].x;
        self.y = world.instances[lift].y - cfg.levels.priceless_freedom.lift_rider_above;
        self.xspd = 0.0;
        self.yspd = 0.0;
        self.is_attacking = false;
        self.is_hurt = false;
        self.is_flying = false;
        self.is_spinning = false;
        self.is_jumping = false;
        self.is_grounded = false;
        self.is_stomping = false;
        self.can_move = true;
    }

    /// obj_majong_controller: Majin Forest's two ends are the same place.
    fn wrap_around(&mut self, world: &World) {
        if self.x <= WRAP_MARGIN && self.xspd < 0.0 {
            self.x = world.room_width - WRAP_MARGIN;
        }
        if self.x >= world.room_width - WRAP_MARGIN && self.xspd > 0.0 {
            self.x = WRAP_MARGIN;
        }
    }

    /// obj_majong_trigger: falling out of Majin Forest puts the player back on the nearest point.
    fn fall_out_of_majin_forest(&mut self, world: &World, events: &mut Vec<SimEvent>) {
        if self.instance_place(world, self.x, self.y, ObjectId::MajongTrigger).is_none() {
            return;
        }
        if !self.teleport_out_of_majin_forest(world, events) {
            // The original flashes and sounds even where the map has no point to return to.
            events.push(SimEvent::TeleportFlash);
            events.push(SimEvent::LocalSound { sound: sound::SND_NPTELEPORT });
        }
    }

    /// The teleport of obj_majong_trigger itself: back on the nearest point, no damage
    /// and its own sound. False when the map has no such point, which is also what tells
    /// the abyss rule whether this map catches its own falls.
    pub(super) fn teleport_out_of_majin_forest(&mut self, world: &World, events: &mut Vec<SimEvent>) -> bool {
        let Some(point) = world.instance_nearest(self.x, self.y, ObjectId::MajongTriggerPoint) else {
            return false;
        };
        self.x = world.instances[point].x;
        self.y = world.instances[point].y;
        self.xspd = 0.0;
        self.gspd = 0.0;
        self.yspd = 0.0;
        events.push(SimEvent::TeleportFlash);
        events.push(SimEvent::LocalSound { sound: sound::SND_NPTELEPORT });
        true
    }

    /// obj_ycr_smokearea Step, with what its puffs (obj_ycr_smoke) found at the end of the
    /// last tick: breathing gas brings the red screen, then every second eats a ring or hurts.
    fn breathe_gas(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let rules = &cfg.levels.you_cant_run;
        let can_breathe = !self.is_dead && self.character != Character::Exe && !self.is_demonized();
        if !(can_breathe && self.touches_gas(world)) {
            self.gas_timer = -config::ticks(rules.gas_red_screen_after_seconds);
            return;
        }
        if self.gas_timer >= config::ticks(rules.gas_hurt_every_seconds) {
            if self.rings > 0 {
                events.push(SimEvent::Sound { sound: sound::SND_RINGABSORB, x: self.x, y: self.y });
                self.rings -= 1;
            } else {
                let hit = Hit { xpw: -self.image_xscale * rules.gas_knockback_x, ..Hit::damage(cfg, rules.gas_damage) };
                self.hurt(world, cfg, hit, events);
            }
            self.gas_timer = 0;
        }
        if self.gas_timer >= 0 && self.red_ring_timer <= 0 {
            events.push(SimEvent::RedRingStarted);
            self.red_ring_timer = config::ticks(rules.gas_red_screen_seconds);
        }
        self.gas_timer += 1;
    }

    /// The puffs a gassed area is filled with, a sprite apart (228 x 178, sway left out).
    // ponytail: the puffs sway 4 px by the clock in the original; their resting boxes are used
    fn touches_gas(&self, world: &World) -> bool {
        let body = crate::core::collision::sprite_bbox(world.sprites.get(self.sprite_index), self.x, self.y, self.image_xscale, 1.0, 0.0);
        let puff = world.sprites.get(sprite::SPR_SMOKE);
        world.ids_of(ObjectId::YcrSmokearea).any(|area| {
            let (ObjectVars::SmokeArea { gassed: true, .. }, Some(bbox)) = (&world.instances[area].vars, world.bbox(area)) else { return false };
            let columns = (0..(bbox.right - bbox.left) as i64).step_by(puff.width as usize);
            columns.into_iter().any(|column| {
                (0..(bbox.bottom - bbox.top) as i64).step_by(puff.height as usize).any(|row| {
                    crate::core::collision::sprite_bbox(puff, bbox.left + column as f64, bbox.top + row as f64, 1.0, 1.0, 0.0).overlaps(&body)
                })
            })
        })
    }

    /// obj_ghz_water Step: in it a player is slowed, and shocked while the lightning lasts.
    fn wade(&mut self, world: &World, cfg: &GameplayConfig, water: InstanceId, events: &mut Vec<SimEvent>) {
        let rules = &cfg.levels.green_hill;
        let feet_in_water = world.collides_point(water, self.sensor_bl.x, self.sensor_bl.y) || world.collides_point(water, self.sensor_br.x, self.sensor_br.y);
        if feet_in_water && world.instances[water].vars == (ObjectVars::Water { electro: true }) {
            let shock = Hit { xpw: -self.image_xscale * rules.shock_knockback_x, ..Hit::damage(cfg, rules.shock_damage) };
            self.hurt(world, cfg, shock, events);
        }
        if feet_in_water != self.in_water {
            self.in_water = feet_in_water;
            let surface = world.bbox(water).map_or(self.y, |bbox| bbox.top);
            events.push(SimEvent::Sound { sound: sound::SND_WATERSPLASH, x: self.x, y: self.y });
            events.push(SimEvent::Effect { sprite: sprite::SPR_WATERSPLASH, x: self.x, y: surface, xscale: 1.0, image_speed: 1.0, yspd: 0.0 });
        }
        if feet_in_water {
            self.slow_down(cfg, cfg.physics.slowdown_percent, rules.water_slow_seconds);
        }
    }

    fn touch_pits(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        // A dead survivor sliding into a pit comes back where teammates can reach it.
        // The original tests the bottom-left sensor twice (BR was meant), so one sensor it is.
        let body_in_pit = self.state == super::animation::DEAD
            && self.hp <= 0
            && world.position_meeting(self.sensor_bl.x, self.sensor_bl.y, ObjectId::Deathtp);
        if let Some(point) = world.instance_nearest(self.x, self.y, ObjectId::DeathtpPoint).filter(|_| body_in_pit) {
            self.x = world.instances[point].x;
            self.y = world.instances[point].y - DEATH_POINT_LIFT;
            self.gspd = 0.0;
            self.xspd = 0.0;
            self.yspd = 0.0;
        }
        let Some(abyss) = self.instance_place(world, self.x, self.y, ObjectId::Abyss) else { return };
        let (abyss_x, abyss_y) = (world.instances[abyss].x, world.instances[abyss].y);
        let should_stun = world.instances[abyss].vars == ObjectVars::Abyss { should_stun: true };
        if let Some(target) = world.instance_nearest(abyss_x, abyss_y, ObjectId::AbyssTarget) {
            self.pull_out_of_abyss(world, cfg, target, should_stun, events);
        }
    }

    pub(super) fn touch_level_objects(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        if self.is_dead {
            return;
        }
        self.use_springs(world, cfg, events);
        self.touch_hazards(world, cfg, events);
        self.break_monitor(world, events);
        self.touch_angle_nullers(world);
        // Crouching in a hiding place.
        self.is_hiding = self.meeting_at(world, self.x, self.y, ObjectId::HidingParent) && self.is_looking_down && !self.is_looking_up;
        if self.character == Character::Sally && self.is_sliding {
            self.is_hiding = false;
        }
    }

    /// obj_weed_conveyor Step: a bottom sensor on a conveyor (or just right of it)
    /// carries the player left. The sensors are where the last tick left them.
    fn ride_conveyors(&mut self, world: &World, cfg: &GameplayConfig) {
        const REACH_RIGHT_OF_IT: f64 = 4.0;
        let on_conveyor = world.ids_of(ObjectId::WeedConveyor).any(|conveyor| {
            let Some(bbox) = world.bbox(conveyor) else { return false };
            // GameMaker's bbox_right and bbox_bottom are the last pixels inside.
            let touches = |sensor: super::Sensor| sensor.x >= bbox.left - REACH_RIGHT_OF_IT && sensor.x <= bbox.right - 1.0 && sensor.y >= bbox.top && sensor.y <= bbox.bottom - 1.0;
            touches(self.sensor_bl) || touches(self.sensor_br)
        });
        if on_conveyor {
            self.x -= cfg.levels.weed_zone.conveyor_speed * config::step();
        }
    }

    /// obj_abadon_cloud Step (obj_am_controller Begin Step clears the flag first).
    fn touch_acid(&mut self, world: &World, cfg: &GameplayConfig) {
        let rules = &cfg.levels.torture_cave;
        let body = crate::core::collision::sprite_bbox(world.sprites.get(self.sprite_index), self.x, self.y, self.image_xscale, 1.0, 0.0);
        self.in_acid = world.ids_of(ObjectId::AbadonCloud).any(|cloud| {
            let instance = &world.instances[cloud];
            let burning = matches!(instance.vars, ObjectVars::AcidCloud { burning: true, .. });
            burning && instance.visible && instance.image_index >= rules.acid_harmful_frame && world.bbox(cloud).is_some_and(|bbox| bbox.overlaps(&body))
        });
    }

    /// obj_am_controller End Step: the acid bites while the player stays in it.
    pub(super) fn bitten_by_acid(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let rules = &cfg.levels.torture_cave;
        if !self.in_acid {
            self.acid_timer = config::ticks(rules.acid_first_bite_seconds);
            return;
        }
        if self.acid_timer > 0 {
            self.acid_timer -= 1;
            return;
        }
        if self.rings <= 0 {
            let hit = Hit { xpw: -self.image_xscale * rules.acid_knockback_x, ..Hit::damage(cfg, rules.acid_damage) };
            self.hurt(world, cfg, hit, events);
        } else {
            events.push(SimEvent::Sound { sound: sound::SND_RINGABSORB, x: self.x, y: self.y });
            self.rings -= 1;
        }
        self.acid_timer = config::ticks(rules.acid_bite_seconds);
    }

    /// Kind and Fair's speed monitors break under an attack, a jump, a spin, a slide or a boost.
    fn break_monitor(&self, world: &World, events: &mut Vec<SimEvent>) {
        let Some(monitor) = world.collision_rectangle(self.x - 6.0, self.y - 20.0, self.x + 6.0, self.y + 20.0, ObjectId::KafSpeedbox) else { return };
        let ObjectVars::Monitor { nid, broken: false } = world.instances[monitor].vars else { return };
        if world.instances[monitor].image_index != 0.0 {
            return;
        }
        use super::amy::AMY_HJUMP;
        use super::animation::FALL;
        use super::sally::SALLY_SLIDE;
        let attacks = matches!(self.character, Character::Exe | Character::Amy | Character::Knux | Character::Eggman | Character::Sally);
        let mut attacking = (self.is_attacking && attacks)
            || (self.character == Character::Amy && self.state == AMY_HJUMP)
            || self.is_jumping
            || (self.state == FALL && self.character == Character::Amy)
            || self.is_spinning
            || (self.state == SALLY_SLIDE && self.character == Character::Sally);
        if self.character == Character::Exe && self.invis_timer > 0 {
            attacking = false;
        }
        if attacking || self.is_boosting {
            events.push(SimEvent::MonitorHit { nid });
        }
    }

    /// Alarm events of the springs this player used.
    pub(super) fn count_down_springs(&mut self) {
        for used in &mut self.used_springs {
            used.frame_ticks -= 1;
            used.recharge_ticks -= 1;
        }
        self.used_springs.retain(|used| used.frame_ticks > 0 || used.recharge_ticks > 0);
    }

    /// Whether this spring still shows pressed and grey for this player.
    pub fn spring_look(&self, spring: InstanceId) -> (bool, bool) {
        let used = self.used_springs.iter().find(|used| used.spring == spring);
        used.map_or((false, false), |used| (used.frame_ticks > 0, used.recharge_ticks > 0))
    }

    fn use_springs(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        if let Some(spring) = self.spring_under_feet(world, ObjectId::SpringParent) {
            let ObjectVars::Spring { xdir, ydir } = world.instances[spring].vars else { return };
            self.launch_from_spring(cfg, spring, xdir, ydir, events);
            if world.room == Some(RoomId::Hideandseek2) && !self.is_demonized() {
                events.push(SimEvent::SpringEcho { x: self.x, y: self.y });
            }
        }
        if let Some(spring) = self.spring_under_feet(world, ObjectId::HdSpring) {
            // Holding jump or up throws higher. Raw keyboard_check in the original, no controls gate.
            let hold_higher = self.buttons.held(Buttons::A) || self.buttons.held(Buttons::UP);
            let launch = if hold_higher { cfg.springs.dream_spring_held } else { cfg.springs.dream_spring };
            self.launch_from_spring(cfg, spring, 0.0, launch, events);
        }
    }

    /// A charged spring touching one of the bottom sensors while falling.
    fn spring_under_feet(&self, world: &World, object: ObjectId) -> Option<InstanceId> {
        let spring = world
            .instance_position(self.sensor_bl.x, self.sensor_bl.y, object)
            .or_else(|| world.instance_position(self.sensor_br.x, self.sensor_br.y, object))?;
        let (_, recharging) = self.spring_look(spring);
        let ready = !recharging && self.yspd > 0.0 && self.shocked_timer <= 0;
        ready.then_some(spring)
    }

    fn launch_from_spring(
        &mut self,
        cfg: &GameplayConfig,
        spring: InstanceId,
        xdir: f64,
        ydir: f64,
        events: &mut Vec<SimEvent>,
    ) {
        self.is_hurt = false;
        match self.character {
            Character::Tails if self.is_flying => {
                self.fly_timer = 0;
                self.is_flying = false;
            }
            Character::Knux => self.knux_spring_reset(&cfg.knuckles),
            // Overwritten by the shared reset below, exactly as in the original.
            Character::Eggman => self.is_jumping = true,
            Character::Amy => self.amy_spring_reset(&cfg.amy),
            Character::Cream => self.cream_spring_reset(&cfg.cream),
            Character::Sally => self.sally_spring_reset(&cfg.sally),
            Character::Exe => {
                // The spring uses the generic EXE recharge for every EXE, as in the original.
                self.exe_spring_reset(&cfg.exe);
                if self.exe_character == super::ExeCharacter::Chaos {
                    self.chaos_spring_reset();
                }
            }
            _ => {}
        }
        self.xspd = xdir;
        self.yspd = ydir;
        self.is_boosting = false;
        self.is_spinning = false;
        self.is_grounded = false;
        self.is_jumping = false;

        let (frame_ticks, recharge_ticks) = (config::ticks(cfg.springs.frame_reset_seconds), config::ticks(cfg.springs.recharge_seconds));
        self.used_springs.retain(|used| used.spring != spring);
        self.used_springs.push(UsedSpring { spring, frame_ticks, recharge_ticks });
        events.push(SimEvent::Sound { sound: sound::SND_SPRING, x: self.x, y: self.y });
    }

    fn touch_hazards(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let hazards = &cfg.hazards;
        let spike_hit = Hit {
            xpw: -self.image_xscale * hazards.spike_knockback_x,
            ypw: hazards.spike_knockback_y,
            sound: sound::SND_SPIKE,
            ..Hit::damage(cfg, hazards.spike_damage)
        };

        // The damage zone's top edge follows the vertical speed, as in the original.
        let [left, top, right, bottom] = hazards.damage_zone_hitbox;
        if self.touches_rect(world, [left, top + self.yspd, right, bottom], ObjectId::Damage) {
            self.angle = 0.0;
            self.hurt(world, cfg, spike_hit, events);
        }
        if self.touches_rect(world, hazards.spike_hitbox, ObjectId::Spike) {
            self.angle = 0.0;
            self.hurt(world, cfg, spike_hit, events);
        }
        // `obj_movingspike.image_index` reads the first moving spike, as GML does.
        let [first_safe, last_safe] = hazards.moving_spike_harmful_frames;
        let first_spike_extended = world
            .ids_of(ObjectId::Movingspike)
            .next()
            .is_some_and(|id| world.instances[id].image_index > first_safe && world.instances[id].image_index < last_safe);
        if first_spike_extended && self.touches_rect(world, hazards.spike_hitbox, ObjectId::Movingspike) {
            self.angle = 0.0;
            self.hurt(world, cfg, spike_hit, events);
        }
    }

    /// collision_rectangle with a rectangle given relative to the player.
    fn touches_rect(&self, world: &World, rect: Rect, object: ObjectId) -> bool {
        let [left, top, right, bottom] = rect;
        world.collision_rectangle(self.x + left, self.y + top, self.x + right, self.y + bottom, object).is_some()
    }

    /// Ramp helpers that force the angle back to 0 next to the side sensors.
    fn touch_angle_nullers(&mut self, world: &World) {
        for sensor in [self.sensor_l, self.sensor_r] {
            // The "eggsafe" one is place_meeting: the player's mask moved to the sensor.
            if world.position_meeting(sensor.x, sensor.y, ObjectId::Angleanuller)
                || self.meeting_at(world, sensor.x, sensor.y, ObjectId::AngleanullerEggsafe)
            {
                self.angle = 0.0;
            }
        }
    }
}
