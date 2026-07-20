use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};
use crate::vote::TICKS_PER_SEC;

pub struct NapIce {
    pub id: u16,
    pub iid: u8,
    pub activated: bool,
    pub timer: f64,
}

impl NapIce {
    pub fn new(iid: u8) -> Self {
        Self { id: 0, iid, activated: false, timer: 0.0 }
    }

    pub fn activate(&mut self, ctx: &mut EntityCtx) {
        if self.activated {
            return;
        }

        let cfg = crate::config::cfg();
        self.timer = cfg.states.gameplay.entities_misc.map_specific.nasty_paradise.ice.regeneration_timer as f64 * TICKS_PER_SEC;
        self.activated = true;

        let mut pkt = Packet::new(PacketType::SERVER_NAPICE_STATE);
        let _ = pkt.write_u8(0);
        let _ = pkt.write_u8(self.iid);
        ctx.broadcast(pkt, true);
    }
}

impl Entity for NapIce {
    fn tag(&self) -> &'static str { "ice" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { (0.0, 0.0) }
    fn nap_iid(&self) -> i16 { self.iid as i16 }
    fn nap_activate(&mut self, ctx: &mut EntityCtx) { self.activate(ctx); }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        if !self.activated {
            return true;
        }

        self.timer -= 1.0;
        if self.timer <= 0.0 {
            let cfg = crate::config::cfg();
            self.timer = cfg.states.gameplay.entities_misc.map_specific.nasty_paradise.ice.regeneration_timer as f64 * TICKS_PER_SEC;
            self.activated = false;

            let mut pkt = Packet::new(PacketType::SERVER_NAPICE_STATE);
            let _ = pkt.write_u8(1);
            let _ = pkt.write_u8(self.iid);
            ctx.broadcast(pkt, true);
        }

        true
    }
}
