use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};

pub struct DtBall {
    pub id: u16,
    pub state: f64,
    pub side: u8,
}

impl DtBall {
    pub fn new() -> Self {
        Self { id: 0, state: 0.0, side: 0 }
    }
}

impl Entity for DtBall {
    fn tag(&self) -> &'static str { "dtball" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { (0.0, 0.0) }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        let cfg = crate::config::cfg();
        let shift = cfg.states.gameplay.entities_misc.map_specific.dark_tower.balls.shift_per_tick;

        if self.side != 0 {
            self.state += shift;
            if self.state >= 1.0 {
                self.side = 0;
            }
        } else {
            self.state -= shift;
            if self.state <= -1.0 {
                self.side = 1;
            }
        }

        let mut pkt = Packet::new(PacketType::SERVER_DTBALL_STATE);
        let _ = pkt.write_f32(self.state as f32);
        ctx.broadcast(pkt, false);

        true
    }
}
