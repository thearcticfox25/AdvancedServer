use std::collections::{HashMap, HashSet};
use std::net::UdpSocket;

use rand::Rng;
use rusty_enet as enet;

use crate::anticheat::auth;
use crate::config::cfg;
use crate::packet::{Packet as GamePacket, PacketType};
use crate::server::{
    apply_outbox, DisconnectReason, GameState, OutboxMsg, PeerData, PendingPeer, Server,
};
use crate::states::{
    char_select::charselect_state_join,
    game::game_state_join,
    lobby::lobby_state_join,
    map_vote::mapvote_state_join,
};

pub fn send_preidentity(idx: usize, host: &mut enet::Host<UdpSocket>, pp: &mut PendingPeer) {
    let mut pkt = GamePacket::new(PacketType::SERVER_PREIDENTITY);
    auth::create_ticket(&mut pp.auth, &mut pkt);
    let ep = enet::Packet::reliable(pkt.data());
    if let Some(peer) = host.get_peer_mut(enet::PeerID(idx)) {
        let _ = peer.send(0, &ep);
    }
}

pub fn disconnect_idx(idx: usize, host: &mut enet::Host<UdpSocket>, reason: u32) {
    log::info!("Disconnected idx {} (reason {})", idx, reason);
    if let Some(peer) = host.get_peer_mut(enet::PeerID(idx)) {
        peer.disconnect(reason);
    }
}

pub fn handle_identity(
    idx: usize,
    data: &[u8],
    host: &mut enet::Host<UdpSocket>,
    server: &mut Server,
    pending: &mut HashMap<usize, PendingPeer>,
    ip_set: &mut HashSet<String>,
    peer_index_map: &mut HashMap<u16, usize>,
    index_to_id: &mut HashMap<usize, u16>,
    all_shared: &[std::sync::Arc<crate::server::ServerShared>],
    base_port: u16,
) {
    let cfg = cfg();
    let mut pkt = GamePacket::from_data(data);
    pkt.pos = 2;


    let version = match pkt.read_u16() {
        Some(v) => v,
        None => { log::debug!("Identity failed for idx {}: truncated version", idx); return; }
    };
    let _server_index = pkt.read_i32();
    let nickname = match pkt.read_str() {
        Some(s) => s,
        None => { log::debug!("Identity failed for idx {}: truncated nickname", idx); return; }
    };
    let udid = match pkt.read_str() {
        Some(s) => s,
        None => { log::debug!("Identity failed for idx {}: truncated udid", idx); return; }
    };
    let lobby_icon = pkt.read_u8().unwrap_or(0);
    let pet = pkt.read_i8().unwrap_or(-1);

    // Limit by Unicode characters, not bytes — matches the upstream "30 characters
    // max" rule (C's string_length counts codepoints) so multi-byte nicks aren't
    // rejected early. read_str already caps the raw field at 128 bytes.
    if crate::packet::str_unicode_len(&nickname) >= 30 {
        disconnect_idx(idx, host, DisconnectReason::Other as u32);
        pending.remove(&idx);
        return;
    }

    let (ip, mod_tool, is_mobile) = match pending.get(&idx) {
        Some(pp) => {
            let mut mt = false;
            let mut mobile = false;
            let mt_cfg = &cfg.states.gameplay.anticheat.useless_anticheat;
            let (sc1, sc2) = if mt_cfg.strict_mode.is_active() {
                (Some(&mt_cfg.strict_mode.c1), Some(&mt_cfg.strict_mode.c2))
            } else {
                (None, None)
            };
            auth::verify_ticket(
                &pp.auth,
                mt_cfg.enable,
                sc1,
                sc2,
                &mut mt,
                &mut mobile,
                &mut pkt,
            );
            (pp.ip.clone(), mt, mobile)
        }
        None => return,
    };

    if !cfg.server_config.pairing.versioning.disable_version_validating
        && version != cfg.server_config.pairing.versioning.target_version
    {
        disconnect_idx(idx, host, DisconnectReason::VersionMismatch as u32);
        pending.remove(&idx);
        return;
    }


    if let Some(ts) = crate::moderation::timeout_check(&udid, &ip) {
        if ts > crate::moderation::now_unix() {
            log::info!("{} is rate-limited (id {}, ip {})", crate::colors::colorize(&nickname), idx, ip);
            disconnect_idx(idx, host, DisconnectReason::RateLimited as u32);
            pending.remove(&idx);
            return;
        }
    }

    let max = cfg.server_config.pairing.maximum_players_per_lobby as usize;
    if server.total_count() >= max {
        // Before rejecting, check sibling lobbies in this same process for
        // room and redirect there instead. No disconnect on this path --
        // just tell the client where to go and leave the pending connection
        // be; it's cleaned up by the ordinary pending-timeout if the client
        // doesn't follow.
        for (j, shared_j) in all_shared.iter().enumerate() {
            if j as u16 == server.id { continue; }
            let count = shared_j.peers.read().map(|g| g.len()).unwrap_or(usize::MAX);
            if count < max {
                let target_port = base_port + j as u16;
                let mut redirect = GamePacket::new(PacketType::SERVER_LOBBY_CHANGELOBBY);
                let _ = redirect.write_u32(target_port as u32);
                let ep = enet::Packet::reliable(redirect.data());
                if let Some(peer) = host.get_peer_mut(enet::PeerID(idx)) {
                    let _ = peer.send(0, &ep);
                }
                log::debug!("Redirecting idx {} to another free server: {}", idx, j);
                return;
            }
        }
        disconnect_idx(idx, host, DisconnectReason::LobbyFull as u32);
        pending.remove(&idx);
        return;
    }

    let op_level = crate::moderation::op_check(&udid, &ip);

    if op_level < 2
        && cfg.server_config.pairing.ip_validation
        && (ip_set.contains(&ip) || ip_set.contains(&udid))
    {
        disconnect_idx(idx, host, DisconnectReason::IpInUse as u32);
        pending.remove(&idx);
        return;
    }

    if crate::moderation::ban_check(&nickname, &udid, &ip) {
        log::info!("{} banned by host (id {}, ip {})", crate::colors::colorize(&nickname), idx, ip);
        disconnect_idx(idx, host, DisconnectReason::BannedByHost as u32);
        pending.remove(&idx);
        return;
    }

    if cfg.states.lobby_misc.moderation.enforce_whitelist
        && !crate::moderation::whitelist_check(&udid, &ip)
    {
        disconnect_idx(idx, host, DisconnectReason::BannedByHost as u32);
        pending.remove(&idx);
        return;
    }

    let game_id = (idx as u16).wrapping_add(1);

    ip_set.insert(ip.clone());
    ip_set.insert(udid.clone());
    pending.remove(&idx);
    peer_index_map.insert(game_id, idx);
    index_to_id.insert(idx, game_id);

    let mut pd = PeerData::new(game_id, ip);
    pd.nickname = nickname;
    pd.udid = udid;
    pd.lobby_icon = lobby_icon;
    pd.pet = pet;
    pd.verified = true;
    pd.op = op_level;
    pd.should_timeout = true;
    pd.mod_tool  = mod_tool;
    pd.is_mobile = is_mobile;

    pd.in_game = server.state == GameState::Lobby;

    pd.exe_chance = 1 + rand::thread_rng().gen_range(0u8..4u8);

    server.peers.push(pd);


    let in_lobby: u8 = if server.state == GameState::Lobby { 1 } else { 0 };
    let mut resp = GamePacket::new(PacketType::SERVER_IDENTITY_RESPONSE);
    let _ = resp.write_u8(in_lobby);
    let _ = resp.write_u16(game_id);
    let ep = enet::Packet::reliable(resp.data());
    if let Some(peer) = host.get_peer_mut(enet::PeerID(idx)) {
        let _ = peer.send(0, &ep);
    }

    let last_peer = server.peers.last();
    log::info!(
        "[Server {}] Player '{}' joined (id={}, mod_tool={}, mobile={})",
        server.id,
        crate::colors::colorize(last_peer.map(|p| p.nickname.as_str()).unwrap_or("?")),
        game_id,
        mod_tool,
        is_mobile,
    );
    log::info!("  IP: {}", last_peer.map(|p| p.ip.as_str()).unwrap_or("?"));
    log::info!("  UID: {}", last_peer.map(|p| p.udid.as_str()).unwrap_or("?"));

    let mut outbox: Vec<OutboxMsg> = Vec::new();
    match server.state {
        GameState::Lobby => lobby_state_join(game_id, server, &mut outbox),
        GameState::MapVote => mapvote_state_join(game_id, server, &mut outbox),
        GameState::CharSelect => charselect_state_join(game_id, server, &mut outbox),
        GameState::Game => game_state_join(game_id, server, &mut outbox),
        GameState::Results => {}
    }

    apply_outbox(host, server, &mut outbox);
}
