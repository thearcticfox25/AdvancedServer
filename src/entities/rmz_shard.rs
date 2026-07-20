use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};

pub struct RmzShard {
    pub id: u16,
    pub pos: (f32, f32),
    pub spawned: u8,
}

impl RmzShard {
    pub fn new(x: f32, y: f32, spawned: u8) -> Self {
        Self { id: 0, pos: (x, y), spawned }
    }
}

impl Entity for RmzShard {
    fn tag(&self) -> &'static str { "shard" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { self.pos }

    fn on_init(&mut self, ctx: &mut EntityCtx) -> bool {
        let mut pkt = Packet::new(PacketType::SERVER_RMZSHARD_STATE);
        let _ = pkt.write_u8(self.spawned);
        let _ = pkt.write_u16(self.id);
        let _ = pkt.write_u16(self.pos.0 as u16);
        let _ = pkt.write_u16(self.pos.1 as u16);
        ctx.broadcast(pkt, true);
        true
    }
}
