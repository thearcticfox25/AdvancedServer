use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};
use crate::vote::TICKS_PER_SEC;
use rand::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MlvState {
    Idle     = 0,
    MoveDown = 1,
    Raise    = 2,
    Move     = 3,
    Lower    = 4,
}

pub struct MjLava {
    pub id: u16,
    pub state: MlvState,
    pub timer: f64,
    pub pos_y: f32,
    pub start: f32,
    pub dist: f32,
    pub vel: f32,
}

impl MjLava {
    pub fn new(start: f32, dist: f32) -> Self {
        Self {
            id: 0,
            state: MlvState::Idle,
            timer: 5.0 * TICKS_PER_SEC,
            pos_y: start,
            start,
            dist,
            vel: 0.0,
        }
    }
}

impl Entity for MjLava {
    fn tag(&self) -> &'static str { "mlava" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { (0.0, self.pos_y) }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        match self.state {
            MlvState::Idle => {
                self.pos_y = self.start + (self.timer as f32 / 25.0).sin() * 6.0;
                self.timer -= 1.0;
                if self.timer <= 0.0 {
                    self.state = MlvState::MoveDown;
                }
            }

            MlvState::MoveDown => {
                if self.pos_y < self.start + 20.0 {
                    self.pos_y += 0.15;
                } else {
                    self.state = MlvState::Raise;
                }
            }

            MlvState::Raise => {
                if self.pos_y > self.start - self.dist {
                    self.pos_y -= self.vel;
                    if self.vel < 5.0 {
                        self.vel += 0.08;
                    } else {
                        self.vel = 5.0;
                    }
                } else {
                    self.state = MlvState::Move;
                    let extra = ctx.rand.gen_range(0u32..3) as f64;
                    self.timer = (4.0 + extra) * TICKS_PER_SEC;
                    self.vel = 0.0;
                }
            }

            MlvState::Move => {
                self.pos_y = (self.start - self.dist) + (self.timer as f32 / 25.0).sin() * 6.0;
                self.timer -= 1.0;
                if self.timer <= 0.0 {
                    self.state = MlvState::Lower;
                }
            }

            MlvState::Lower => {
                if self.start > self.pos_y {
                    self.pos_y += self.vel;
                    if self.vel < 5.0 {
                        self.vel += 0.08;
                    } else {
                        self.vel = 5.0;
                    }
                } else {
                    self.state = MlvState::Idle;
                    let extra = ctx.rand.gen_range(0u32..4) as f64;
                    self.timer = (5.0 + extra) * TICKS_PER_SEC;
                    self.vel = 0.0;
                }
            }
        }

        let mut pkt = Packet::new(PacketType::SERVER_MJLAVA_STATE);
        let _ = pkt.write_u8(self.state as u8);
        let _ = pkt.write_f32(self.pos_y);
        ctx.broadcast(pkt, false);

        true
    }
}
