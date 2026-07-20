use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};
use crate::vote::TICKS_PER_SEC;
use rand::Rng;

pub struct MjAss {
    pub id: u16,
    pub next_time: f64,
    pub timer: f64,
    pub state: bool,
}

impl MjAss {
    pub fn new() -> Self {
        Self {
            id: 0,
            next_time: 10.0 * TICKS_PER_SEC,
            timer: 0.0,
            state: false,
        }
    }
}

impl Entity for MjAss {
    fn tag(&self) -> &'static str { "mass" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { (0.0, 0.0) }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        if self.timer >= self.next_time {
            self.state = !self.state;
            self.timer = 0.0;

            let mut pkt = Packet::new(PacketType::SERVER_MJCRYSTAL_STATE);
            let _ = pkt.write_u8(self.state as u8);
            ctx.broadcast(pkt, true);

            let extra = ctx.rand.gen_range(0u32..5) as f64;
            self.next_time = (10.0 + extra) * TICKS_PER_SEC;
        }

        self.timer += 1.0;
        true
    }
}
