use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};
use crate::vote::TICKS_PER_SEC;
use rand::Rng;

pub struct WdLatern {
    pub id: u16,
    pub side: bool,
    pub timer: f64,
    pub lantern_id: u8,
    pub time: u16,
}

impl WdLatern {
    pub fn new() -> Self {
        Self { id: 0, side: false, timer: 0.0, lantern_id: 0, time: 7 }
    }
}

impl Entity for WdLatern {
    fn tag(&self) -> &'static str { "latrn" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { (0.0, 0.0) }

    fn on_init(&mut self, ctx: &mut EntityCtx) -> bool {
        self.time = 7 + ctx.rand.gen_range(0u16..2);
        true
    }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        self.timer += 1.0;

        if !self.side {
            if self.timer >= self.time as f64 * TICKS_PER_SEC {
                self.lantern_id = ctx.rand.gen_range(0u8..7);

                let mut pkt = Packet::new(PacketType::SERVER_WDLATERN_ACTIVATE);
                let _ = pkt.write_u8(1);
                let _ = pkt.write_u8(self.lantern_id);
                ctx.broadcast(pkt, true);

                self.side = true;
                self.timer = 0.0;
                self.time = 20 + ctx.rand.gen_range(0u16..2);
            }
        } else if self.timer >= self.time as f64 * TICKS_PER_SEC {
            let mut pkt = Packet::new(PacketType::SERVER_WDLATERN_ACTIVATE);
            let _ = pkt.write_u8(0);
            let _ = pkt.write_u8(self.lantern_id);
            ctx.broadcast(pkt, true);

            self.side = false;
            self.timer = 0.0;
            self.time = 7 + ctx.rand.gen_range(0u16..2);
        }

        true
    }
}
