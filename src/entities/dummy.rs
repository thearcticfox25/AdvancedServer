use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};

pub struct Dummy {
    pub id: u16,
    pub pos: (f32, f32),
    pub vel: f64,
}

impl Dummy {
    pub fn new() -> Self {
        Self { id: 0, pos: (1616.0, 2608.0), vel: 0.0 }
    }

    pub fn activate(&mut self, dir: i8) {
        self.vel = dir as f64;
    }
}

impl Entity for Dummy {
    fn tag(&self) -> &'static str { "dummy" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { self.pos }
    fn dummy_push(&mut self, spd: i8) { self.activate(spd); }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        self.pos.0 += self.vel as f32;
        self.pos.0 = self.pos.0.min(2944.0).max(1282.0);


        let friction = self.vel.abs().min(0.046875 * 4.0);
        let sign = if self.vel > 0.0 { 1.0 } else if self.vel < 0.0 { -1.0 } else { 0.0 };
        self.vel -= friction * sign;

        let mut pkt = Packet::new(PacketType::SERVER_FART_STATE);
        let _ = pkt.write_u16(self.pos.0 as u16);
        let _ = pkt.write_u16(self.pos.1 as u16);
        ctx.broadcast(pkt, false);

        true
    }
}
