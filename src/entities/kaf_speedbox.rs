use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};
use crate::vote::TICKS_PER_SEC;
use rand::Rng;

pub struct KafSpeedbox {
    pub id: u16,
    pub box_id: u8,
    pub timer: f64,
    pub activated: bool,
}

impl KafSpeedbox {
    pub fn new(box_id: u8) -> Self {
        Self { id: 0, box_id, timer: 0.0, activated: false }
    }

    pub fn activate(&mut self, ctx: &mut EntityCtx, player_id: u16, is_proj: bool) {
        if self.activated {
            return;
        }

        let cfg = crate::config::cfg();
        let speedbox = &cfg.states.gameplay.entities_misc.map_specific.kind_and_fair.speedbox;
        let offset = if speedbox.timer_offset == 0 {
            0u64
        } else {
            ctx.rand.gen_range(0u64..speedbox.timer_offset as u64)
        };

        let mut pkt = Packet::new(PacketType::SERVER_KAFMONITOR_STATE);
        let _ = pkt.write_u8(2);
        let _ = pkt.write_u8(self.box_id);
        let _ = pkt.write_u16(if is_proj { 0 } else { player_id });
        ctx.broadcast(pkt, true);

        self.activated = true;
        self.timer = (speedbox.timer as f64 + offset as f64) * TICKS_PER_SEC;
    }
}

impl Entity for KafSpeedbox {
    fn tag(&self) -> &'static str { "kafbox" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { (0.0, 0.0) }
    fn kaf_nid(&self) -> i16 { self.box_id as i16 }
    fn kaf_activate(&mut self, ctx: &mut EntityCtx, player_id: u16, is_proj: bool) { self.activate(ctx, player_id, is_proj); }

    fn on_init(&mut self, ctx: &mut EntityCtx) -> bool {
        let mut pkt = Packet::new(PacketType::SERVER_KAFMONITOR_STATE);
        let _ = pkt.write_u8(0);
        let _ = pkt.write_u8(self.box_id);
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
        let _ = pkt.write_u8(self.box_id);
        ctx.broadcast(pkt, true);

        self.activated = false;
        true
    }
}
