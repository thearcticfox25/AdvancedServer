use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};
use crate::vote::TICKS_PER_SEC;

pub struct LcEye {
    pub id: u16,
    pub eye_id: u8,
    pub use_id: u16,
    pub used: bool,
    pub charge: u8,
    pub target: u16,
    pub cooldown: f64,
    pub timer: f64,
}

impl LcEye {
    pub fn new(eye_id: u8) -> Self {
        Self {
            id: 0,
            eye_id,
            use_id: 0,
            used: false,
            charge: 100,
            target: 0,
            cooldown: 0.0,
            timer: 0.0,
        }
    }

    fn send_update(&self, ctx: &mut EntityCtx) {
        let mut pkt = Packet::new(PacketType::SERVER_LCEYE_STATE);
        let _ = pkt.write_u8(self.eye_id);
        let _ = pkt.write_u8(self.used as u8);
        let _ = pkt.write_u16(self.use_id);
        let _ = pkt.write_u8(self.target as u8);
        let _ = pkt.write_u8(self.charge);
        ctx.broadcast(pkt, true);
    }
}

impl Entity for LcEye {
    fn tag(&self) -> &'static str { "lceye" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { (0.0, 0.0) }
    fn lceye_nid(&self) -> i16 { self.eye_id as i16 }
    fn lceye_get(&self) -> (bool, u8) { (self.used, self.charge) }
    fn lceye_set_used(&mut self, used: bool, use_id: u16, target: u8, ctx: &mut EntityCtx) {
        self.used = used;
        self.use_id = use_id;
        self.target = target as u16;
        self.timer = 0.0;
        self.send_update(ctx);
    }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        if self.cooldown > 0.0 {
            self.cooldown -= 1.0;
            return true;
        }

        if self.timer >= TICKS_PER_SEC {
            let cfg = crate::config::cfg();
            let eye_cfg = &cfg.states.gameplay.entities_misc.map_specific.limb_city.eye;

            if self.used && self.charge > 0 {
                self.charge = self.charge.saturating_sub(eye_cfg.use_cost);
                if self.charge < 20 {
                    self.cooldown = eye_cfg.recharge_timer as f64 * TICKS_PER_SEC;
                    self.used = false;
                    self.timer = 0.0;
                }
                self.send_update(ctx);
            } else if !self.used && self.charge < 100 {
                self.charge = self.charge.saturating_add(eye_cfg.recharge_strength).min(100);
                self.send_update(ctx);
            }

            self.timer = 0.0;
        }

        self.timer += 1.0;
        true
    }
}
