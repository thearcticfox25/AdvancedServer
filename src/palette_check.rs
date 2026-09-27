//! The one thing a client still chooses for itself and the server checks: the colours
//! of its skin (CLIENT_PLAYER_PALETTE). Everything about the round is the server's, so
//! there is nothing else to doubt (states::round).

use crate::packet::Packet;
use crate::core::palettes::PALETTE_TABLES;

const PALETTE_EPSILON: u8 = 10;

fn palette_within_epsilon(reported: u8, expected: u8) -> bool {
    reported >= expected.saturating_sub(PALETTE_EPSILON)
        && reported <= expected.saturating_add(PALETTE_EPSILON)
}

fn palette_matches(colors: &[[u8; 3]], palette: &[u32]) -> bool {
    let count = colors.len().min(palette.len());
    if count == 0 {
        return false;
    }
    let mut match_count = 0usize;
    for slot in 0..count {
        let rgb = palette[slot];
        let red   = ((rgb >> 16) & 0xFF) as u8;
        let green = ((rgb >> 8)  & 0xFF) as u8;
        let blue  = (rgb        & 0xFF) as u8;
        if palette_within_epsilon(colors[slot][0], red)
            && palette_within_epsilon(colors[slot][1], green)
            && palette_within_epsilon(colors[slot][2], blue)
        {
            match_count += 1;
        }
    }
    match_count != 0 && match_count == count
}

pub fn palette_player_validate(player_id: u16, packet: &mut Packet) -> bool {
    let (Some(from), Some(id), Some(name), Some(size)) = (packet.read_u8(), packet.read_u16(), packet.read_str(), packet.read_u8()) else {
        return false;
    };

    if size == 0 { return false; }
    if id != player_id { return false; }

    let count = (size / 4) as usize;
    if count > 19 { return false; }

    let is_custom = name.starts_with("custom");

    let mut colors = [[0u8; 3]; 19];
    for slot in 0..count {
        let (Some(red), Some(green), Some(blue)) = (packet.read_u8(), packet.read_u8(), packet.read_u8()) else {
            return false;
        };
        let _ =       packet.read_u8();
        colors[slot] = [red, green, blue];
    }
    let colors = &colors[..count];

    // The skin tables are shared with the game (crate::core::palettes).
    let known: Vec<&[u32]> = match (from != 0, is_custom) {
        (true, true) => PALETTE_TABLES.iter().map(|table| table.custom_colours).collect(),
        (true, false) => PALETTE_TABLES.iter().flat_map(|table| table.skins.iter().map(|skin| skin.from)).collect(),
        (false, true) => return true,
        (false, false) => PALETTE_TABLES.iter().flat_map(|table| table.skins.iter().map(|skin| skin.to)).collect(),
    };
    known.iter().any(|palette| palette_matches(colors, palette))
}
