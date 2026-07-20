use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};
use crate::vote::TICKS_PER_SEC;
use rand::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YccState {
    None,
    Some,
}

pub struct YouCantRun {
    pub id: u16,
    pub state: YccState,
    pub timer: f64,
    pub smoke_id: u8,
    pub activated: u8,
}

impl YouCantRun {
    pub fn new() -> Self {
        Self { id: 0, state: YccState::None, timer: 0.0, smoke_id: 0, activated: 0 }
    }
}

impl Entity for YouCantRun {
    fn tag(&self) -> &'static str { "ycrctrl" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { (0.0, 0.0) }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        let cfg = crate::config::cfg();

        match self.state {
            YccState::None => {
                if self.timer >= TICKS_PER_SEC {
                    let mut pkt = Packet::new(PacketType::SERVER_YCRSMOKE_READY);
                    let _ = pkt.write_u8(self.smoke_id);
                    ctx.broadcast(pkt, true);

                    self.state = YccState::Some;
                }
            }

            YccState::Some => {
                let delay = cfg.states.gameplay.entities_misc.map_specific.you_cant_run.gas.delay as f64 * TICKS_PER_SEC;
                if self.timer >= delay {
                    self.state = YccState::None;
                    self.timer = 0.0;
                    self.activated = if self.activated == 0 { 1 } else { 0 };

                    if self.activated != 0 {
                        self.smoke_id = ctx.rand.gen_range(0u8..7);
                    } else {
                        self.smoke_id = 0;
                    }

                    let mut pkt = Packet::new(PacketType::SERVER_YCRSMOKE_STATE);
                    let _ = pkt.write_u8(self.activated);
                    let _ = pkt.write_u8(self.smoke_id);
                    ctx.broadcast(pkt, true);
                }
            }
        }

        self.timer += 1.0;
        true
    }
}
