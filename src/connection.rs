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

/// Everything the worker knows about connections, as opposed to about the round.
///
/// ENet addresses a peer by its slot index, the game protocol addresses it by a
/// 1-based id; these maps translate between the two. They always travel together,
/// so they are passed around as one value instead of four.
pub struct PeerRegistry {
    /// Connected, but not identified yet: no nickname or UDID seen so far.
    pub pending: HashMap<usize, PendingPeer>,
    /// IPs and UDIDs currently in use, for the one-connection-per-address rule.
    pub taken_addresses: HashSet<String>,
    pub game_id_of_slot: HashMap<usize, u16>,
    pub slot_of_game_id: HashMap<u16, usize>,
}

impl PeerRegistry {
    pub fn new() -> Self {
        Self {
            pending:         HashMap::new(),
            taken_addresses: HashSet::new(),
            game_id_of_slot: HashMap::new(),
            slot_of_game_id: HashMap::new(),
        }
    }

    /// Registers an identified player, so packets from ENet slot `idx` can be
    /// routed to game id `game_id` and back.
    pub fn accept(&mut self, idx: usize, game_id: u16, ip: &str, udid: &str) {
        self.pending.remove(&idx);
        self.taken_addresses.insert(ip.to_string());
        self.taken_addresses.insert(udid.to_string());
        self.game_id_of_slot.insert(idx, game_id);
        self.slot_of_game_id.insert(game_id, idx);
    }
}

/// Drops a connection that was never accepted into the lobby.
fn reject(
    idx: usize,
    host: &mut enet::Host<UdpSocket>,
    peers: &mut PeerRegistry,
    reason: DisconnectReason,
) {
    disconnect_idx(idx, host, reason as u32);
    peers.pending.remove(&idx);
}

pub fn send_preidentity(idx: usize, host: &mut enet::Host<UdpSocket>, pending: &mut PendingPeer) {
    let mut pkt = GamePacket::new(PacketType::SERVER_PREIDENTITY);
    auth::create_ticket(&mut pending.auth, &mut pkt);
    let enet_packet = enet::Packet::reliable(pkt.data());
    if let Some(peer) = host.get_peer_mut(enet::PeerID(idx)) {
        let _ = peer.send(0, &enet_packet);
    }
}

pub fn disconnect_idx(idx: usize, host: &mut enet::Host<UdpSocket>, reason: u32) {
    log::info!("Disconnected idx {} (reason {})", idx, reason);
    if let Some(peer) = host.get_peer_mut(enet::PeerID(idx)) {
        peer.disconnect(reason);
    }
}

/// Handles the IDENTITY packet: the client's one chance to say who it is.
/// Everything that can turn a connection away (wrong version, ban, timeout, full
/// lobby, duplicate address) is decided here, and only then does the connection
/// become a `PeerData` in `server.peers`.
pub fn handle_identity(
    idx: usize,
    data: &[u8],
    host: &mut enet::Host<UdpSocket>,
    server: &mut Server,
    peers: &mut PeerRegistry,
    all_shared: &[std::sync::Arc<crate::server::ServerShared>],
    base_port: u16,
) {
    let cfg = cfg();
    let mut pkt = GamePacket::from_data(data);
    pkt.pos = 2;

    let version = match pkt.read_u16() {
        Some(found) => found,
        None => { log::debug!("Identity failed for idx {}: truncated version", idx); return; }
    };
    let _server_index = pkt.read_i32();
    let nickname = match pkt.read_str() {
        Some(text) => text,
        None => { log::debug!("Identity failed for idx {}: truncated nickname", idx); return; }
    };
    let udid = match pkt.read_str() {
        Some(text) => text,
        None => { log::debug!("Identity failed for idx {}: truncated udid", idx); return; }
    };
    let lobby_icon = pkt.read_u8().unwrap_or(0);
    let pet = pkt.read_i8().unwrap_or(-1);

    // Limit by Unicode characters, not bytes - matches the upstream "30 characters
    // max" rule (C's string_length counts codepoints) so multi-byte nicks aren't
    // rejected early. read_str already caps the raw field at 128 bytes.
    if crate::packet::str_unicode_len(&nickname) >= 30 {
        reject(idx, host, peers, DisconnectReason::Other);
        return;
    }

    let (ip, mod_tool, is_mobile) = match peers.pending.get(&idx) {
        Some(pending) => {
            let mut mod_tool_cfg = false;
            let mut mobile = false;
            let mt_cfg = &cfg.states.gameplay.anticheat.useless_anticheat;
            let (sc1, sc2) = if mt_cfg.strict_mode.is_active() {
                (Some(&mt_cfg.strict_mode.c1), Some(&mt_cfg.strict_mode.c2))
            } else {
                (None, None)
            };
            auth::verify_ticket(
                &pending.auth,
                mt_cfg.enable,
                sc1,
                sc2,
                &mut mod_tool_cfg,
                &mut mobile,
                &mut pkt,
            );
            (pending.ip.clone(), mod_tool_cfg, mobile)
        }
        None => return,
    };

    if !cfg.server_config.pairing.versioning.disable_version_validating
        && version != cfg.server_config.pairing.versioning.target_version
    {
        reject(idx, host, peers, DisconnectReason::VersionMismatch);
        return;
    }

    if let Some(ts) = crate::moderation::timeout_check(&udid, &ip) {
        if ts > crate::moderation::now_unix() {
            log::info!("{} is rate-limited (id {}, ip {})", crate::colors::colorize(&nickname), idx, ip);
            reject(idx, host, peers, DisconnectReason::RateLimited);
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
        for (sibling_id, sibling) in all_shared.iter().enumerate() {
            if sibling_id as u16 == server.id { continue; }
            let count = sibling.peers.read().map(|guard| guard.len()).unwrap_or(usize::MAX);
            if count < max {
                let target_port = base_port + sibling_id as u16;
                let mut redirect = GamePacket::new(PacketType::SERVER_LOBBY_CHANGELOBBY);
                let _ = redirect.write_u32(target_port as u32);
                let enet_packet = enet::Packet::reliable(redirect.data());
                if let Some(peer) = host.get_peer_mut(enet::PeerID(idx)) {
                    let _ = peer.send(0, &enet_packet);
                }
                log::debug!("Redirecting idx {} to another free server: {}", idx, sibling_id);
                return;
            }
        }
        reject(idx, host, peers, DisconnectReason::LobbyFull);
        return;
    }

    let op_level = crate::moderation::op_check(&udid, &ip);

    if op_level < 2
        && cfg.server_config.pairing.ip_validation
        && (peers.taken_addresses.contains(&ip) || peers.taken_addresses.contains(&udid))
    {
        reject(idx, host, peers, DisconnectReason::IpInUse);
        return;
    }

    if crate::moderation::ban_check(&nickname, &udid, &ip) {
        log::info!("{} banned by host (id {}, ip {})", crate::colors::colorize(&nickname), idx, ip);
        reject(idx, host, peers, DisconnectReason::BannedByHost);
        return;
    }

    if cfg.states.lobby_misc.moderation.enforce_whitelist
        && !crate::moderation::whitelist_check(&udid, &ip)
    {
        reject(idx, host, peers, DisconnectReason::BannedByHost);
        return;
    }

    let game_id = (idx as u16).wrapping_add(1);
    peers.accept(idx, game_id, &ip, &udid);

    let mut peer = PeerData::new(game_id, ip);
    peer.nickname = nickname;
    peer.udid = udid;
    peer.lobby_icon = lobby_icon;
    peer.pet = pet;
    peer.verified = true;
    peer.op = op_level;
    peer.should_timeout = true;
    peer.mod_tool  = mod_tool;
    peer.is_mobile = is_mobile;

    peer.in_game = server.state == GameState::Lobby;

    peer.exe_chance = 1 + rand::thread_rng().gen_range(0u8..4u8);

    server.peers.push(peer);

    let in_lobby: u8 = if server.state == GameState::Lobby { 1 } else { 0 };
    let mut resp = GamePacket::new(PacketType::SERVER_IDENTITY_RESPONSE);
    let _ = resp.write_u8(in_lobby);
    let _ = resp.write_u16(game_id);
    let enet_packet = enet::Packet::reliable(resp.data());
    if let Some(peer) = host.get_peer_mut(enet::PeerID(idx)) {
        let _ = peer.send(0, &enet_packet);
    }

    let last_peer = server.peers.last();
    log::info!(
        "[Server {}] Player '{}' joined (id={}, mod_tool={}, mobile={})",
        server.id,
        crate::colors::colorize(last_peer.map(|peer| peer.nickname.as_str()).unwrap_or("?")),
        game_id,
        mod_tool,
        is_mobile,
    );
    log::info!("  IP: {}", last_peer.map(|peer| peer.ip.as_str()).unwrap_or("?"));
    log::info!("  UID: {}", last_peer.map(|peer| peer.udid.as_str()).unwrap_or("?"));

    let mut outbox: Vec<OutboxMsg> = Vec::new();
    crate::states::state_join(game_id, server, &mut outbox);
    apply_outbox(host, server, &mut outbox);
}
