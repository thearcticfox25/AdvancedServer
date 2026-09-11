use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};
use crate::vote::TICKS_PER_SEC;
use rand::Rng;

pub struct HillThunder {
    pub id: u16,
    pub timer: f64,
    pub flag: bool,
}

impl HillThunder {
    pub fn new() -> Self {
        Self { id: 0, timer: 15.0 * TICKS_PER_SEC, flag: false }
    }
}

impl Entity for HillThunder {
    fn tag(&self) -> &'static str { "thunder" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { (0.0, 0.0) }

    fn on_init(&mut self, ctx: &mut EntityCtx) -> bool {
        let extra = ctx.rand.gen_range(0u32..5) as f64;
        self.timer = (15.0 + extra) * TICKS_PER_SEC;
        true
    }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        if self.timer <= 2.0 * TICKS_PER_SEC && !self.flag {
            let mut pkt = Packet::new(PacketType::SERVER_GHZTHUNDER_STATE);
            let _ = pkt.write_u8(0);
            ctx.broadcast(pkt, true);
            self.flag = true;
        } else if self.timer <= 0.0 {
            let mut pkt = Packet::new(PacketType::SERVER_GHZTHUNDER_STATE);
            let _ = pkt.write_u8(1);
            ctx.broadcast(pkt, true);

            let cfg = crate::config::cfg();
            let thunder = &cfg.states.gameplay.entities_misc.map_specific.hills.thunder;
            let offset = if thunder.timer_offset == 0 {
                0u32
            } else {
                ctx.rand.gen_range(0u32..thunder.timer_offset as u32)
            };

            self.timer = (thunder.timer as f64 + offset as f64) * TICKS_PER_SEC;
            self.flag = false;
            return true;
        }

        self.timer -= 1.0;
        true
    }
}
