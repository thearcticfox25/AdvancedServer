use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};
use std::time::Instant;

/// Erector black ring sprite is 30x30 with origin (0,0), so its collision box is
/// `[pos.x, pos.x+30] x [pos.y, pos.y+30]`. Map-placed rings arrive with a sentinel
/// position (server doesn't know their real coordinates), so they are never used for
/// the server-side contact check.
const MAP_BRING: f32 = 32767.0;

pub struct BlackRing {
    pub id: u16,
    pub pos: (f32, f32),
    spawn: Option<Instant>,
}

impl BlackRing {
    pub fn new(x: f32, y: f32) -> Self {
        Self { id: 0, pos: (x, y), spawn: None }
    }
}

impl Entity for BlackRing {
    fn tag(&self) -> &'static str { "bring" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { self.pos }

    fn bring_hit_pos(&self) -> Option<(f32, f32)> {
        if self.pos.0 == MAP_BRING || self.pos.1 == MAP_BRING {
            return None;
        }
        // Mirror the client's 0.5s fade-in before the ring can be collected, so a
        // survivor who legitimately leaves during the fade is never force-collected.
        match self.spawn {
            Some(spawned_at) if spawned_at.elapsed().as_millis() >= 500 => Some(self.pos),
            _ => None,
        }
    }

    fn on_init(&mut self, ctx: &mut EntityCtx) -> bool {
        self.spawn = Some(Instant::now());
        if self.pos.0 == MAP_BRING || self.pos.1 == MAP_BRING {
            let mut pkt = Packet::new(PacketType::SERVER_BRING_STATE);
            let _ = pkt.write_u8(0);
            let _ = pkt.write_u16(self.id);
            ctx.broadcast(pkt, true);
        } else {
            let mut pkt = Packet::new(PacketType::SERVER_ERECTOR_BRING_SPAWN);
            let _ = pkt.write_u16(self.id);
            let _ = pkt.write_u16(self.pos.0 as u16);
            let _ = pkt.write_u16(self.pos.1 as u16);
            ctx.broadcast(pkt, true);
        }
        true
    }

    fn on_uninit(&mut self, ctx: &mut EntityCtx) {
        let mut pkt = Packet::new(PacketType::SERVER_BRING_STATE);
        let _ = pkt.write_u8(1);
        let _ = pkt.write_u16(self.id);
        ctx.broadcast(pkt, true);
    }
}
