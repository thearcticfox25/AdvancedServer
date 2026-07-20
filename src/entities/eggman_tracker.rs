use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};

pub struct EggmanTracker {
    pub id: u16,
    pub pos: (f32, f32),
    pub activ_id: u16,
}

impl EggmanTracker {
    pub fn new(x: f32, y: f32) -> Self {
        Self { id: 0, pos: (x, y), activ_id: 0 }
    }
}

impl Entity for EggmanTracker {
    fn tag(&self) -> &'static str { "eggtrack" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { self.pos }

    fn on_init(&mut self, ctx: &mut EntityCtx) -> bool {
        let mut pkt = Packet::new(PacketType::SERVER_ETRACKER_STATE);
        let _ = pkt.write_u8(0);
        let _ = pkt.write_u16(self.id);
        let _ = pkt.write_u16(self.pos.0 as u16);
        let _ = pkt.write_u16(self.pos.1 as u16);
        ctx.broadcast(pkt, true);
        true
    }

    fn set_activ_id(&mut self, id: u16) { self.activ_id = id; }

    fn on_uninit(&mut self, ctx: &mut EntityCtx) {
        let mut pkt = Packet::new(PacketType::SERVER_ETRACKER_STATE);
        let _ = pkt.write_u8(1);
        let _ = pkt.write_u16(self.id);
        let _ = pkt.write_u16(self.activ_id);
        ctx.broadcast(pkt, true);
    }
}
