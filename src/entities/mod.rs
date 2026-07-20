pub mod ring;
pub mod cream_ring;
pub mod black_ring;
pub mod tails_projectile;
pub mod eggman_tracker;
pub mod exeller_clone;
pub mod dt_ball;
pub mod dt_stalactits;
pub mod dt_tails_doll;
pub mod lc_chain;
pub mod lc_eye;
pub mod nap_ice;
pub mod nap_snowball;
pub mod not_perfect;
pub mod pf_lift;
pub mod rmz_shard;
pub mod rmz_slug;
pub mod spike_controller;
pub mod tc_acid;
pub mod vv_lava;
pub mod vv_vase;
pub mod wd_latern;
pub mod you_cant_run;
pub mod kaf_speedbox;
pub mod act9_wall;
pub mod hd_door;
pub mod hill_thunder;
pub mod mj_lava;
pub mod mj_judger;
pub mod mj_ass;
pub mod dummy;

use crate::packet::Packet;
use crate::server::OutboxMsg;

pub trait Entity: Send {
    fn tag(&self) -> &'static str;
    fn id(&self) -> u16;
    fn set_id(&mut self, id: u16);
    fn pos(&self) -> (f32, f32);


    fn on_init(&mut self, ctx: &mut EntityCtx) -> bool { true }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool { true }

    fn on_uninit(&mut self, ctx: &mut EntityCtx) {}

    fn set_activ_id(&mut self, _id: u16) {}

    fn is_red(&self) -> bool { false }

    /// For erector-spawned black rings only: the world position at which a survivor
    /// standing inside the ring should have taken its damage. `None` for every other
    /// entity, for map-placed rings (unknown position), and during the fade-in.
    fn bring_hit_pos(&self) -> Option<(f32, f32)> { None }

    fn owner_id(&self) -> u16 { 0 }
    fn dir_i8(&self) -> i8 { 0 }

    fn dt_sid(&self) -> i16 { -1 }
    fn dt_activate(&mut self, _ctx: &mut EntityCtx) {}

    fn kaf_nid(&self) -> i16 { -1 }
    fn kaf_activate(&mut self, _ctx: &mut EntityCtx, _pid: u16, _is_proj: bool) {}

    fn pf_lid(&self) -> i16 { -1 }
    fn pf_activate(&mut self, _ctx: &mut EntityCtx, _pid: u16) {}

    fn hd_toggle(&mut self, _ctx: &mut EntityCtx) {}

    fn nap_iid(&self) -> i16 { -1 }
    fn nap_activate(&mut self, _ctx: &mut EntityCtx) {}

    fn vv_vid(&self) -> i16 { -1 }
    fn vv_vtype(&self) -> u8 { 0 }

    fn lceye_nid(&self) -> i16 { -1 }
    fn lceye_get(&self) -> (bool, u8) { (false, 0) }
    fn lceye_set_used(&mut self, _used: bool, _use_id: u16, _target: u8, _ctx: &mut EntityCtx) {}

    fn dummy_push(&mut self, _spd: i8) {}

    fn rmz_slug_ring(&self) -> Option<u8> { None }

    fn spawner_slug_id(&self) -> u16 { 0 }
    fn spawner_clear_slug(&mut self, _slug_id: u16) {}
    fn spawner_pos(&self) -> Option<(f32, f32)> { None }
    fn spawner_take_spawn(&mut self) -> bool { false }
    fn spawner_set_slug(&mut self, _id: u16) {}
}

pub struct EntityCtx<'a> {
    pub outbox: &'a mut Vec<OutboxMsg>,
    pub server_id: u16,
    pub map_id: i8,
    pub map_ring_count: u8,
    pub rings: &'a mut [bool; 256],
    pub ring_coff: u8,

    pub ingame_peers: Vec<(u16, (f32, f32), u8, i8, i8, u8)>,
    pub entity_ids: Vec<u16>,

    pub exe_id: i32,
    pub game_time_sec: u16,
    pub game_time: f64,
    pub rand: &'a mut rand::rngs::SmallRng,

    pub spawn_queue: Vec<Box<dyn Entity>>,
}

impl<'a> EntityCtx<'a> {
    pub fn broadcast(&mut self, pkt: Packet, reliable: bool) {
        self.outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), reliable));
    }
}
