use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};
use crate::vote::TICKS_PER_SEC;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LccState {
    None,
    Prepare,
    Activate,
}

pub struct LcChain {
    pub id: u16,
    pub timer: f64,
    pub state: LccState,
}

impl LcChain {
    pub fn new() -> Self {
        Self { id: 0, timer: 0.0, state: LccState::None }
    }
}

impl Entity for LcChain {
    fn tag(&self) -> &'static str { "lcchain" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { (0.0, 0.0) }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        let cfg = crate::config::cfg();
        let chain = &cfg.states.gameplay.entities_misc.map_specific.limb_city.chain;

        match self.state {
            LccState::None => {
                if self.timer >= chain.delay as f64 * TICKS_PER_SEC {
                    let mut pkt = Packet::new(PacketType::SERVER_LCCHAIN_STATE);
                    let _ = pkt.write_u8(0);
                    ctx.broadcast(pkt, true);

                    self.timer = 0.0;
                    self.state = LccState::Prepare;
                }
            }

            LccState::Prepare => {
                if self.timer >= chain.warning as f64 * TICKS_PER_SEC {
                    let mut pkt = Packet::new(PacketType::SERVER_LCCHAIN_STATE);
                    let _ = pkt.write_u8(1);
                    ctx.broadcast(pkt, true);

                    self.timer = 0.0;
                    self.state = LccState::Activate;
                }
            }

            LccState::Activate => {
                if self.timer >= chain.shocking_time as f64 * TICKS_PER_SEC {
                    let mut pkt = Packet::new(PacketType::SERVER_LCCHAIN_STATE);
                    let _ = pkt.write_u8(2);
                    ctx.broadcast(pkt, true);

                    self.timer = 0.0;
                    self.state = LccState::None;
                }
            }
        }

        self.timer += 1.0;
        true
    }
}
