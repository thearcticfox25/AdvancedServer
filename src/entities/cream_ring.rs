use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};

pub struct CreamRing {
    pub id: u16,
    pub pos: (f32, f32),
    pub rid: u8,
    pub red: bool,
}

impl CreamRing {
    pub fn new(x: f32, y: f32, red: bool) -> Self {
        Self { id: 0, pos: (x, y), rid: 255, red }
    }
}

impl Entity for CreamRing {
    fn tag(&self) -> &'static str { "cring" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { self.pos }

    fn is_red(&self) -> bool { self.red }

    fn on_init(&mut self, ctx: &mut EntityCtx) -> bool {

        let mut pkt = Packet::new(PacketType::SERVER_RING_STATE);
        let _ = pkt.write_u8(2);
        let _ = pkt.write_u16(self.pos.0 as u16);
        let _ = pkt.write_u16(self.pos.1 as u16);
        let _ = pkt.write_u8(self.rid);
        let _ = pkt.write_u16(self.id);
        let _ = pkt.write_u8(self.red as u8);
        ctx.broadcast(pkt, true);
        true
    }

    fn on_uninit(&mut self, ctx: &mut EntityCtx) {
        let mut pkt = Packet::new(PacketType::SERVER_RING_STATE);
        let _ = pkt.write_u8(1);
        let _ = pkt.write_u8(self.rid);
        let _ = pkt.write_u16(self.id);
        ctx.broadcast(pkt, true);
    }
}
