use std::collections::{HashMap, HashSet};
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use rusty_enet as enet;

use crate::connection::{handle_identity, send_preidentity};
use crate::terminal::ConsoleCmd;
use crate::packet::{Packet as GamePacket, PacketType};
use crate::server::{
    apply_outbox, broadcast_chat, DisconnectReason, GameState, OutboxMsg, PeerSummary,
    PendingPeer, Server, ServerShared,
};
use crate::states::{
    char_select::{charselect_state_handle, charselect_state_left, charselect_state_tick},
    game::{game_state_handle, game_state_left, game_state_tick},
    lobby::{
        handle_console_cmd, lobby_broadcast_init, lobby_init, lobby_state_handle,
        lobby_state_left, lobby_state_tick,
    },
    map_vote::{mapvote_state_handle, mapvote_state_left, mapvote_state_tick},
    results::{results_state_handle, results_state_left, results_state_tick},
};

fn print_network_setup_tutorial(base_port: u16, server_count: u16) {
    log::info!("==========================================================");
    log::info!(" HINT: No one connected in the last 3 minutes.");
    log::info!(" Your ports may not be open. See the setup guide below.");
    log::info!("==========================================================");
    log::info!("");
    log::info!("--- NFTABLES (Linux firewall) ---");
    log::info!("Edit /etc/nftables.conf and add to your inet filter table:");
    log::info!("");
    log::info!("  table inet filter {{");
    log::info!("    chain input {{");
    log::info!("      type filter hook input priority 0; policy drop;");
    log::info!("      ct state established,related accept");
    log::info!("      iif lo accept");
    if server_count > 1 {
        log::info!("      udp dport {}-{} accept   # game ports", base_port, base_port + server_count - 1);
    } else {
        log::info!("      udp dport {} accept   # game port", base_port);
    }
    log::info!("    }}");
    log::info!("  }}");
    log::info!("");
    log::info!("After editing the file, apply the changes (run as root):");
    log::info!("  systemctl restart nftables");
    log::info!("");
    log::info!("--- IF YOUR COMPUTER IS BEHIND A ROUTER ---");
    log::info!("1. Find your local IP address:");
    log::info!("     ifconfig");
    log::info!("   Look for your network interface (e.g. eth0 or enp3s0) and note");
    log::info!("   the inet address, e.g.: 192.168.0.17");
    log::info!("");
    log::info!("2. Open the MikroTik management panel:");
    log::info!("   - Launch WinBox or open a browser -> http://192.168.0.1 (or your gateway)");
    log::info!("   - Assign a static IP to this computer by MAC address:");
    log::info!("       IP -> DHCP Server -> Leases -> find the entry -> make it Static");
    log::info!("");
    log::info!("3. Set up port forwarding via NAT:");
    log::info!("       IP -> Firewall -> NAT -> Add (+)");
    log::info!("       Chain:      dstnat");
    log::info!("       Protocol:   UDP");
    if server_count > 1 {
        log::info!("       Dst. Port:  {}-{}", base_port, base_port + server_count - 1);
    } else {
        log::info!("       Dst. Port:  {}", base_port);
    }
    log::info!("       Action:     dst-nat");
    log::info!("       To Address: <your local IP, e.g. 192.168.0.17>");
    log::info!("       OK -> Apply");
    log::info!("");
    log::info!("4. Share your connection address with players:");
    log::info!("   - Find your PUBLIC IP at https://2ip.ru");
    log::info!("     You can share this IP directly, but if you own a domain name,");
    log::info!("     it is better to use it: a domain is easier to remember, and");
    log::info!("     if your server's IP ever changes you only need to update the DNS");
    log::info!("     record -- players keep connecting to the same address.");
    log::info!("   - To use a domain: add an A record pointing to your public IP");
    log::info!("     at your domain registrar (e.g. reg.ru: Personal account ->");
    log::info!("     Domains -> DNS -> Add record -> Type A).");
    log::info!("     NOTE: the game does NOT support IPv6 -- do NOT add an AAAA record");
    log::info!("     unless you have a specific reason to do so.");
    log::info!("");
    log::info!("--- PLAYING OVER A VPN? ---");
    log::info!("  ZeroTier-One / RadminVPN / Hamachi / Tailscale:");
    log::info!("    Make sure ALL players are connected to the same VPN network,");
    log::info!("    and that they join using your IP address INSIDE that VPN network");
    log::info!("    (not your LAN or public IP).");
    log::info!("");
    log::info!("  Self-hosted OpenVPN / WireGuard:");
    log::info!("    Make sure peer-to-peer traffic is allowed (clients can reach each other).");
    log::info!("    OpenVPN: add 'client-to-client' to your server config.");
    log::info!("    WireGuard: enable IP forwarding and set AllowedIPs to the full VPN subnet.");
    log::info!("    Players must join using the VPN IP, not the LAN or public IP.");
    log::info!("==========================================================");
}

pub fn server_worker(
    server_id: u16,
    port: u16,
    running: Arc<AtomicBool>,
    cmd_rx: &mpsc::Receiver<ConsoleCmd>,
    shared: Arc<ServerShared>,
) {
    if server_id > 0 {
        std::thread::sleep(std::time::Duration::from_millis(server_id as u64 * 50));
    }

    let mut server = Server::new(server_id);

    let socket = match UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], port))) {
        Ok(s) => s,
        Err(e) => {
            log::error!(
                "[Server {}] Failed to bind UDP socket on port {}: {}",
                server_id, port, e
            );
            return;
        }
    };

    let mut host = match enet::Host::new(
        socket,
        enet::HostSettings {
            peer_limit: 50,
            channel_limit: 2,
            ..Default::default()
        },
    ) {
        Ok(h) => h,
        Err(e) => {
            log::error!("[Server {}] Failed to create ENet host: {:?}", server_id, e);
            return;
        }
    };

    log::info!("[Server {}] Listening on port {}", server_id, port);

    {
        let mut outbox: Vec<OutboxMsg> = Vec::new();
        lobby_init(&mut server);
        lobby_broadcast_init(&server, &mut outbox);
        apply_outbox(&mut host, &mut server, &mut outbox);
    }

    let target_tick = Duration::from_nanos((1_000_000_000.0 / 60.0) as u64);
    let mut next_tick = Instant::now() + target_tick;

    let mut ip_set: HashSet<String> = HashSet::new();
    let mut pending: HashMap<usize, PendingPeer> = HashMap::new();
    let mut peer_index_map: HashMap<u16, usize> = HashMap::new();
    let mut index_to_id: HashMap<usize, u16> = HashMap::new();

    let cfg = crate::config::cfg();
    let base_port = cfg.server_config.networking.port;
    let server_count = cfg.server_config.networking.server_count;


    let mut empty_ticks: u64 = 0;
    let mut tutorial_shown = false;
    const TUTORIAL_TICKS: u64 = 3 * 60 * 60;
    let mut ever_had_player = false;

    const SUMMARY_REFRESH_TICKS: u64 = 10;
    let mut summary_tick: u64 = 0;
    let mut last_peer_count: usize = usize::MAX;

    while running.load(Ordering::Relaxed) && server.running {
        loop {
            match host.service() {
                Ok(Some(event)) => {
                    let ev = event.no_ref();
                    match ev {
                        enet::EventNoRef::Connect { peer: peer_id, .. } => {
                            let idx = peer_id.0;
                            let ip = host
                                .get_peer(peer_id)
                                .and_then(|p| p.address())
                                .map(|a| a.ip().to_string())
                                .unwrap_or_default();

                            log::debug!(
                                "[Server {}] New connection from {} (idx={})",
                                server_id, ip, idx
                            );

                            let mut pp = PendingPeer {
                                ip,
                                timeout: 5.0 * 60.0,
                                auth: crate::anticheat::auth::AuthData::default(),
                            };
                            send_preidentity(idx, &mut host, &mut pp);
                            pending.insert(idx, pp);
                        }

                        enet::EventNoRef::Disconnect { peer: peer_id, .. } => {
                            let idx = peer_id.0;
                            pending.remove(&idx);

                            if let Some(game_id) = index_to_id.remove(&idx) {
                                peer_index_map.remove(&game_id);

                                if let Some(pd) = server.find_peer(game_id) {
                                    let nick       = pd.nickname.clone();
                                    let udid       = pd.udid.clone();
                                    let ip         = pd.ip.clone();
                                    let op         = pd.op;
                                    let should_tmo = pd.should_timeout;

                                    ip_set.remove(&ip);
                                    ip_set.remove(&udid);

                                    log::info!(
                                        "[Server {}] Player '{}' (id={}) disconnected",
                                        server_id, crate::colors::colorize(&nick), game_id
                                    );

                                    let mut outbox: Vec<OutboxMsg> = Vec::new();
                                    match server.state {
                                        GameState::Lobby => {
                                            lobby_state_left(game_id, &mut server, &mut outbox)
                                        }
                                        GameState::MapVote => {
                                            mapvote_state_left(game_id, &mut server, &mut outbox)
                                        }
                                        GameState::CharSelect => {
                                            charselect_state_left(game_id, &mut server, &mut outbox)
                                        }
                                        GameState::Game => {
                                            game_state_left(game_id, &mut server, &mut outbox)
                                        }
                                        GameState::Results => {
                                            results_state_left(game_id, &mut server, &mut outbox)
                                        }
                                    }


                                    if op < 2 && should_tmo {
                                        if crate::moderation::timeout_check(&udid, &ip).is_none() {
                                            let secs = cfg.server_config.pairing.rate_limit_window as u64;
                                            let expires = crate::moderation::now_unix() + secs;
                                            crate::moderation::timeout_set(&nick, &udid, &ip, expires, &nick);
                                        }
                                    }

                                    server.peers.retain(|p| p.id != game_id);
                                    apply_outbox(&mut host, &mut server, &mut outbox);
                                }
                            }
                        }

                        enet::EventNoRef::Receive { peer: peer_id, packet, .. } => {
                            let idx = peer_id.0;
                            let data = packet.data().to_vec();

                            if let Some(game_id) = index_to_id.get(&idx).copied() {
                                let rtt_ms = host
                                    .get_peer(enet::PeerID(idx))
                                    .map(|p| p.round_trip_time().as_millis() as u16)
                                    .unwrap_or(0);
                                if let Some(pd) = server.find_peer_mut(game_id) {
                                    pd.rtt = rtt_ms;
                                }

                                let mut pkt = GamePacket::from_data(&data);
                                pkt.pos = 2;

                                let mut outbox: Vec<OutboxMsg> = Vec::new();
                                match server.state {
                                    GameState::Lobby => {
                                        lobby_state_handle(game_id, &mut pkt, &mut server, &mut outbox)
                                    }
                                    GameState::MapVote => {
                                        mapvote_state_handle(game_id, &mut pkt, &mut server, &mut outbox)
                                    }
                                    GameState::CharSelect => {
                                        charselect_state_handle(game_id, &mut pkt, &mut server, &mut outbox)
                                    }
                                    GameState::Game => {
                                        game_state_handle(game_id, &mut pkt, &mut server, &mut outbox)
                                    }
                                    GameState::Results => {
                                        results_state_handle(game_id, &mut pkt, &mut server, &mut outbox)
                                    }
                                }
                                apply_outbox(&mut host, &mut server, &mut outbox);
                            } else if pending.contains_key(&idx) {
                                let mut pkt = GamePacket::from_data(&data);
                                if pkt.len < 2 {
                                    continue;
                                }
                                if pkt.packet_type() == Some(PacketType::IDENTITY) {
                                    handle_identity(
                                        idx,
                                        &data,
                                        &mut host,
                                        &mut server,
                                        &mut pending,
                                        &mut ip_set,
                                        &mut peer_index_map,
                                        &mut index_to_id,
                                    );
                                }
                            }
                        }
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    log::warn!("[Server {}] ENet service error: {:?}", server_id, e);
                    break;
                }
            }
        }

        let delta = server.delta;
        pending.retain(|idx, pp| {
            pp.timeout -= delta * 60.0;
            if pp.timeout <= 0.0 {
                if let Some(peer) = host.get_peer_mut(enet::PeerID(*idx)) {
                    peer.disconnect_now(DisconnectReason::ServerTimeout as u32);
                }
                false
            } else {
                true
            }
        });

        while let Ok(cmd) = cmd_rx.try_recv() {
            let mut outbox: Vec<OutboxMsg> = Vec::new();
            match cmd {
                ConsoleCmd::Chat(msg) => {
                    broadcast_chat(&mut outbox, &msg);
                }
                ConsoleCmd::ExecAsServer(line) => {
                    handle_console_cmd(&line, &mut server, &mut outbox);
                }
            }
            apply_outbox(&mut host, &mut server, &mut outbox);
        }

        let now = Instant::now();
        if now >= next_tick {
            next_tick += target_tick;


            if server_id == 0 && !tutorial_shown && cfg.miscellaneous.other.instructor_enabled {
                if !server.peers.is_empty() {
                    ever_had_player = true;
                }
                if ever_had_player {
                    empty_ticks = 0;
                } else {
                    empty_ticks += 1;
                    if empty_ticks >= TUTORIAL_TICKS {
                        print_network_setup_tutorial(base_port, server_count);
                        tutorial_shown = true;
                    }
                }
            }


            if empty_ticks % (60 * 60) == 0 || empty_ticks == 1 {
                crate::moderation::cleanup_expired_timeouts();
            }

            let mut outbox: Vec<OutboxMsg> = Vec::new();
            match server.state {
                GameState::Lobby => lobby_state_tick(&mut server, &mut outbox),
                GameState::MapVote => mapvote_state_tick(&mut server, &mut outbox),
                GameState::CharSelect => charselect_state_tick(&mut server, &mut outbox),
                GameState::Game => game_state_tick(&mut server, &mut outbox),
                GameState::Results => results_state_tick(&mut server, &mut outbox),
            }
            apply_outbox(&mut host, &mut server, &mut outbox);

            let to_disconnect: Vec<u16> = server
                .peers
                .iter()
                .filter(|p| p.disconnecting)
                .map(|p| p.id)
                .collect();
            for id in to_disconnect {
                if let Some(idx) = peer_index_map.get(&id).copied() {
                    if let Some(peer) = host.get_peer_mut(enet::PeerID(idx)) {
                        peer.disconnect(DisconnectReason::KickedByHost as u32);
                    }
                }
            }

            summary_tick += 1;
            let peer_count = server.peers.len();
            if peer_count != last_peer_count || summary_tick % SUMMARY_REFRESH_TICKS == 0 {
                last_peer_count = peer_count;
                let summary: Vec<PeerSummary> = server
                    .peers
                    .iter()
                    .map(|p| PeerSummary {
                        id: p.id,
                        ip: p.ip.clone(),
                        udid: p.udid.clone(),
                        nickname: p.nickname.clone(),
                        op: p.op,
                        mod_tool: p.mod_tool,
                        is_mobile: p.is_mobile,
                        in_game: p.in_game,
                    })
                    .collect();
                if let Ok(mut guard) = shared.peers.write() {
                    *guard = summary;
                }
            }
        }

        std::thread::sleep(Duration::from_millis(1));
    }


    let remaining_ids: Vec<(u16, usize)> = server.peers.iter()
        .filter_map(|p| peer_index_map.get(&p.id).map(|&idx| (p.id, idx)))
        .collect();
    for (_id, idx) in remaining_ids {
        if let Some(peer) = host.get_peer_mut(enet::PeerID(idx)) {
            peer.disconnect_now(DisconnectReason::Shutdown as u32);
        }
    }

    log::info!("[Server {}] Shutting down.", server_id);
}
