use super::{Entity, EntityCtx};
use rand::Rng;

pub struct VvVase {
    pub id: u16,
    pub vid: u8,
    pub vtype: u8,
}

impl VvVase {
    pub fn new(vid: u8) -> Self {
        Self { id: 0, vid, vtype: 0 }
    }
}

impl Entity for VvVase {
    fn tag(&self) -> &'static str { "vase" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { (0.0, 0.0) }
    fn vv_vid(&self) -> i16 { self.vid as i16 }
    fn vv_vtype(&self) -> u8 { self.vtype }

    fn on_init(&mut self, ctx: &mut EntityCtx) -> bool {

        self.vtype = ctx.rand.gen_range(0u8..4);
        true
    }
}
