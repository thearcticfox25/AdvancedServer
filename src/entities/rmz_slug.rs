use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};
use rand::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SlugState {
    NoneRight = 0,
    NoneLeft  = 1,
    RingRight = 2,
    RingLeft  = 3,
    RedRingRight = 4,
    RedRingLeft  = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlugRing {
    NoRing,
    Ring,
    RedRing,
}

pub struct RmzSlug {
    pub id: u16,
    pub pos: (f32, f32),
    pub spawn_x: f32,
    pub spawn_y: f32,
    pub state: SlugState,
    pub ring: SlugRing,
}

impl RmzSlug {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            id: 0,
            pos: (x, y),
            spawn_x: x,
            spawn_y: y,
            state: SlugState::NoneRight,
            ring: SlugRing::NoRing,
        }
    }

    fn face(&mut self, side: bool) {
        self.state = match (side, self.ring) {
            (true,  SlugRing::NoRing)  => SlugState::NoneRight,
            (true,  SlugRing::Ring)    => SlugState::RingRight,
            (true,  SlugRing::RedRing) => SlugState::RedRingRight,
            (false, SlugRing::NoRing)  => SlugState::NoneLeft,
            (false, SlugRing::Ring)    => SlugState::RingLeft,
            (false, SlugRing::RedRing) => SlugState::RedRingLeft,
        };
    }
}

impl Entity for RmzSlug {
    fn tag(&self) -> &'static str { "slug" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { self.pos }
    fn rmz_slug_ring(&self) -> Option<u8> {
        match self.ring {
            SlugRing::NoRing  => None,
            SlugRing::Ring    => Some(0),
            SlugRing::RedRing => Some(1),
        }
    }
    fn spawner_clear_slug(&mut self, _slug_id: u16) {}

    fn on_init(&mut self, ctx: &mut EntityCtx) -> bool {
        let cfg = crate::config::cfg();
        let slugs = &cfg.states.gameplay.entities_misc.map_specific.ravine_mist.slugs;

        let roll = ctx.rand.gen_range(0u8..100);
        if roll < slugs.red_ring_chance {
            self.ring = SlugRing::RedRing;
        } else if roll < slugs.red_ring_chance + slugs.ring_chance {
            self.ring = SlugRing::Ring;
        } else {
            self.ring = SlugRing::NoRing;
        }

        self.spawn_x = self.pos.0;
        self.spawn_y = self.pos.1;

        let dir = ctx.rand.gen_range(0u8..2) != 0;
        self.face(dir);

        let mut pkt = Packet::new(PacketType::SERVER_RMZSLIME_STATE);
        let _ = pkt.write_u8(0);
        let _ = pkt.write_u16(self.id);
        let _ = pkt.write_u16(self.pos.0 as u16);
        let _ = pkt.write_u16(self.pos.1 as u16);
        let _ = pkt.write_u8(self.state as u8);
        ctx.broadcast(pkt, true);
        true
    }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        match self.state {
            SlugState::NoneLeft | SlugState::RingLeft | SlugState::RedRingLeft => {
                self.pos.0 -= 1.0;
                if self.pos.0 <= self.spawn_x - 100.0 {
                    self.face(true);
                }
            }
            SlugState::NoneRight | SlugState::RingRight | SlugState::RedRingRight => {
                self.pos.0 += 1.0;
                if self.pos.0 >= self.spawn_x + 100.0 {
                    self.face(false);
                }
            }
        }

        let mut pkt = Packet::new(PacketType::SERVER_RMZSLIME_STATE);
        let _ = pkt.write_u8(1);
        let _ = pkt.write_u16(self.id);
        let _ = pkt.write_u16(self.pos.0 as u16);
        let _ = pkt.write_u16(self.pos.1 as u16);
        let _ = pkt.write_u8(self.state as u8);
        ctx.broadcast(pkt, false);
        true
    }

    fn on_uninit(&mut self, ctx: &mut EntityCtx) {
        let mut pkt = Packet::new(PacketType::SERVER_RMZSLIME_STATE);
        let _ = pkt.write_u8(2);
        let _ = pkt.write_u16(self.id);
        ctx.broadcast(pkt, true);
    }
}

pub struct SlugSpawner {
    pub id: u16,
    pub pos: (f32, f32),
    pub slug_id: u16,
    pub timer: f64,
    pub offset: f64,
    pub need_spawn: bool,
}

impl SlugSpawner {
    pub fn new(x: f32, y: f32) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let offset = (rng.gen_range(0u32..2) as f64) * crate::vote::TICKS_PER_SEC;
        Self { id: 0, pos: (x, y), slug_id: 0, timer: 0.0, offset, need_spawn: false }
    }
}

impl Entity for SlugSpawner {
    fn tag(&self) -> &'static str { "slugspawn" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { self.pos }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        if self.slug_id != 0 && !ctx.entity_ids.contains(&self.slug_id) {
            log::debug!("removed slug from {}", self.id);
            self.slug_id = 0;
        }
        if self.slug_id != 0 {
            return true;
        }
        if self.offset > 0.0 {
            self.offset -= 1.0;
            return true;
        }
        self.timer += 1.0;
        if self.timer >= 15.0 * crate::vote::TICKS_PER_SEC {
            self.timer = 0.0;
            self.need_spawn = true;
            ctx.spawn_queue.push(Box::new(RmzSlug::new(self.pos.0, self.pos.1)));
        }
        true
    }

    fn spawner_slug_id(&self) -> u16 { self.slug_id }
    fn spawner_set_slug(&mut self, id: u16) { self.slug_id = id; self.need_spawn = false; }
    fn spawner_take_spawn(&mut self) -> bool {
        let wanted = self.need_spawn;
        self.need_spawn = false;
        wanted
    }
    fn spawner_pos(&self) -> Option<(f32, f32)> { Some(self.pos) }
}
