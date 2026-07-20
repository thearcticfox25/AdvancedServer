use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};

pub struct ExellerClone {
    pub id: u16,
    pub pos: (f32, f32),
    pub dir: i8,
    pub owner: u16,
}

impl ExellerClone {
    pub fn new(x: f32, y: f32, dir: i8, owner: u16) -> Self {
        Self { id: 0, pos: (x, y), dir, owner }
    }
}

impl Entity for ExellerClone {
    fn tag(&self) -> &'static str { "exclone" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { self.pos }

    fn on_init(&mut self, ctx: &mut EntityCtx) -> bool {
        let mut pkt = Packet::new(PacketType::SERVER_EXELLERCLONE_STATE);
        let _ = pkt.write_u8(0);
        let _ = pkt.write_u16(self.id);
        let _ = pkt.write_u16(self.owner);
        let _ = pkt.write_u16(self.pos.0 as u16);
        let _ = pkt.write_u16(self.pos.1 as u16);
        let _ = pkt.write_i8(self.dir);
        ctx.broadcast(pkt, true);
        true
    }

    fn on_uninit(&mut self, _ctx: &mut EntityCtx) {}

    fn owner_id(&self) -> u16 { self.owner }
    fn dir_i8(&self) -> i8 { self.dir }
}
