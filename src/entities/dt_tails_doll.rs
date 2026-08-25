use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};
use crate::server::OutboxMsg;
use crate::vote::TICKS_PER_SEC;
use rand::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TdState {
    None   = 0,
    Ready  = 1,
    Follow = 2,
    Reloc  = 3,
}

const SPOTS: [(f32, f32); 11] = [
    (177.0,  944.0),
    (1953.0, 544.0),
    (3279.0, 224.0),
    (4101.0, 544.0),
    (4060.0, 1264.0),
    (3805.0, 1824.0),
    (2562.0, 1584.0),
    (515.0,  1824.0),
    (2115.0, 1056.0),
    (984.0,  1184.0),
    (1498.0, 1504.0),
];

const PLAYER_ESCAPED:   u8 = 0x1 << 0;
const PLAYER_DEAD:      u8 = 0x1 << 1;
const PLAYER_DEMONIZED: u8 = 0x1 << 2;

pub struct DtTailsDoll {
    pub id: u16,
    pub pos: (f32, f32),
    pub state: TdState,
    pub target: i32,
    pub timer: f64,
    pub velx: f64,
    pub vely: f64,
}

impl DtTailsDoll {
    pub fn new() -> Self {
        Self {
            id: 0,
            pos: (0.0, 0.0),
            state: TdState::None,
            target: -1,
            timer: 0.0,
            velx: 0.0,
            vely: 0.0,
        }
    }

    fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
        let dx = a.0 - b.0;
        let dy = a.1 - b.1;
        (dx * dx + dy * dy).sqrt()
    }

    fn find_spot(&mut self, ctx: &mut EntityCtx) {
        let mut valid: Vec<(f32, f32)> = Vec::new();
        for &spot in &SPOTS {
            let can_use = ctx.ingame_peers.iter().all(|(_, pos, _, _, _, _)| {
                Self::dist(spot, *pos) >= 480.0
            });
            if can_use {
                valid.push(spot);
            }
        }

        if !valid.is_empty() {
            let idx = ctx.rand.gen_range(0..valid.len());
            self.pos = valid[idx];
            log::debug!("Tails doll found spot {} at {} {}", idx, self.pos.0, self.pos.1);
        } else {
            let idx = ctx.rand.gen_range(0..SPOTS.len());
            self.pos = SPOTS[idx];
            log::debug!("Tails doll didn't find a spot, using {} {}", self.pos.0, self.pos.1);
        }
    }

    fn is_valid_target(&self, ctx: &EntityCtx) -> bool {
        if self.target < 0 { return false; }
        let tid = self.target as u16;
        ctx.ingame_peers.iter().any(|(id, _, flags, _, _, _)| {
            *id == tid
                && *id != ctx.exe_id as u16
                && flags & PLAYER_DEAD == 0
                && flags & PLAYER_DEMONIZED == 0
                && flags & PLAYER_ESCAPED == 0
        })
    }

    fn find_target(&mut self, ctx: &EntityCtx) -> bool {
        for (id, pos, flags, _, _, _) in &ctx.ingame_peers {
            if *id == ctx.exe_id as u16 { continue; }
            if flags & PLAYER_DEAD != 0 { continue; }
            if flags & PLAYER_DEMONIZED != 0 { continue; }
            if flags & PLAYER_ESCAPED != 0 { continue; }

            if Self::dist(self.pos, *pos) < 130.0 {
                self.target = *id as i32;
                return true;
            }
        }
        false
    }

    fn sign(v: f64) -> f64 {
        if v > 0.0 { 1.0 } else if v < 0.0 { -1.0 } else { 0.0 }
    }
}

impl Entity for DtTailsDoll {
    fn tag(&self) -> &'static str { "tdoll" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { self.pos }

    fn on_init(&mut self, ctx: &mut EntityCtx) -> bool {
        self.find_spot(ctx);

        let mut pkt = Packet::new(PacketType::SERVER_DTTAILSDOLL_STATE);
        let _ = pkt.write_u8(0);
        let _ = pkt.write_u16(self.pos.0 as u16);
        let _ = pkt.write_u16(self.pos.1 as u16);
        let _ = pkt.write_u8(self.state as u8);
        ctx.broadcast(pkt, true);
        true
    }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        match self.state {
            TdState::None => {
                if self.find_target(ctx) {
                    let extra = ctx.rand.gen_range(0u32..2) as f64;
                    self.timer = (1.0 + extra * 0.5) * TICKS_PER_SEC;
                    self.state = TdState::Ready;

                    if self.target >= 0 {
                        let mut pkt = Packet::new(PacketType::SERVER_DTTAILSDOLL_STATE);
                        let _ = pkt.write_u8(2);

                        // Marked/pounce/caught cues are private to the hunted player --
                        // broadcasting them would out who's being stalked.
                        ctx.outbox.push(OutboxMsg::SendTo(self.target as u16, pkt.data().to_vec(), true));
                    }
                }
            }

            TdState::Ready => {
                self.timer -= 1.0;
                if self.timer <= 0.0 {
                    if self.target >= 0 {
                        let mut pkt = Packet::new(PacketType::SERVER_DTTAILSDOLL_STATE);
                        let _ = pkt.write_u8(3);
                        ctx.outbox.push(OutboxMsg::SendTo(self.target as u16, pkt.data().to_vec(), true));
                    }
                    self.state = TdState::Follow;
                }
            }

            TdState::Follow => {
                self.find_target(ctx);

                if !self.is_valid_target(ctx) {
                    self.state = TdState::Reloc;
                } else if let Some((_, target_pos, _, _, _, _)) = ctx.ingame_peers.iter()
                    .find(|(id, _, _, _, _, _)| *id == self.target as u16)
                    .copied()
                {
                    let dx = (target_pos.0 - self.pos.0) as i32;
                    let dy = (target_pos.1 - self.pos.1) as i32;

                    if dx.abs() >= 4 {
                        self.velx += Self::sign(dx as f64) * 0.512;
                        self.velx = self.velx.clamp(-5.0, 5.0);
                    }
                    if dy.abs() >= 5 {
                        self.vely += Self::sign(dy as f64) * 0.480;
                        self.vely = self.vely.clamp(-5.0, 5.0);
                    }

                    self.pos.0 += self.velx as f32;
                    self.pos.1 += self.vely as f32;

                    if Self::dist(self.pos, target_pos) < 12.0 {
                        if self.target >= 0 {
                            let mut pkt = Packet::new(PacketType::SERVER_DTTAILSDOLL_STATE);
                            let _ = pkt.write_u8(1);
                            ctx.outbox.push(OutboxMsg::SendTo(self.target as u16, pkt.data().to_vec(), true));
                        }
                        self.state = TdState::Reloc;
                    }
                } else {
                    self.state = TdState::Reloc;
                }
            }

            TdState::Reloc => {
                self.find_spot(ctx);
                self.state = TdState::None;
            }
        }

        let mut pkt = Packet::new(PacketType::SERVER_DTTAILSDOLL_STATE);
        let _ = pkt.write_u16(self.pos.0 as u16);
        let _ = pkt.write_u16(self.pos.1 as u16);
        let _ = pkt.write_u8(self.state as u8);
        ctx.broadcast(pkt, false);

        true
    }
}
