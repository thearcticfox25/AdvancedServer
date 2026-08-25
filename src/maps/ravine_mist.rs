use crate::config::cfg;
use crate::entities::rmz_shard::RmzShard;
use crate::entities::rmz_slug::SlugSpawner;
use crate::packet::{Packet, PacketType};
use crate::player::flags as plrflags;
use crate::server::{BigRingState, OutboxMsg, Server};
use crate::maps::{map_time_ex, map_ring};
use crate::states::game::{game_spawn, game_despawn, game_bigring};

const ALL_SHARD_POSITIONS: [(f32, f32); 12] = [
    (862.0,  248.0),
    (3078.0, 248.0),
    (292.0,  558.0),
    (2918.0, 558.0),
    (1100.0, 820.0),
    (980.0,  1188.0),
    (1870.0, 1252.0),
    (2180.0, 1508.0),
    (2920.0, 2216.0),
    (282.0,  2228.0),
    (1318.0, 1916.0),
    (3010.0, 1766.0),
];

const SLUG_SPAWNER_POSITIONS: [(f32, f32); 11] = [
    (1901.0, 392.0),
    (2193.0, 392.0),
    (2468.0, 392.0),
    (1188.0, 860.0),
    (2577.0, 1952.0),
    (2564.0, 2264.0),
    (2782.0, 2264.0),
    (1441.0, 2264.0),
    (884.0,  2264.0),
    (988.0,  2004.0),
    (915.0,  2004.0),
];

fn rmz_checkstate(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();
    let shard_cfg = &cfg.states.gameplay.entities_misc.map_specific.ravine_mist.shards;
    let amount = shard_cfg.amount;
    let required = shard_cfg.required_for_exit;

    let remaining = server.game.entities.iter().filter(|e| e.tag() == "shard").count().min(amount as usize) as u8;
    let total = amount.saturating_sub(remaining);

    let mut pkt = Packet::new(PacketType::SERVER_RMZSHARD_STATE);
    let _ = pkt.write_u8(3);
    let _ = pkt.write_u8(total);
    outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));

    if server.game.time_sec <= 50 && !cfg.states.gameplay.banana.disable_timer {
        let remaining2 = server.game.entities.iter().filter(|e| e.tag() == "shard").count().min(amount as usize) as u8;
        let total2 = amount.saturating_sub(remaining2);
        if total2 >= required {
            game_bigring(server, BigRingState::Activated, outbox);
        } else {
            game_bigring(server, BigRingState::Deactivated, outbox);
        }
    }
}

fn rmz_spawnshards(peer_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let shard_count = server.find_peer(peer_id).map(|p| p.plr.data[0]).unwrap_or(0);
    let pos = server.find_peer(peer_id).map(|p| p.plr.pos).unwrap_or((0.0, 0.0));
    for _ in 0..shard_count {
        let ox: f32 = (rand::random::<u8>() % 17) as f32 - 8.0;
        log::debug!("shard spawned at {} {}", pos.0 + ox, pos.1);
        game_spawn(server, outbox, RmzShard::new(pos.0 + ox, pos.1, 1));
    }
    if let Some(pd) = server.find_peer_mut(peer_id) {
        pd.plr.data[0] = 0;
    }
}

pub fn rmz_init(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();
    map_time_ex(server, 180, 20);
    map_ring(server, 5);
    server.game.bring_state = BigRingState::None;

    if cfg.states.gameplay.entities_misc.map_specific.ravine_mist.slugs.enabled {
        for &(x, y) in &SLUG_SPAWNER_POSITIONS {
            game_spawn(server, outbox, SlugSpawner::new(x, y));
        }
    }

    let amount = cfg.states.gameplay.entities_misc.map_specific.ravine_mist.shards.amount as usize;
    let mut shards = ALL_SHARD_POSITIONS;
    let n = shards.len();
    for i in 0..n - 1 {
        let j = i + (rand::random::<usize>() % (n - i));
        shards.swap(i, j);
    }
    for i in 0..amount.min(n) {
        game_spawn(server, outbox, RmzShard::new(shards[i].0, shards[i].1, 0));
    }
}

pub fn rmz_tick(server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let cfg = cfg();
    if cfg.states.gameplay.banana.disable_timer { return; }
    if server.game.time == 0.0 { return; }

    let ring_time = cfg.states.gameplay.ring_appearance_timer as u16;
    let escape_time = cfg.states.gameplay.escape_time as u16;

    if server.game.time_sec <= ring_time && server.game.bring_state < BigRingState::Deactivated {
        game_bigring(server, BigRingState::Deactivated, outbox);
    }

    if server.game.time_sec <= escape_time && server.game.bring_state < BigRingState::Activated {
        let shard_cfg = &cfg.states.gameplay.entities_misc.map_specific.ravine_mist.shards;
        let remaining = server.game.entities.iter().filter(|e| e.tag() == "shard").count() as u8;
        let collected = shard_cfg.amount.saturating_sub(remaining);
        if collected >= shard_cfg.required_for_exit {
            game_bigring(server, BigRingState::Activated, outbox);
        }
    }
}

pub fn rmz_tcpmsg(peer_id: u16, packet: &mut Packet, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    let ptype = match packet.packet_type() { Some(t) => t, None => return };

    match ptype {
        PacketType::CLIENT_RMZSLIME_HIT => {
            let peer = match server.find_peer(peer_id) { Some(p) => p, None => return };
            if !peer.in_game { return; }
            if server.game.end > 0.0 { return; }

            packet.pos = 2;
            let eid  = match packet.read_u16() { Some(v) => v, None => return };
            let proj = match packet.read_u8()  { Some(v) => v, None => return };

            let ring_type = server.game.entities.iter()
                .find(|e| e.id() == eid && e.tag() == "slug")
                .and_then(|e| e.rmz_slug_ring());

            let slug_exists = server.game.entities.iter().any(|e| e.id() == eid && e.tag() == "slug");
            if !slug_exists { return; }

            game_despawn(server, outbox, eid);

            if proj != 0 { return; }

            if let Some(ring_bonus) = ring_type {
                if ring_bonus == 0 {
                    if let Some(pd) = server.find_peer_mut(peer_id) {
                        pd.plr.rings += 1;
                        pd.plr.stats.rings += 1;
                        pd.plr.last_rings = Some(std::time::Instant::now());
                    }
                }
                let has_rings = server.find_peer(peer_id).map(|p| p.plr.rings > 0).unwrap_or(false);
                let mut pkt = Packet::new(PacketType::SERVER_RMZSLIME_RINGBONUS);
                let _ = pkt.write_u8(ring_bonus);
                let _ = pkt.write_u8(has_rings as u8);
                outbox.push(OutboxMsg::SendTo(peer_id, pkt.data().to_vec(), true));
            }
        }

        PacketType::CLIENT_RMZSHARD_COLLECT => {
            let peer = match server.find_peer(peer_id) { Some(p) => p, None => return };
            if !peer.in_game { return; }
            if server.game.end > 0.0 { return; }

            packet.pos = 2;
            let eid = match packet.read_u16() { Some(v) => v, None => return };

            let shard_exists = server.game.entities.iter().any(|e| e.id() == eid && e.tag() == "shard");
            if !shard_exists { return; }

            if let Some(pd) = server.find_peer_mut(peer_id) {
                pd.plr.data[0] = pd.plr.data[0].saturating_add(1);
            }

            let exe_id = server.game.exe;
            let ingame_ids: Vec<u16> = server.peers.iter()
                .filter(|p| p.in_game)
                .map(|p| p.id)
                .collect();
            for pid in ingame_ids {
                let is_exe = pid as i32 == exe_id;
                let flags = server.find_peer(pid).map(|p| p.plr.flags).unwrap_or(0);
                let is_demonized = flags & plrflags::DEMONIZED != 0;
                let display_pid = if is_exe || is_demonized { 0u16 } else { peer_id };
                let mut pkt = Packet::new(PacketType::SERVER_RMZSHARD_STATE);
                let _ = pkt.write_u8(2);
                let _ = pkt.write_u16(eid);
                let _ = pkt.write_u16(display_pid);
                outbox.push(OutboxMsg::SendTo(pid, pkt.data().to_vec(), true));
            }

            game_despawn(server, outbox, eid);
            rmz_checkstate(server, outbox);
        }

        PacketType::CLIENT_PLAYER_DEATH_STATE => {
            if server.game.end > 0.0 { return; }

            let peer = match server.find_peer(peer_id) { Some(p) => p, None => return };
            if peer_id as i32 == server.game.exe { return; }
            if !peer.in_game { return; }

            packet.pos = 2;
            let dead = match packet.read_u8() { Some(v) => v, None => return };
            let _rtimes = packet.read_u8();

            if dead != 0 {
                rmz_spawnshards(peer_id, server, outbox);
            }
            rmz_checkstate(server, outbox);
        }

        _ => {}
    }
}

pub fn rmz_left(peer_id: u16, server: &mut Server, outbox: &mut Vec<OutboxMsg>) {
    if !server.game.started { return; }
    rmz_spawnshards(peer_id, server, outbox);
    rmz_checkstate(server, outbox);
}
