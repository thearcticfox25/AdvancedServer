//! GameMaker collision rules with "collision compatibility mode" off (as in the
//! original project options). Behavior follows the open-source GameMaker HTML5
//! runner; the closed Windows runner the original shipped with was not checked.

use crate::core::resources::sprites::SpriteMeta;
use crate::core::world::{InstanceId, World};
use crate::core::objects::ids::ObjectId;

/// Floating point bounding box. `right` and `bottom` are exclusive.
#[derive(Clone, Copy, Debug)]
pub struct Bbox {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

/// Keeps a point exactly on the right/bottom edge outside the box, despite float noise.
const EDGE_EPSILON: f64 = 0.00001;

/// GameMaker's Round(): halves go up.
// ponytail: native runner rounding of exact .5 not verified, matters only for half-pixel positions
fn gm_round(value: f64) -> f64 {
    (value + 0.5).floor()
}

impl World {
    /// None when the instance has no sprite and no mask: such instances never collide.
    pub fn bbox(&self, id: InstanceId) -> Option<Bbox> {
        let instance = &self.instances[id];
        let sprite = self.sprites.get(instance.mask_index.or(instance.sprite_index)?);
        Some(sprite_bbox(sprite, instance.x, instance.y, instance.image_xscale, instance.image_yscale, instance.image_angle))
    }

    /// Does the point (x, y) touch this instance's mask?
    pub fn collides_point(&self, id: InstanceId, x: f64, y: f64) -> bool {
        let Some(bbox) = self.bbox(id) else { return false };
        if !bbox.contains_point(x, y) {
            return false;
        }

        let instance = &self.instances[id];
        let sprite = self.sprites.get(instance.mask_index.or(instance.sprite_index).unwrap());
        let Some(masks) = &sprite.masks else { return true };

        // Back from room coordinates into sprite pixels. The half pixel offset matches the runner.
        let dx = gm_round(x) - (gm_round(instance.x) - 0.5);
        let dy = gm_round(y) - (gm_round(instance.y) - 0.5);
        let (u, v) = if instance.image_angle.abs() < 0.0001 {
            (dx / instance.image_xscale + sprite.origin_x, dy / instance.image_yscale + sprite.origin_y)
        } else {
            let (sin, cos) = (-instance.image_angle).to_radians().sin_cos();
            (
                (cos * dx + sin * dy) / instance.image_xscale + sprite.origin_x,
                (cos * dy - sin * dx) / instance.image_yscale + sprite.origin_y,
            )
        };

        let (u, v) = (u.floor() as i32, v.floor() as i32);
        let [left, top, right, bottom] = sprite.bbox;
        if u < left || u > right || v < top || v > bottom {
            return false;
        }
        let mask = &masks[instance.image_index.floor() as usize % masks.len()];
        mask[(v as u32 * sprite.width + u as u32) as usize]
    }

    /// position_meeting(x, y, object)
    pub fn position_meeting(&self, x: f64, y: f64, object: ObjectId) -> bool {
        self.ids_of(object).any(|id| self.collides_point(id, x, y))
    }

    /// position_meeting(x, y, array_of_instances)
    pub fn position_meeting_any(&self, x: f64, y: f64, candidates: &[InstanceId]) -> bool {
        candidates.iter().any(|&id| self.collides_point(id, x, y))
    }

    /// instance_position(x, y, array_of_instances): the first one touching the point.
    pub fn instance_position_any(&self, x: f64, y: f64, candidates: &[InstanceId]) -> Option<InstanceId> {
        candidates.iter().copied().find(|&id| self.collides_point(id, x, y))
    }

    /// collision_rectangle(x1, y1, x2, y2, object, prec = false, notme): first instance
    /// whose bounding box overlaps the rectangle.
    pub fn collision_rectangle(&self, x1: f64, y1: f64, x2: f64, y2: f64, object: ObjectId) -> Option<InstanceId> {
        let (left, right) = (x1.min(x2), x1.max(x2));
        let (top, bottom) = (y1.min(y2), y1.max(y2));
        self.ids_of(object).find(|&id| {
            self.bbox(id)
                .is_some_and(|bbox| left < bbox.right && right >= bbox.left && top < bbox.bottom && bottom >= bbox.top)
        })
    }

    /// instance_position(x, y, object)
    pub fn instance_position(&self, x: f64, y: f64, object: ObjectId) -> Option<InstanceId> {
        self.ids_of(object).find(|&id| self.collides_point(id, x, y))
    }

    /// collision_circle(x, y, radius, object, prec = true, notme): does any mask
    /// pixel of any instance of `object` lie inside the circle?
    // ponytail: samples whole room pixels inside circle and bbox; GameMaker's exact
    // precise-ellipse rule was not checked against the runner
    pub fn collision_circle_precise(&self, x: f64, y: f64, radius: f64, object: ObjectId) -> bool {
        self.ids_of(object).any(|id| {
            let Some(bbox) = self.bbox(id) else { return false };
            let (left, right) = (bbox.left.max(x - radius).floor() as i64, bbox.right.min(x + radius).ceil() as i64);
            let (top, bottom) = (bbox.top.max(y - radius).floor() as i64, bbox.bottom.min(y + radius).ceil() as i64);
            (top..=bottom).any(|py| {
                (left..=right).any(|px| {
                    let (dx, dy) = (px as f64 - x, py as f64 - y);
                    dx * dx + dy * dy <= radius * radius && self.collides_point(id, px as f64, py as f64)
                })
            })
        })
    }

    /// collision_circle_list(x, y, radius, object, prec = false, notme, list, ordered = true).
    /// A bounding box counts when its nearest point is within the radius.
    pub fn collision_circle_list(&self, x: f64, y: f64, radius: f64, object: ObjectId) -> Vec<InstanceId> {
        let mut found: Vec<InstanceId> = self
            .ids_of(object)
            .filter(|&id| {
                self.bbox(id).is_some_and(|bbox| {
                    let nearest_x = x.clamp(bbox.left, bbox.right);
                    let nearest_y = y.clamp(bbox.top, bbox.bottom);
                    (nearest_x - x).powi(2) + (nearest_y - y).powi(2) <= radius * radius
                })
            })
            .collect();
        // "ordered" sorts by distance from the circle center to the instance position.
        let distance = |id: InstanceId| (self.instances[id].x - x).hypot(self.instances[id].y - y);
        found.sort_by(|&a, &b| distance(a).total_cmp(&distance(b)));
        found
    }
}

/// Bounding box of a sprite placed at (x, y) with scale and rotation (Compute_BoundingBox).
pub fn sprite_bbox(sprite: &SpriteMeta, x: f64, y: f64, xscale: f64, yscale: f64, angle: f64) -> Bbox {
    let [left, top, right, bottom] = sprite.bbox;

    // Box corners relative to the position, before rotation.
    let x_min = xscale * (left as f64 - sprite.origin_x);
    let x_max = xscale * (right as f64 + 1.0 - sprite.origin_x);
    let y_min = yscale * (top as f64 - sprite.origin_y);
    let y_max = yscale * (bottom as f64 + 1.0 - sprite.origin_y);

    if angle == 0.0 {
        return Bbox {
            left: x + x_min.min(x_max),
            right: x + x_min.max(x_max),
            top: y + y_min.min(y_max),
            bottom: y + y_min.max(y_max),
        };
    }

    let (sin, cos) = angle.to_radians().sin_cos();
    let span = |a: f64, b: f64| (a.min(b), a.max(b));
    let (cos_x_lo, cos_x_hi) = span(cos * x_min, cos * x_max);
    let (sin_y_lo, sin_y_hi) = span(sin * y_min, sin * y_max);
    let (cos_y_lo, cos_y_hi) = span(cos * y_min, cos * y_max);
    let (sin_x_lo, sin_x_hi) = span(sin * x_min, sin * x_max);
    Bbox {
        left: x + cos_x_lo + sin_y_lo,
        right: x + cos_x_hi + sin_y_hi,
        top: y + cos_y_lo - sin_x_hi,
        bottom: y + cos_y_hi - sin_x_lo,
    }
}

impl Bbox {
    /// position_meeting against a rectangle mask: the right and bottom edges are outside.
    pub fn contains_point(&self, x: f64, y: f64) -> bool {
        x >= self.left && x < self.right - EDGE_EPSILON && y >= self.top && y < self.bottom - EDGE_EPSILON
    }

    /// distance_to_object: the gap between two bounding boxes, 0 when they overlap.
    pub fn distance(&self, other: &Bbox) -> f64 {
        let dx = (other.left - self.right).max(self.left - other.right).max(0.0);
        let dy = (other.top - self.bottom).max(self.top - other.bottom).max(0.0);
        dx.hypot(dy)
    }

    /// Instance against instance, bounding boxes only (Collision_Instance without precise masks).
    /// Boxes that only touch along an edge do not collide, same as the runner.
    pub fn overlaps(&self, other: &Bbox) -> bool {
        self.left < other.right && other.left < self.right && self.top < other.bottom && other.top < self.bottom
    }
}

/// place_meeting for a 1x1 sensor with a full 1 pixel mask at origin 0,0, as
/// obj_player_sensor* are: equal to a point check at the sensor position.
pub fn sensor_meeting(world: &World, x: f64, y: f64, object: ObjectId) -> bool {
    world.ids_of(object).any(|id| world.collides_point(id, x, y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxes_apart_by_three_and_four_are_five_away() {
        let a = Bbox { left: 0.0, top: 0.0, right: 10.0, bottom: 10.0 };
        let b = Bbox { left: 13.0, top: 14.0, right: 20.0, bottom: 20.0 };
        assert_eq!(a.distance(&b), 5.0);
        assert_eq!(a.distance(&Bbox { left: 5.0, top: 5.0, right: 8.0, bottom: 8.0 }), 0.0);
    }
}
