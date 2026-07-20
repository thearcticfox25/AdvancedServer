use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};
use crate::vote::TICKS_PER_SEC;
use rand::Rng;

pub struct TcAcid {
    pub id: u16,
    pub acid_id: u8,
    pub activated: u8,
    pub timer: f64,
}

impl TcAcid {
    pub fn new() -> Self {
        Self { id: 0, acid_id: 0, activated: 0, timer: 0.0 }
    }
}

impl Entity for TcAcid {
    fn tag(&self) -> &'static str { "acid" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { (0.0, 0.0) }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        let cfg = crate::config::cfg();
        let delay = cfg.states.gameplay.entities_misc.map_specific.torture_cave.acid.delay as f64 * TICKS_PER_SEC;

        if self.timer >= delay {
            self.timer = 0.0;
            self.activated = if self.activated == 0 { 1 } else { 0 };

            if self.activated != 0 {
                self.acid_id = ctx.rand.gen_range(0u8..7);
            }

            let mut pkt = Packet::new(PacketType::SERVER_TCGOM_STATE);
            let _ = pkt.write_u8(self.acid_id);
            let _ = pkt.write_u8(self.activated);
            ctx.broadcast(pkt, true);
        }

        self.timer += 1.0;
        true
    }
}
