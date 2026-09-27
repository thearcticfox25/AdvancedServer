//! The moving spikes of You Can't Run, "..." and Fart Zone (the server's
//! spike_controller): every spike shows the same frame, which rises and falls on a clock.

use crate::core::resources::names::sprite;
use crate::core::config::{ticks, Hazards};
use crate::core::events::SimEvent;
use crate::core::objects::ids::ObjectId;
use crate::core::world::World;
use serde::{Deserialize, Serialize};

/// The spikes wait at these frames (down, up); every other frame lasts a tick.
const WAITING_FRAMES: [u8; 2] = [0, 2];
const LAST_FRAME: u8 = 5;
/// The frames the spikes make their sound at, going up and going down.
const MOVING_FRAMES: [u8; 2] = [1, 3];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpikeClock {
    frame: u8,
    timer: i32,
}

impl Default for SpikeClock {
    fn default() -> SpikeClock {
        // spike_controller starts with two seconds down, whatever the configured wait.
        SpikeClock { frame: 0, timer: ticks(2.0) }
    }
}

impl SpikeClock {
    pub(super) fn tick(&mut self, hazards: &Hazards, events: &mut Vec<SimEvent>) {
        self.timer -= 1;
        if self.timer > 0 {
            return;
        }
        self.frame = if self.frame >= LAST_FRAME { 0 } else { self.frame + 1 };
        // The server set 0.25 ticks for the frames between: gone on the next tick.
        self.timer = if WAITING_FRAMES.contains(&self.frame) { ticks(hazards.moving_spike_wait_seconds) } else { 1 };
        if MOVING_FRAMES.contains(&self.frame) {
            events.push(SimEvent::SpikesMove);
        }
    }

    pub(super) fn frame(&self) -> u8 {
        self.frame
    }
}

/// SERVER_MOVINGSPIKE_STATE on clients, then obj_movingspike's Begin Step: a spike
/// that is up blocks like a whole spike, one that is down only by its base.
pub(super) fn apply(world: &mut World, frame: u8) {
    let up = frame > 0 && frame < LAST_FRAME;
    let spikes: Vec<_> = world.ids_of(ObjectId::Movingspike).collect();
    for spike in spikes {
        let instance = &mut world.instances[spike];
        instance.image_index = frame as f64;
        instance.mask_index = up.then_some(sprite::SPR_SPIKE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spikes_wait_down_rise_wait_up_and_fall_back() {
        let hazards = Hazards::default();
        let mut clock = SpikeClock::default();
        let mut events = Vec::new();
        let mut frames = Vec::new();
        for _ in 0..ticks(2.0) * 2 + 5 {
            clock.tick(&hazards, &mut events);
            if frames.last() != Some(&clock.frame()) {
                frames.push(clock.frame());
            }
        }
        assert_eq!(frames, vec![0, 1, 2, 3, 4, 5, 0]);
        assert_eq!(events.len(), 2, "a sound going up and one going down");
    }
}
