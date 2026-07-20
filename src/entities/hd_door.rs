use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};
use crate::vote::TICKS_PER_SEC;

pub struct HdDoor {
    pub id: u16,
    pub state: u8,
    pub timer: f64,
}

impl HdDoor {
    pub fn new() -> Self {
        Self { id: 0, state: 0, timer: 0.0 }
    }

    pub fn toggle(&mut self, ctx: &mut EntityCtx) -> bool {
        if self.timer > 0.0 {
            return false;
        }

        let cfg = crate::config::cfg();
        self.state = if self.state == 0 { 1 } else { 0 };
        self.timer = cfg.states.gameplay.entities_misc.map_specific.haunting_dream.doors.toggle_delay as f64 * TICKS_PER_SEC;

        let mut pkt = Packet::new(PacketType::SERVER_HDDOOR_STATE);
        let _ = pkt.write_u8(0);
        let _ = pkt.write_u8(self.state);
        ctx.broadcast(pkt, true);

        let mut pkt2 = Packet::new(PacketType::SERVER_HDDOOR_STATE);
        let _ = pkt2.write_u8(1);
        let _ = pkt2.write_u8(0);
        ctx.broadcast(pkt2, true);

        true
    }
}

impl Entity for HdDoor {
    fn tag(&self) -> &'static str { "hddoor" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { (0.0, 0.0) }
    fn hd_toggle(&mut self, ctx: &mut EntityCtx) { self.toggle(ctx); }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        if self.timer > 0.0 {
            self.timer -= 1.0;
            if self.timer <= 0.0 {
                let mut pkt = Packet::new(PacketType::SERVER_HDDOOR_STATE);
                let _ = pkt.write_u8(1);
                let _ = pkt.write_u8(1);
                ctx.broadcast(pkt, true);
            }
        }

        true
    }
}
