use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};
use crate::vote::TICKS_PER_SEC;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcState {
    None,
    Prepare,
}

pub struct NotPerfect {
    pub id: u16,
    pub state: NpcState,
    pub stage: u8,
    pub timer: f64,
    pub balls: bool,
}

impl NotPerfect {
    pub fn new() -> Self {
        Self { id: 0, state: NpcState::None, stage: 0, timer: 0.0, balls: false }
    }
}

impl Entity for NotPerfect {
    fn tag(&self) -> &'static str { "npctrl" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { (0.0, 0.0) }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        let cfg = crate::config::cfg();
        let gameplay = &cfg.states.gameplay;

        if ctx.game_time_sec <= gameplay.ring_appearance_timer as u16 && !self.balls {
            self.timer = 5.0 * TICKS_PER_SEC;
            self.state = NpcState::Prepare;
            self.balls = true;
        }

        match self.state {
            NpcState::None => {
                let disable_timer = gameplay.banana.disable_timer;
                let switch_interval = if ctx.game_time_sec < gameplay.ring_appearance_timer as u16 && !disable_timer {
                    gameplay.entities_misc.map_specific.not_perfect.switch_warning_timer_chase as f64
                } else {
                    gameplay.entities_misc.map_specific.not_perfect.switch_warning_timer as f64
                };

                if self.timer >= switch_interval * TICKS_PER_SEC {
                    let mut pkt = Packet::new(PacketType::SERVER_NPCONTROLLER_STATE);
                    let _ = pkt.write_u8(0);
                    let _ = pkt.write_u8(0);
                    let _ = pkt.write_u8(0);
                    ctx.broadcast(pkt, true);

                    self.state = NpcState::Prepare;
                    self.timer = 0.0;
                }
            }

            NpcState::Prepare => {
                let disable_timer = gameplay.banana.disable_timer;
                let warning_interval = if ctx.game_time_sec < gameplay.ring_appearance_timer as u16 && !disable_timer {
                    gameplay.entities_misc.map_specific.not_perfect.switch_timer_chase as f64
                } else {
                    gameplay.entities_misc.map_specific.not_perfect.switch_timer as f64
                };

                if self.timer >= warning_interval * TICKS_PER_SEC {
                    self.stage = self.stage.wrapping_add(1);

                    let prev = if self.stage == 0 { 0u8 } else { self.stage - 1 };
                    let mut pkt = Packet::new(PacketType::SERVER_NPCONTROLLER_STATE);
                    let _ = pkt.write_u8(1);
                    let _ = pkt.write_u8(self.stage % 4);
                    let _ = pkt.write_u8(prev % 4);
                    ctx.broadcast(pkt, true);

                    self.state = NpcState::None;
                    self.timer = 0.0;
                }
            }
        }

        self.timer += 1.0;
        true
    }
}
