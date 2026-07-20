use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};
use crate::vote::TICKS_PER_SEC;
use rand::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MjjState {
    Wait    = 0,
    Prepare = 1,
    Fire    = 2,
}

pub struct MjJudger {
    pub id: u16,
    pub next_time: f64,
    pub timer: f64,
    pub state: MjjState,
}

impl MjJudger {
    pub fn new() -> Self {

        Self {
            id: 0,
            next_time: 10.0 * TICKS_PER_SEC,
            timer: 0.0,
            state: MjjState::Wait,
        }
    }
}

impl Entity for MjJudger {
    fn tag(&self) -> &'static str { "mjud" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { (0.0, 0.0) }

    fn on_init(&mut self, ctx: &mut EntityCtx) -> bool {
        let extra = ctx.rand.gen_range(0u32..5) as f64;
        self.next_time = (10.0 + extra) * TICKS_PER_SEC;
        true
    }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        if self.timer >= self.next_time {
            self.state = match self.state {
                MjjState::Wait => {
                    self.next_time = 2.0 * TICKS_PER_SEC;
                    MjjState::Prepare
                }
                MjjState::Prepare => {
                    let extra = ctx.rand.gen_range(0u32..2) as f64;
                    self.next_time = (7.0 + extra) * TICKS_PER_SEC;
                    MjjState::Fire
                }
                MjjState::Fire => {
                    let extra = ctx.rand.gen_range(0u32..5) as f64;
                    self.next_time = (10.0 + extra) * TICKS_PER_SEC;
                    MjjState::Wait
                }
            };

            let mut pkt = Packet::new(PacketType::SERVER_MJJUDGER_STATE);
            let _ = pkt.write_u8(self.state as u8);
            ctx.broadcast(pkt, true);

            self.timer = 0.0;
        }

        self.timer += 1.0;
        true
    }
}
