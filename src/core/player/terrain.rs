//! scr_collision_basic: walls, ceiling, floor and slope angle from the sensors.

use super::{point_direction, Character, Player, Sensor};
use crate::core::config::{step, GameplayConfig, Physics};
use crate::core::events::SimEvent;
use crate::core::objects::ids::ObjectId;
use crate::core::rooms::ids::RoomId;
use crate::core::world::{InstanceId, World};

impl Player {
    pub(super) fn collision_basic(&mut self, world: &World, cfg: &GameplayConfig, events: &mut Vec<SimEvent>) {
        let physics = &cfg.physics;
        self.xspd = self.xspd.clamp(-self.max_h_speed, self.max_h_speed);
        self.yspd = self.yspd.clamp(-physics.max_fall_speed_per_tick, physics.max_fall_speed_per_tick);
        if self.y - physics.abyss_margin >= world.room_height {
            self.return_from_abyss(world, cfg, events);
        }

        if !self.is_grounded {
            // Torture Cave holds everyone in the air until the round starts.
            if world.room == Some(RoomId::Torturecave) && !world.round_started {
                return;
            }
            self.yspd += physics.gravity_per_tick * step();
        }
        self.break_ice(world, cfg, events);

        let near = world.collision_circle_list(self.x, self.y, physics.terrain_search_radius, ObjectId::FloorParent);
        self.check_walls(world, cfg, &near, events);
        self.x += self.xspd * step();
        self.check_ceiling(world, physics, &near);
        self.y += self.yspd * step();
        self.check_floor(world, cfg, &near, events);
        self.hold_inside_the_room(world, physics);
        self.touch_level_objects(world, cfg, events);
    }

    /// The room's sides are a wall. Maps draw a frame of wall objects around themselves
    /// (Angel Island does), which a modder can forget or move; the rule here costs the
    /// server nothing and keeps anyone from walking out of the world. The top is left
    /// open because things legitimately come in from above the room, and the bottom is
    /// the abyss, met at the start of the step.
    fn hold_inside_the_room(&mut self, world: &World, physics: &Physics) {
        // Majin Forest's controller carries the player from one side to the other.
        if world.ids_of(ObjectId::MajongController).next().is_some() {
            return;
        }
        let spread = physics.side_sensor_spread;
        let held = self.x.clamp(spread, (world.room_width - spread).max(spread));
        if held != self.x {
            self.x = held;
            self.stop_against_wall();
        }
    }

    /// scr_collision_check: side sensors, scanned over the player's height.
    fn check_walls(&mut self, world: &World, cfg: &GameplayConfig, near: &[InstanceId], events: &mut Vec<SimEvent>) {
        let physics = &cfg.physics;
        self.sensor_l.coll = false;
        self.sensor_r.coll = false;
        // The sensors look as far ahead as this step moves, so a smaller step stops
        // against a wall at the same place, one step later.
        let xspd = self.xspd * step();
        let reach = self.max_h_speed + 1.0;
        let scan_end = if self.is_grounded { 0.0 } else { physics.air_wall_scan_bottom - xspd.abs() };
        let spread = physics.side_sensor_spread;
        let min_speed = physics.wall_check_min_speed;

        let mut yy = physics.wall_scan_top;
        while yy < scan_end {
            self.sensor_l = Sensor { x: self.x.ceil() - spread, y: (self.y + yy).ceil(), coll: self.sensor_l.coll };
            self.sensor_r = Sensor { x: self.x.ceil() + spread, y: (self.y + yy).ceil(), coll: self.sensor_r.coll };
            let left_distance = scan_until_hit(world, near, &mut self.sensor_l, -1.0, 0.0, reach);
            let right_distance = scan_until_hit(world, near, &mut self.sensor_r, 1.0, 0.0, reach);

            if -left_distance >= xspd.min(-min_speed) && !sensor_meets_platform(world, self.sensor_l) {
                self.sensor_l.coll = true;
                if self.xspd < 0.0 {
                    self.x = self.sensor_l.x.ceil() + spread;
                    self.stop_against_wall();
                    let wall = world.instance_position_any(self.sensor_l.x, self.sensor_l.y, near);
                    self.touch_wall_with_ability(world, wall, true, events);
                    break;
                }
            }
            if right_distance <= xspd.max(min_speed) && !sensor_meets_platform(world, self.sensor_r) {
                self.sensor_r.coll = true;
                if self.xspd > 0.0 {
                    self.x = self.sensor_r.x.ceil() - spread;
                    self.stop_against_wall();
                    let wall = world.instance_position_any(self.sensor_r.x, self.sensor_r.y, near);
                    self.touch_wall_with_ability(world, wall, false, events);
                    break;
                }
            }
            yy += 1.0;
        }
    }

    fn touch_wall_with_ability(&mut self, world: &World, wall: Option<InstanceId>, wall_is_left: bool, events: &mut Vec<SimEvent>) {
        match self.character {
            Character::Knux => self.knux_touch_wall(world, wall, wall_is_left, events),
            Character::Exe if self.exe_character == super::ExeCharacter::Chaos => {
                let cfg = world.config.clone();
                self.chaos_touch_wall(world, &cfg.chaos, wall, events);
            }
            _ => {}
        }
    }

    fn stop_against_wall(&mut self) {
        self.xspd = 0.0;
        self.gspd = 0.0;
        self.is_boosting = false;
        if self.character == Character::Sally {
            self.is_sliding = false;
        }
    }

    /// scr_collision_check_top: stop upward movement under a ceiling.
    fn check_ceiling(&mut self, world: &World, physics: &Physics, near: &[InstanceId]) {
        let reach = physics.max_fall_speed_per_tick + 1.0;
        let (spread, above) = (physics.top_sensor_spread, physics.head_sensor_above_position);
        self.sensor_tl = Sensor { x: self.x.ceil() - spread, y: self.y.ceil() - above, coll: self.sensor_tl.coll };
        self.sensor_tr = Sensor { x: self.x.ceil() + spread, y: self.y.ceil() - above, coll: self.sensor_tr.coll };
        let distance_1 = scan_until_hit(world, near, &mut self.sensor_tl, 0.0, -1.0, reach);
        let distance_2 = scan_until_hit(world, near, &mut self.sensor_tr, 0.0, -1.0, reach);
        let hit_1 = distance_1 != reach;
        let hit_2 = distance_2 != reach;

        // With both sensors hitting, the lower ceiling point decides.
        let winner = match (hit_1, hit_2) {
            (true, true) if self.sensor_tl.y >= self.sensor_tr.y => Some((self.sensor_tl, distance_1)),
            (true, true) => Some((self.sensor_tr, distance_2)),
            (true, false) => Some((self.sensor_tl, distance_1)),
            (false, true) => Some((self.sensor_tr, distance_2)),
            (false, false) => None,
        };
        let Some((sensor, distance)) = winner else { return };
        if -distance >= self.yspd * step() && self.yspd < 0.0 && !sensor_meets_platform(world, sensor) {
            self.y = sensor.y.ceil() + above;
            self.yspd = 0.0;
        }
    }

    /// scr_collision_check_bottom: landing while in the air, then snapping to the ground.
    fn check_floor(&mut self, world: &World, cfg: &GameplayConfig, near: &[InstanceId], events: &mut Vec<SimEvent>) {
        let physics = &cfg.physics;
        let spread = physics.bottom_sensor_spread;
        self.sensor_bl = Sensor { x: self.x.ceil() - spread, y: self.sensor_bl.y, coll: false };
        self.sensor_br = Sensor { x: self.x.ceil() + spread, y: self.sensor_br.y, coll: false };
        if !self.is_grounded {
            self.try_land(world, physics, near);
        }
        if self.is_grounded {
            // An air dash that lands sticks Chaos to the floor, facing the dash direction.
            if self.exe_character == super::ExeCharacter::Chaos && self.character == Character::Exe {
                self.chaos_try_stick(&cfg.chaos, true, self.image_xscale, events);
            }
            self.stick_to_ground(world, physics, near);
        }
    }

    fn try_land(&mut self, world: &World, physics: &Physics, near: &[InstanceId]) {
        let reach = physics.landing_scan_reach;
        self.angle = 0.0;
        self.sensor_bl.y = self.y.ceil() - physics.sensor_start_above_position;
        self.sensor_br.y = self.y.ceil() - physics.sensor_start_above_position;
        let distance_1 = scan_until_hit(world, near, &mut self.sensor_bl, 0.0, 1.0, reach);
        let distance_2 = scan_until_hit(world, near, &mut self.sensor_br, 0.0, 1.0, reach);
        let hit_1 = distance_1 != reach;
        let hit_2 = distance_2 != reach;

        // The higher floor point wins. Note: the original checks `dist2 >= 15` for a
        // jump-through platform even when only the left sensor hit; kept as is.
        let (sensor, platform_distance) = match (hit_1, hit_2) {
            (true, true) if self.sensor_bl.y < self.sensor_br.y => (self.sensor_br, distance_2),
            (true, true) => (self.sensor_bl, distance_1),
            (true, false) => (self.sensor_bl, distance_2),
            (false, true) => (self.sensor_br, distance_2),
            (false, false) => return,
        };
        // A jump-through platform only catches a player whose feet were above it.
        let on_platform = sensor_meets_platform(world, sensor);
        if self.yspd > 0.0 && (!on_platform || platform_distance >= physics.jump_through_min_distance) {
            self.y = sensor.y - physics.feet_below_position;
            self.is_grounded = true;
            self.yspd = 0.0;
            self.gspd = self.xspd;
        }
    }

    fn stick_to_ground(&mut self, world: &World, physics: &Physics, near: &[InstanceId]) {
        self.sensor_bl.y = self.y.ceil() - physics.sensor_start_above_position;
        self.sensor_br.y = self.y.ceil() - physics.sensor_start_above_position;
        let on_angle_allower = world.position_meeting(self.sensor_l.x, self.sensor_l.y, ObjectId::Angleallower)
            || world.position_meeting(self.sensor_r.x, self.sensor_r.y, ObjectId::Angleallower);
        let reach = if on_angle_allower { physics.ground_scan_reach_on_angle_allower } else { physics.ground_scan_reach };

        let distance_1 = scan_until_hit(world, near, &mut self.sensor_bl, 0.0, 1.0, reach);
        let distance_2 = scan_until_hit(world, near, &mut self.sensor_br, 0.0, 1.0, reach);
        if distance_1 == reach && distance_2 == reach {
            self.angle = 0.0;
            self.is_grounded = false;
            return;
        }

        if self.sensor_bl.y == self.sensor_br.y {
            self.sensor_bl.coll = true;
            self.sensor_br.coll = true;
        }
        if self.sensor_bl.y > self.sensor_br.y {
            self.sensor_br.coll = true;
            self.y = self.sensor_br.y - physics.feet_below_position;
        } else {
            self.sensor_bl.coll = true;
            self.y = self.sensor_bl.y - physics.feet_below_position;
        }
        self.update_ground_angle(world, physics, near);
        // Touching the ground ends Amy's big jump pose.
        if self.character == Character::Amy {
            self.is_hj = false;
        }
    }

    /// scr_collision_check_angle: slope angle from the two angle sensors.
    fn update_ground_angle(&mut self, world: &World, physics: &Physics, near: &[InstanceId]) {
        let reach = physics.angle_scan_reach;
        let (spread, above) = (physics.bottom_sensor_spread, physics.sensor_start_above_position);
        // Unlike the other sensors these start from the exact (not ceiled) position.
        self.sensor_al = Sensor { x: self.x - spread, y: self.y - above, coll: self.sensor_al.coll };
        self.sensor_ar = Sensor { x: self.x + spread, y: self.y - above, coll: self.sensor_ar.coll };
        let distance_1 = scan_until_hit(world, near, &mut self.sensor_al, 0.0, 1.0, reach);
        let distance_2 = scan_until_hit(world, near, &mut self.sensor_ar, 0.0, 1.0, reach);
        let span = physics.angle_sensor_distance;
        // The state here is last tick's: End Step has not run yet.
        let exe_end_pose = self.character == Character::Exe
            && (self.state == super::exe::EXE_WON || self.state == super::exe::EXE_LOST);
        self.angle = if distance_1 != reach && distance_2 != reach && !exe_end_pose {
            point_direction(-span, distance_1, span, distance_2).to_radians()
        } else {
            0.0
        };
    }
}

/// Moves a sensor one pixel at a time until it touches terrain or `reach` is used up.
/// Returns how many pixels it moved (equal to `reach` when nothing was hit).
fn scan_until_hit(world: &World, near: &[InstanceId], sensor: &mut Sensor, step_x: f64, step_y: f64, reach: f64) -> f64 {
    let mut distance = 0.0;
    while !world.position_meeting_any(sensor.x, sensor.y, near) && distance < reach {
        sensor.x += step_x;
        sensor.y += step_y;
        distance += 1.0;
    }
    distance
}

fn sensor_meets_platform(world: &World, sensor: Sensor) -> bool {
    world.position_meeting(sensor.x, sensor.y, ObjectId::PlatformJumptrough)
}
