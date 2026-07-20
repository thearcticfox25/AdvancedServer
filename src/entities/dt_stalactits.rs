use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};
use crate::vote::TICKS_PER_SEC;
use rand::Rng;

pub struct DtStalactits {
    pub id: u16,
    pub sid: u8,
    pub state: bool,
    pub show: bool,
    pub sx: u16,
    pub sy: u16,
    pub pos: (f32, f32),
    pub timer: f64,
    pub vel: f32,
}

impl DtStalactits {

    pub fn new(sid: u8, x: u16, y: u16) -> Self {
        Self {
            id: 0,
            sid,
            state: false,
            show: true,
            sx: x,
            sy: y,
            pos: (x as f32, y as f32),
            timer: 0.0,
            vel: 0.0,
        }
    }
}

impl Entity for DtStalactits {
    fn tag(&self) -> &'static str { "dttits" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { self.pos }
    fn dt_sid(&self) -> i16 { self.sid as i16 }
    fn dt_activate(&mut self, ctx: &mut EntityCtx) { self.activate(ctx); }

    fn on_init(&mut self, ctx: &mut EntityCtx) -> bool {
        let mut pkt = Packet::new(PacketType::SERVER_DTASS_STATE);
        let _ = pkt.write_u8(0);
        let _ = pkt.write_u8(self.sid);
        let _ = pkt.write_u16(self.pos.0 as u16);
        let _ = pkt.write_u16(self.pos.1 as u16);
        ctx.broadcast(pkt, true);
        true
    }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        let cfg = crate::config::cfg();

        if self.state {

            self.vel += cfg.states.gameplay.entities_misc.map_specific.dark_tower.stalactites.acceleration as f32;
            self.pos.1 += self.vel;

            let mut pkt = Packet::new(PacketType::SERVER_DTASS_STATE);
            let _ = pkt.write_u8(self.sid);
            let _ = pkt.write_u16(self.pos.0 as u16);
            let _ = pkt.write_u16(self.pos.1 as u16);
            ctx.broadcast(pkt, false);
        } else {
            if !self.show {
                if self.timer > TICKS_PER_SEC {
                    self.timer -= 1.0;
                }

                if self.timer <= TICKS_PER_SEC {
                    let mut pkt = Packet::new(PacketType::SERVER_DTASS_STATE);
                    let _ = pkt.write_u8(0);
                    let _ = pkt.write_u8(self.sid);
                    let _ = pkt.write_u16(self.pos.0 as u16);
                    let _ = pkt.write_u16(self.pos.1 as u16);
                    ctx.broadcast(pkt, true);

                    self.show = true;
                }
            } else if self.timer > 0.0 {
                self.timer -= 1.0;
            }

            if self.timer <= 0.0 {

                for (_, pos, flags, _, _, _) in &ctx.ingame_peers {
                    const PLAYER_DEAD: u8 = 0x1 << 1;
                    if flags & PLAYER_DEAD != 0 {
                        continue;
                    }

                    let dist_y = pos.1 - self.pos.1;
                    if dist_y > 0.0 && dist_y <= 336.0
                        && pos.0 >= self.pos.0
                        && pos.0 <= self.pos.0 + 80.0
                    {
                        self.vel = 0.0;

                        let mut pkt = Packet::new(PacketType::SERVER_DTASS_STATE);
                        let _ = pkt.write_u8(2);
                        let _ = pkt.write_u8(self.sid);
                        ctx.broadcast(pkt, true);

                        self.state = true;
                        break;
                    }
                }
            }
        }

        true
    }
}

impl DtStalactits {

    pub fn activate(&mut self, ctx: &mut EntityCtx) {
        if !self.state {
            return;
        }

        let cfg = crate::config::cfg();
        let st = &cfg.states.gameplay.entities_misc.map_specific.dark_tower.stalactites;
        let offset = if st.timer_offset == 0 {
            0u64
        } else {
            ctx.rand.gen_range(0..st.timer_offset as u64)
        };

        self.show = false;
        self.state = false;
        self.timer = (st.timer as f64 + offset as f64) * TICKS_PER_SEC;
        self.pos.1 = self.sy as f32;
        self.vel = 0.0;

        let mut pkt = Packet::new(PacketType::SERVER_DTASS_STATE);
        let _ = pkt.write_u8(1);
        let _ = pkt.write_u8(self.sid);
        ctx.broadcast(pkt, true);
    }
}
