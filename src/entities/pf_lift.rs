use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};
use crate::vote::TICKS_PER_SEC;

pub struct PfLift {
    pub id: u16,
    pub pos_y: f32,
    pub lift_id: u8,
    pub timer: f64,
    pub start: f32,
    pub end: f32,
    pub speed: f32,
    pub activator: u16,
    pub activated: bool,
}

impl PfLift {
    pub fn new(lift_id: u8, start: f32, end: f32) -> Self {
        Self {
            id: 0,
            pos_y: start,
            lift_id,
            timer: 0.0,
            start,
            end,
            speed: 0.0,
            activator: 0,
            activated: false,
        }
    }

    pub fn activate(&mut self, ctx: &mut EntityCtx, activator_id: u16) {
        if self.activated || self.timer > 0.0 {
            return;
        }

        self.activator = activator_id;
        self.timer = 0.0;
        self.speed = 0.0;
        self.pos_y = self.start;
        self.activated = true;

        let mut pkt = Packet::new(PacketType::SERVER_PFLIFT_STATE);
        let _ = pkt.write_u8(0);
        let _ = pkt.write_u8(self.lift_id);
        let _ = pkt.write_u16(self.activator);
        ctx.broadcast(pkt, true);
    }
}

impl Entity for PfLift {
    fn tag(&self) -> &'static str { "pflift" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { (0.0, self.pos_y) }
    fn pf_lid(&self) -> i16 { self.lift_id as i16 }
    fn pf_activate(&mut self, ctx: &mut EntityCtx, player_id: u16) { self.activate(ctx, player_id); }

    fn on_init(&mut self, _ctx: &mut EntityCtx) -> bool {
        self.pos_y = self.start;
        true
    }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        if !self.activated {
            if self.timer > 0.0 {
                self.timer -= 1.0;
                if self.timer <= 0.0 {
                    let mut pkt = Packet::new(PacketType::SERVER_PFLIFT_STATE);
                    let _ = pkt.write_u8(3);
                    let _ = pkt.write_u8(self.lift_id);
                    let _ = pkt.write_u16(self.start as u16);
                    ctx.broadcast(pkt, true);
                }
            }
            return true;
        }

        if self.pos_y > self.end {
            if self.speed < 7.0 {
                self.speed += 0.052;
            }
            self.pos_y -= self.speed;
        } else {
            let mut pkt = Packet::new(PacketType::SERVER_PFLIFT_STATE);
            let _ = pkt.write_u8(2);
            let _ = pkt.write_u8(self.lift_id);
            let _ = pkt.write_u16(self.activator);
            let _ = pkt.write_u16(self.pos_y as u16);
            ctx.broadcast(pkt, true);

            self.timer = (1.5 * TICKS_PER_SEC) as f64;
            self.activated = false;
            self.activator = 0;
        }

        let mut pkt = Packet::new(PacketType::SERVER_PFLIFT_STATE);
        let _ = pkt.write_u8(1);
        let _ = pkt.write_u8(self.lift_id);
        let _ = pkt.write_u16(self.activator);
        let _ = pkt.write_u16(self.pos_y as u16);
        ctx.broadcast(pkt, false);

        true
    }
}
