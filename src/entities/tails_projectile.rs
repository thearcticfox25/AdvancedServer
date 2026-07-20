use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};
use crate::vote::TICKS_PER_SEC;

pub struct TailsProjectile {
    pub id: u16,
    pub pos: (f32, f32),
    pub owner: u16,
    pub dir: i8,
    pub exe: bool,
    pub charge: u8,
    pub damage: u8,
    pub timer: f64,
}

impl TailsProjectile {
    pub fn new(x: f32, y: f32, owner: u16, dir: i8, exe: bool, charge: u8, damage: u8) -> Self {
        let cfg = crate::config::cfg();
        let timer = cfg.states.gameplay.entities_misc.character_specific.tails.projectile_timeout_timer as f64 * TICKS_PER_SEC;
        Self { id: 0, pos: (x, y), owner, dir, exe, charge, damage, timer }
    }
}

impl Entity for TailsProjectile {
    fn tag(&self) -> &'static str { "tproj" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { self.pos }

    fn on_init(&mut self, ctx: &mut EntityCtx) -> bool {


        let mut pkt = Packet::new(PacketType::SERVER_TPROJECTILE_STATE);
        let _ = pkt.write_u8(0);
        let _ = pkt.write_u16(self.pos.0 as u16);
        let _ = pkt.write_u16(self.pos.1 as u16);
        let _ = pkt.write_u16(self.owner);
        let _ = pkt.write_i8(self.dir);
        let _ = pkt.write_u8(self.damage);
        let _ = pkt.write_u8(self.exe as u8);
        let _ = pkt.write_u8(self.charge);
        ctx.broadcast(pkt, true);
        true
    }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        if self.timer <= 0.0 {
            return false;
        }


        if ctx.map_id != 13 {
            if self.pos.0 <= 0.0 {
                return false;
            }
        } else {
            if self.pos.0 < 0.0 {
                self.pos.0 = 3648.0;
            } else if self.pos.0 > 3648.0 {
                self.pos.0 = 0.0;
            }
        }


        let mut pkt = Packet::new(PacketType::SERVER_TPROJECTILE_STATE);
        let _ = pkt.write_u8(1);
        let _ = pkt.write_u16(self.pos.0 as u16);
        let _ = pkt.write_u16(self.pos.1 as u16);
        ctx.broadcast(pkt, false);


        let cfg = crate::config::cfg();
        let speed = cfg.states.gameplay.entities_misc.character_specific.tails.projectile_speed as f32;
        self.pos.0 += self.dir as f32 * speed;
        self.timer -= 1.0;

        true
    }

    fn on_uninit(&mut self, ctx: &mut EntityCtx) {

        let mut pkt = Packet::new(PacketType::SERVER_TPROJECTILE_STATE);
        let _ = pkt.write_u8(2);
        ctx.broadcast(pkt, true);
    }
}
