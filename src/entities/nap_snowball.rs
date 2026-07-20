use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};
use crate::vote::TICKS_PER_SEC;

const NUM_FRAMES: f64 = 32.0;
const ROLL_START: f64 = 16.0;

pub struct NapSnowball {
    pub id: u16,
    pub sid: u8,
    pub active: bool,
    pub state: usize,
    pub stage_prog: f64,
    pub dir: i8,
    pub frame: f64,
    pub vel: f64,
    pub p_count: usize,
    pub timer: f64,
    pub p_move: [f32; 20],
    pub p_anim: [f32; 20],
}

impl NapSnowball {
    pub fn new(sid: u8, p_count: usize, dir: i8) -> Self {
        let mut sb = Self {
            id: 0,
            sid,
            active: false,
            state: 0,
            stage_prog: 0.0,
            dir,
            frame: 0.0,
            vel: 0.0,
            p_count,
            timer: 0.0,
            p_move: [0.0; 20],
            p_anim: [0.0; 20],
        };

        for i in 0..20 {
            sb.p_move[i] = 0.05;
            sb.p_anim[i] = 0.35;
        }
        sb
    }

    pub fn activate(&mut self, ctx: &mut EntityCtx) {
        if self.active {
            return;
        }

        self.vel = 0.0;
        self.state = 0;
        self.stage_prog = 0.0;
        self.frame = 0.0;
        self.active = true;

        let mut pkt = Packet::new(PacketType::SERVER_NAPBALL_STATE);
        let _ = pkt.write_u8(0);
        let _ = pkt.write_u8(self.sid);
        let _ = pkt.write_u8(self.dir as u8);
        ctx.broadcast(pkt, true);
    }
}

impl Entity for NapSnowball {
    fn tag(&self) -> &'static str { "snowball" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { (0.0, 0.0) }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        if self.timer < 20.0 * TICKS_PER_SEC {
            self.timer += 1.0;
        } else {
            self.activate(ctx);
            self.timer = 0.0;
        }

        if self.active {
            if self.vel > 1.0 {
                self.frame += self.p_anim[self.state] as f64;
                self.stage_prog += self.p_move[self.state] as f64;

                if self.frame >= NUM_FRAMES {
                    self.frame = ROLL_START;
                }
            } else {
                self.vel += 0.016;
                self.frame += self.vel * 0.45;
                self.stage_prog += self.vel * 0.05;
            }

            if self.stage_prog > 1.0 {
                self.stage_prog = 0.0;
                self.state += 1;

                if self.state >= self.p_count.saturating_sub(1) {
                    self.state = 0;
                    self.stage_prog = 0.0;
                    self.active = false;
                    self.frame = 0.0;

                    let mut pkt = Packet::new(PacketType::SERVER_NAPBALL_STATE);
                    let _ = pkt.write_u8(2);
                    let _ = pkt.write_u8(self.sid);
                    ctx.broadcast(pkt, true);

                    return true;
                }
            }

            let mut pkt = Packet::new(PacketType::SERVER_NAPBALL_STATE);
            let _ = pkt.write_u8(1);
            let _ = pkt.write_u8(self.sid);
            let _ = pkt.write_u8(self.state as u8);
            let _ = pkt.write_u8(self.frame as u8);
            let _ = pkt.write_f64(self.stage_prog);
            ctx.broadcast(pkt, false);
        }

        true
    }
}
