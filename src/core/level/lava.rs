//! Lava that bobs in its pit for a random while, sinks a little, shoots up, bobs up
//! there and comes back down: Volcano Valley's columns (the server's vv_lava) and Echidna
//! Ruins' platform (mj_lava).

use super::Random;
use crate::core::config::{step, ticks, LavaRules};
use crate::core::objects::ids::ObjectId;
use crate::core::world::{ObjectVars, World};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LavaColumns {
    /// By the map's columns in instance order.
    columns: Vec<Column>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Column {
    state: State,
    timer: i32,
    /// How far below its place it is (negative: above).
    below: f64,
    speed: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
enum State {
    Resting = 0,
    Sinking = 1,
    Rising = 2,
    Up = 3,
    Falling = 4,
}

/// What clients are told of one column (SERVER_VVLCOLUMN_STATE, SERVER_MJLAVA_STATE):
/// how far below its place it is, and its state (2 rising, 3 up, 4 falling).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LavaView {
    pub below: f32,
    pub state: u8,
}

impl LavaColumns {
    /// The lava of the map's instances of `object`.
    pub(super) fn new(rules: &LavaRules, world: &World, object: ObjectId, random: &mut Random) -> LavaColumns {
        let columns = world
            .ids_of(object)
            .map(|_| {
                let timer = rules.first_rest_seconds.map_or_else(|| rest_ticks(rules, random), ticks);
                Column { state: State::Resting, timer, below: 0.0, speed: 0.0 }
            })
            .collect();
        LavaColumns { columns }
    }

    pub(super) fn tick(&mut self, rules: &LavaRules, random: &mut Random) {
        let bob = |timer: i32| (timer as f64 * step() / rules.bob_ticks_per_radian).sin() * rules.bob;
        for column in &mut self.columns {
            match column.state {
                State::Resting => {
                    column.below = bob(column.timer);
                    column.timer -= 1;
                    if column.timer <= 0 {
                        column.state = State::Sinking;
                    }
                }
                State::Sinking if column.below < rules.sink => column.below += rules.sink_speed * step(),
                State::Sinking => column.state = State::Rising,
                State::Rising if column.below > -rules.rise => {
                    column.below -= column.speed * step();
                    column.speed = speed_up(column.speed, rules);
                }
                State::Rising => {
                    column.state = State::Up;
                    column.timer = ticks(rules.top_seconds + random.below(rules.top_extra_seconds) as f64);
                    column.speed = 0.0;
                }
                State::Up => {
                    column.below = -rules.rise + bob(column.timer);
                    column.timer -= 1;
                    if column.timer <= 0 {
                        column.state = State::Falling;
                    }
                }
                State::Falling if column.below < 0.0 => {
                    column.below += column.speed * step();
                    column.speed = speed_up(column.speed, rules);
                }
                State::Falling => {
                    column.state = State::Resting;
                    column.timer = rest_ticks(rules, random);
                    column.speed = 0.0;
                }
            }
        }
    }

    pub(super) fn view(&self) -> Vec<LavaView> {
        self.columns.iter().map(|column| LavaView { below: column.below as f32, state: column.state as u8 }).collect()
    }
}

/// The server adds to the speed while below the top speed, so it can pass it for a tick.
fn speed_up(speed: f64, rules: &LavaRules) -> f64 {
    if speed < rules.max_speed {
        speed + rules.acceleration * step()
    } else {
        rules.max_speed
    }
}

fn rest_ticks(rules: &LavaRules, random: &mut Random) -> i32 {
    ticks(rules.rest_seconds + random.below(rules.rest_extra_seconds) as f64)
}

pub(super) fn apply(world: &mut World, columns: &[LavaView]) {
    let ids: Vec<_> = world.ids_of(ObjectId::VvLavacolumn).chain(world.ids_of(ObjectId::MarijunaLavaplatform)).collect();
    for (id, view) in ids.into_iter().zip(columns) {
        let instance = &mut world.instances[id];
        if let ObjectVars::LavaColumn { nid, start_y, .. } = instance.vars {
            instance.vars = ObjectVars::LavaColumn { nid, start_y, state: view.state };
            instance.y = start_y + view.below as f64;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_column_rests_rises_stays_up_and_comes_back() {
        let rules = LavaRules { rest_extra_seconds: 0, top_extra_seconds: 0, ..LavaRules::default() };
        let mut random = Random::new(1);
        let mut lava = LavaColumns { columns: vec![Column { state: State::Resting, timer: rest_ticks(&rules, &mut random), below: 0.0, speed: 0.0 }] };
        let mut highest = 0.0f64;
        let mut roared = 0;
        for _ in 0..ticks(rules.rest_seconds) {
            lava.tick(&rules, &mut random);
        }
        assert!(lava.view()[0].state <= 1, "not rising yet");
        for _ in 0..ticks(60.0) {
            lava.tick(&rules, &mut random);
            highest = highest.min(lava.columns[0].below);
            roared += matches!(lava.view()[0].state, 2 | 3) as i32;
            if lava.columns[0].state == State::Resting {
                break;
            }
        }
        assert!(highest <= -rules.rise, "went up {highest}");
        assert!(roared >= ticks(rules.top_seconds), "roared while up: {roared}");
        assert_eq!(lava.columns[0].state, State::Resting, "back down");
        assert!(lava.columns[0].below >= 0.0 && lava.columns[0].below <= rules.max_speed + rules.acceleration);
    }
}
