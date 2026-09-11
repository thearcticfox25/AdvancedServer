use super::{Entity, EntityCtx};
use crate::packet::{Packet, PacketType};
use crate::server::{DisconnectReason, OutboxMsg};

const ACT9_ROOM_WIDTH: f64 = 3072.0;

pub struct Act9Wall {
    pub id: u16,
    pub pos: (f32, f32),
    pub wall_id: u8,
    pub start_time: f64,
}

impl Act9Wall {
    pub fn new(wall_id: u8, x: f32, y: f32) -> Self {
        Self { id: 0, pos: (x, y), wall_id, start_time: 0.0 }
    }
}

impl Entity for Act9Wall {
    fn tag(&self) -> &'static str { "act9wall" }
    fn id(&self) -> u16 { self.id }
    fn set_id(&mut self, id: u16) { self.id = id; }
    fn pos(&self) -> (f32, f32) { self.pos }

    fn on_init(&mut self, ctx: &mut EntityCtx) -> bool {
        self.start_time = ctx.game_time;
        true
    }

    fn on_tick(&mut self, ctx: &mut EntityCtx) -> bool {
        let time = ctx.game_time;
        let scale = if self.start_time > 0.0 {
            (self.start_time - time) / self.start_time
        } else {
            0.0
        };

        let x = self.pos.0 as f64 * scale;
        let y = self.pos.1 as f64 * scale;

        let mut pkt = Packet::new(PacketType::SERVER_ACT9WALL_STATE);
        let _ = pkt.write_u8(self.wall_id);
        let _ = pkt.write_u16(x as u16);
        let _ = pkt.write_u16(y as u16);
        ctx.broadcast(pkt, false);

        let cfg = crate::config::cfg();
        if cfg.states.gameplay.anticheat.zone_anticheat {
            let (wx_min, wy_min, wx_max, wy_max): (f64, f64, f64, f64) = match self.wall_id {
                0 => {
                    let box_x = -2240.0_f64;
                    let box_y = y - 768.0;
                    (box_x, box_y, box_x + 64.0 * 117.0, box_y + 64.0 * 12.0)
                }
                1 => {
                    let box_x = x - 2240.0;
                    (box_x, y, box_x + 64.0 * 34.0, y + 64.0 * 19.5)
                }
                2 => {
                    let box_x = (ACT9_ROOM_WIDTH - x) + 64.0;
                    (box_x, y, box_x + 64.0 * 34.0, y + 64.0 * 19.5)
                }
                _ => { log::error!("Invalid wall id!"); return false; }
            };

            let to_kick: Vec<u16> = ctx.ingame_peers.iter()
                .filter(|(_, pos, flags, _, _, _)| {
                    const PLAYER_DEAD: u8 = 0x1 << 1;
                    if flags & PLAYER_DEAD != 0 { return false; }
                    if pos.0 == 0.0 && pos.1 == 0.0 { return false; }
                    (pos.0 as f64) >= wx_min && (pos.1 as f64) >= wy_min
                        && (pos.0 as f64) <= wx_max && (pos.1 as f64) <= wy_max
                })
                .map(|(id, _, _, _, _, _)| *id)
                .collect();

            for id in to_kick {
                ctx.outbox.push(OutboxMsg::Disconnect(id, DisconnectReason::Other as u32));
            }
        }

        true
    }
}
