use rand::Rng;

use crate::packet::Packet;

const AUTH_BIT_A: u32     = 0x0000_0200;
const AUTH_BIT_B: u32     = 0x8000_0000;
const AUTH_BIT_C: u32     = 0x0400_0000;
const AUTH_FLAG_MASK: u32 = AUTH_BIT_A | AUTH_BIT_B | AUTH_BIT_C;

/// The challenge this server sent one connecting client. The client must hash
/// it back with constants only the real game binary knows (see verify_ticket).
#[derive(Debug, Clone, Default)]
pub struct AuthData {
    pub challenge_one: u8,
    pub challenge_two: u8,
    pub auth_type:     u32,
}

pub fn create_ticket(auth: &mut AuthData, packet: &mut Packet) {
    let mut rng = rand::thread_rng();

    auth.challenge_one = rng.gen::<u8>() & 0x7F;
    auth.challenge_two = rng.gen_range(0u8..255);

    let flag = match rng.gen_range(0..3u32) {
        0 => AUTH_BIT_A,
        1 => AUTH_BIT_B,
        _ => AUTH_BIT_C,
    };
    let noise = rng.gen::<u32>() & !AUTH_FLAG_MASK;
    auth.auth_type = flag | noise;

    const BALLS: [u8; 6] = [0xff, 0x1c, 0x22, 0x00, 0x14, 0x80];
    let _ = packet.write_u16(0);
    let _ = packet.write_u16(1);
    let _ = packet.write_u8(auth.challenge_one);
    let _ = packet.write_u8(rng.gen_range(0u8..2));
    let _ = packet.write_u8(auth.challenge_two);
    for _ in 0..3 {
        let _ = packet.write_u8(BALLS[rng.gen_range(0..BALLS.len())]);
    }
    let _ = packet.write_u32(auth.auth_type);
}

pub fn verify_ticket(
    auth:      &AuthData,
    enable:    bool,
    strict_c1: Option<&[u64; 3]>,
    strict_c2: Option<&[u64; 3]>,
    mod_tool:  &mut bool,
    is_mobile: &mut bool,
    packet:    &mut Packet,
) -> bool {
    let checksum1 = packet.read_u64().unwrap_or(0);
    let checksum2 = packet.read_u64().unwrap_or(0);

    let ticket = auth.auth_type;
    let base   = ticket as u64;

    if enable {
        match (strict_c1, strict_c2) {
            (Some(c1), Some(c2)) => {
                // Strict mode: verify against PC-client constants.
                let slot: Option<usize> =
                    if      ticket & AUTH_BIT_A != 0 { Some(0) }
                    else if ticket & AUTH_BIT_B != 0 { Some(1) }
                    else if ticket & AUTH_BIT_C != 0 { Some(2) }
                    else { None };

                match slot {
                    Some(slot) => {
                        if checksum1 != base + c1[slot] { *mod_tool = true; }
                        if checksum2 != base + c2[slot] { *mod_tool = true; }
                    }
                    None => *mod_tool = true,
                }
            }
            _ => {
                // FOSS mode: betterserver-oss-main behaviour.
                *mod_tool = checksum1 == 0 || checksum2 == 0;
            }
        }
    }

    // Mobile fallback: overrides mod_tool regardless of strict/FOSS result.
    if checksum1 == base + auth.challenge_one as u64 && checksum2 == base + auth.challenge_two as u64 {
        *mod_tool  = false;
        *is_mobile = true;
    }

    true
}
