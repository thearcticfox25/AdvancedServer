use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};

pub struct Ring {
    pub id: u16,
    pub rid: u8,
    pub red: bool,
}

impl Ring {
    pub fn new() -> Self {
        Self { id: 0, rid: 0, red: false }
    }
}

impl Entity for Ring {
    fn tag(&self) -> &'static str { "ring" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { (0.0, 0.0) }

    fn is_red(&self) -> bool { self.red }

    fn on_init(&mut self, ctx: &mut EntityCtx) -> bool {
        use rand::Rng;
        let ring_count = ctx.map_ring_count as usize;


        let cnt = ctx.rings[..ring_count].iter().filter(|&&r| r).count();
        if cnt >= ring_count {
            return false;
        }


        let mut rnd;
        loop {
            rnd = ctx.rand.gen_range(0..ring_count);
            if !ctx.rings[rnd] { break; }
        }

        ctx.rings[rnd] = true;
        self.rid = rnd as u8;
        let cfg = crate::config::cfg();
        self.red = ctx.rand.gen_range(0..100) < cfg.states.gameplay.entities_misc.global.rings.red_ring_chance;

        let mut pkt = Packet::new(PacketType::SERVER_RING_STATE);
        let _ = pkt.write_u8(0);
        let _ = pkt.write_u8(self.rid);
        let _ = pkt.write_u16(self.id);
        let _ = pkt.write_u8(self.red as u8);
        ctx.broadcast(pkt, true);
        true
    }

    fn on_uninit(&mut self, ctx: &mut EntityCtx) {
        ctx.rings[self.rid as usize] = false;

        let mut pkt = Packet::new(PacketType::SERVER_RING_STATE);
        let _ = pkt.write_u8(1);
        let _ = pkt.write_u8(self.rid);
        let _ = pkt.write_u16(self.id);
        ctx.broadcast(pkt, true);
    }
}
