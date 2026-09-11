use super::{Entity, EntityCtx};
use rand::Rng;

pub struct VvVase {
    pub id: u16,
    pub vase_id: u8,
    pub vase_type: u8,
}

impl VvVase {
    pub fn new(vase_id: u8) -> Self {
        Self { id: 0, vase_id, vase_type: 0 }
    }
}

impl Entity for VvVase {
    fn tag(&self) -> &'static str { "vase" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { (0.0, 0.0) }
    fn vv_vid(&self) -> i16 { self.vase_id as i16 }
    fn vv_vtype(&self) -> u8 { self.vase_type }

    fn on_init(&mut self, ctx: &mut EntityCtx) -> bool {
        self.vase_type = ctx.rand.gen_range(0u8..4);
        true
    }
}
