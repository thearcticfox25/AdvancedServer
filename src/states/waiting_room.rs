use crate::colors::*;
use crate::config::cfg;
use crate::maps::MAP_LIST;
use crate::packet::{Packet, PacketType};
use crate::server::{broadcast_chat, send_chat, OutboxMsg, Server};
use crate::terminal::cmd::parse_cmd;

pub fn send_waiting_player_list(
    target_id: u16,
    exe_id: i32,
    server: &Server,
    outbox: &mut Vec<OutboxMsg>,
) {
    let anon_mode = cfg().states.lobby_misc.anonymous_mode;
    let target_op = server.find_peer(target_id).map(|p| p.op).unwrap_or(0);
    let anonymous = anon_mode && target_op < 1;

    for p in server.peers.iter().filter(|p| p.id != target_id) {
        let mut pkt = Packet::new(PacketType::SERVER_WAITING_PLAYER_INFO);
        let _ = pkt.write_u8(if p.in_game { 1 } else { 0 });
        let _ = pkt.write_u16(p.id);
        if anonymous {
            let _ = pkt.write_str("anonymous");
            let _ = pkt.write_u8(0);
        } else {
            let _ = pkt.write_str(&p.nickname);
            if p.in_game {
                let is_exe = p.id as i32 == exe_id;
                let _ = pkt.write_u8(if is_exe { 1 } else { 0 });
                let _ = pkt.write_i8(if is_exe { p.exe_char as i8 } else { p.surv_char as i8 });
            } else {
                let _ = pkt.write_u8(p.lobby_icon);
            }
        }
        outbox.push(OutboxMsg::SendTo(target_id, pkt.data().to_vec(), true));
    }
}

pub fn announce_waiter_joined(v_id: u16, server: &Server, outbox: &mut Vec<OutboxMsg>) {
    let anon_mode = cfg().states.lobby_misc.anonymous_mode;
    let joiner_icon = server.find_peer(v_id).map(|p| p.lobby_icon).unwrap_or(0);
    let joiner_nick = server.find_peer(v_id).map(|p| p.nickname.clone()).unwrap_or_default();

    let targets: Vec<(u16, u8)> = server.peers.iter()
        .filter(|p| p.id != v_id && !p.in_game)
        .map(|p| (p.id, p.op))
        .collect();

    for (oid, oop) in targets {
        let anon_for_other = anon_mode && oop < 1;
        let mut pkt = Packet::new(PacketType::SERVER_WAITING_PLAYER_INFO);
        let _ = pkt.write_u8(0u8);
        let _ = pkt.write_u16(v_id);
        if anon_for_other {
            let _ = pkt.write_str("anonymous");
            let _ = pkt.write_u8(0);
        } else {
            let _ = pkt.write_str(&joiner_nick);
            let _ = pkt.write_u8(joiner_icon);
        }
        outbox.push(OutboxMsg::SendTo(oid, pkt.data().to_vec(), true));
    }
}

pub fn send_waiting_room_greeting(
    v_id: u16,
    map: Option<i8>,
    server: &Server,
    outbox: &mut Vec<OutboxMsg>,
) {
    crate::states::lobby::send_greeting(v_id, server, outbox);
    if let Some(map_i8) = map {
        let idx = map_i8 as usize;
        if idx < MAP_LIST.len() {
            send_chat(outbox, v_id, &format!("{}map:~ {}", COLOR_CYAN, MAP_LIST[idx].name));
        }
    }
}

pub fn broadcast_map_announcement(map: i8, outbox: &mut Vec<OutboxMsg>) {
    let idx = map as usize;
    if idx < MAP_LIST.len() {
        broadcast_chat(outbox, &format!("{}map:~ {}", COLOR_CYAN, MAP_LIST[idx].name));
    }
}

pub fn handle_waiter_chat(
    v_id: u16,
    msg: &str,
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
) {
    let trimmed = msg.trim();
    if trimmed.starts_with(':') || trimmed.starts_with('.') {
        if let Some(cmd) = parse_cmd(trimmed) {
            let op = server.find_peer(v_id).map(|p| p.op).unwrap_or(0);
            if crate::states::lobby::exec_cmd_pub(&cmd, v_id, op, server, outbox) {
                return;
            }
        }
    }
    let mut pkt = Packet::new(PacketType::CLIENT_CHAT_MESSAGE);
    let _ = pkt.write_u16(v_id);
    let _ = pkt.write_str(msg);
    outbox.push(OutboxMsg::BroadcastEx(pkt.data().to_vec(), true, v_id));
}
