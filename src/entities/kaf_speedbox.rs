use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};
use crate::vote::TICKS_PER_SEC;
use rand::Rng;

pub struct KafSpeedbox {
    pub id: u16,
    pub nid: u8,
    pub timer: f64,
    pub activated: bool,
}

impl KafSpeedbox {
    pub fn new(nid: u8) -> Self {
        Self { id: 0, nid, timer: 0.0, activated: false }
    }

    pub fn activate(&mut self, ctx: &mut EntityCtx, pid: u16, is_proj: bool) {
        if self.activated {
            return;
        }

        let cfg = crate::config::cfg();
        let sb = &cfg.states.gameplay.entities_misc.map_specific.kind_and_fair.speedbox;
        let offset = if sb.timer_offset == 0 {
            0u64
        } else {
            ctx.rand.gen_range(0u64..sb.timer_offset as u64)
        };

        let mut pkt = Packet::new(PacketType::SERVER_KAFMONITOR_STATE);
        let _ = pkt.write_u8(2);
        let _ = pkt.write_u8(self.nid);
        let _ = pkt.write_u16(if is_proj { 0 } else { pid });
        ctx.broadcast(pkt, true);

        self.activated = true;
        self.timer = (sb.timer as f64 + offset as f64) * TICKS_PER_SEC;
    }
}

impl Entity for KafSpeedbox {
    fn tag(&self) -> &'static str { "kafbox" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { (0.0, 0.0) }
    fn kaf_nid(&self) -> i16 { self.nid as i16 }
    fn kaf_activate(&mut self, ctx: &mut EntityCtx, pid: u16, is_proj: bool) { self.activate(ctx, pid, is_proj); }

    fn on_init(&mut self, ctx: &mut EntityCtx) -> bool {
        let mut pkt = Packet::new(PacketType::SERVER_KAFMONITOR_STATE);
        let _ = pkt.write_u8(0);
        let _ = pkt.write_u8(self.nid);
        ctx.broadcast(pkt, true);
        true
    }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        if !self.activated {
            return true;
        }

        if self.timer > 0.0 {
            self.timer -= 1.0;
            return true;
        }

        let mut pkt = Packet::new(PacketType::SERVER_KAFMONITOR_STATE);
        let _ = pkt.write_u8(1);
        let _ = pkt.write_u8(self.nid);
        ctx.broadcast(pkt, true);

        self.activated = false;
        true
    }
}
