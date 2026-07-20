use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};
use crate::vote::TICKS_PER_SEC;

pub struct SpikeController {
    pub id: u16,
    pub frame: u8,
    pub timer: f64,
}

impl SpikeController {
    pub fn new() -> Self {
        Self { id: 0, frame: 0, timer: 2.0 * TICKS_PER_SEC }
    }
}

impl Entity for SpikeController {
    fn tag(&self) -> &'static str { "spikectrl" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { (0.0, 0.0) }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        self.timer -= 1.0;
        if self.timer > 0.0 {
            return true;
        }

        self.frame += 1;
        if self.frame > 5 {
            self.frame = 0;
        }

        let cfg = crate::config::cfg();
        if self.frame == 0 || self.frame == 2 {
            self.timer = cfg.states.gameplay.entities_misc.global.spikes.timer as f64 * TICKS_PER_SEC;
        } else {

            self.timer = 0.25;
        }

        let mut pkt = Packet::new(PacketType::SERVER_MOVINGSPIKE_STATE);
        let _ = pkt.write_u8(self.frame);
        ctx.broadcast(pkt, true);

        true
    }
}
