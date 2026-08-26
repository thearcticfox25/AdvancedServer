use std::net::UdpSocket;
use std::sync::RwLock;
use std::time::Instant;

use rusty_enet as enet;

pub const SERVER_VERSION: &str  = "1.1.0.1.psi-testing-3";

use crate::anticheat::auth::AuthData;
use crate::entities::Entity;
use crate::packet::{Packet as GamePacket, PacketType};
use crate::player::Player;
use crate::vote::Vote;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectReason {
    Other           = 255,
    KickedByHost    = 1,
    BannedByHost    = 2,
    VersionMismatch = 3,
    ServerTimeout   = 4,
    PacketsNotRecv  = 5,
    AfkTimeout      = 7,
    LobbyFull       = 8,
    RateLimited     = 9,
    Shutdown        = 10,
    IpInUse         = 11,
}

pub enum OutboxMsg {
    Broadcast(Vec<u8>, bool),
    BroadcastEx(Vec<u8>, bool, u16),
    SendTo(u16, Vec<u8>, bool),
    Disconnect(u16, u32),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(i8)]
pub enum SurvChar {
    None   = -1,
    Tails  =  0,
    Knux   =  1,
    Eggman =  2,
    Amy    =  3,
    Cream  =  4,
    Sally  =  5,
}

impl SurvChar {
    pub fn from_i8(v: i8) -> Self {
        match v {
            0 => Self::Tails,
            1 => Self::Knux,
            2 => Self::Eggman,
            3 => Self::Amy,
            4 => Self::Cream,
            5 => Self::Sally,
            _ => Self::None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(i8)]
pub enum ExeChar {
    None     = -1,
    Original =  0,
    Chaos    =  1,
    Exetior  =  2,
    Exeller  =  3,
}

impl ExeChar {
    pub fn from_i8(v: i8) -> Self {
        match v {
            0 => Self::Original,
            1 => Self::Chaos,
            2 => Self::Exetior,
            3 => Self::Exeller,
            _ => Self::None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum BigRingState {
    None        = 0,
    Deactivated = 1,
    Activated   = 2,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ending {
    ExeWin  = 0,
    SurvWin = 1,
    TimeOver = 2,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameState {
    Lobby,
    MapVote,
    CharSelect,
    Game,
    Results,
}

pub struct PeerData {
    pub id: u16,
    pub ip: String,
    pub plr: Player,
    pub nickname: String,
    pub udid: String,
    pub lobby_icon: u8,
    pub pet: i8,
    pub verified: bool,
    pub in_game: bool,
    pub op: u8,
    pub ready: bool,
    pub mod_tool: bool,
    pub is_mobile: bool,
    pub auth: AuthData,
    pub can_vote: bool,
    pub voted: bool,
    pub disconnecting: bool,
    pub surv_char: SurvChar,
    pub exe_char: ExeChar,
    pub should_timeout: bool,
    pub exe_chance: u8,
    pub timeout: f64,
    pub vote_cooldown: f64,
    pub rtt: u16,
    // Per-peer chat token bucket (anti-flood). Refilled on demand from
    // wall-clock time, so no per-tick bookkeeping is needed.
    pub chat_tokens: f64,
    pub chat_last: Instant,
}

impl PeerData {
    /// Consume one chat token. `burst`/`refill_per_sec` come from config
    /// (`states.lobby_misc.chat_rate_limit`). Returns true if the message is allowed,
    /// false if the peer is currently rate-limited and the message should be dropped.
    pub fn chat_token_take(&mut self, burst: f64, refill_per_sec: f64) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.chat_last).as_secs_f64();
        self.chat_last = now;
        self.chat_tokens = (self.chat_tokens + elapsed * refill_per_sec).min(burst);
        if self.chat_tokens >= 1.0 {
            self.chat_tokens -= 1.0;
            true
        } else {
            false
        }
    }

    pub fn new(id: u16, ip: String) -> Self {
        Self {
            id,
            ip,
            plr: Player::default(),
            nickname: String::new(),
            udid: String::new(),
            lobby_icon: 0,
            pet: -1,
            verified: false,
            in_game: false,
            op: 0,
            ready: false,
            mod_tool: false,
            is_mobile: false,
            auth: AuthData::default(),
            can_vote: true,
            voted: false,
            disconnecting: false,
            surv_char: SurvChar::None,
            exe_char: ExeChar::None,
            should_timeout: false,
            exe_chance: 1,
            timeout: 0.0,
            vote_cooldown: 0.0,
            rtt: 0,
            // Start with a full bucket so a fresh peer can chat immediately; the cap is
            // re-applied from config on the first take.
            chat_tokens: crate::config::cfg().states.lobby_misc.chat_rate_limit.burst,
            chat_last: Instant::now(),
        }
    }
}

pub struct LobbyData {
    pub countdown: f64,
    pub prac_countdown: f64,
    pub countdown_sec: u8,
    pub vote: Vote,
    pub kick_target: Option<PeerData>,
    pub voting_map: i8,
    pub maps: [u8; 3],
    pub votes: [u8; 3],
    pub map: i8,
    pub exe: u16,
    pub avail: [bool; 6],
    // legacy (v1.0.0 C#) votekick state, used only when
    // states.lobby_misc.votekick.use_legacy_voting is enabled
    pub legacy_votekick_ongoing: bool,
    pub legacy_votekick_target: Option<u16>,
    pub legacy_votekick_timer: f64,
    pub legacy_votekick_votes: Vec<u16>,
    // legacy (v1.0.0 C#) votepractice state
    pub legacy_practice_ongoing: bool,
    pub legacy_practice_votes: Vec<u16>,
    /// Set by tournament_advance instead of calling mapvote_init directly, so
    /// the countdown this tick started actually elapses in real time -- giving
    /// every client's freshly-entered Lobby room time to finish loading -- before
    /// lobby_state_tick's normal countdown-expiry branch moves on to MapVote.
    pub tournament_pending: bool,
}

impl Default for LobbyData {
    fn default() -> Self {
        Self {
            countdown: 0.0,
            prac_countdown: 0.0,
            countdown_sec: 0,
            vote: Vote::default(),
            kick_target: None,
            voting_map: -1,
            maps: [0; 3],
            votes: [0; 3],
            map: -1,
            exe: 0,
            avail: [true; 6],
            legacy_votekick_ongoing: false,
            legacy_votekick_target: None,
            legacy_votekick_timer: 0.0,
            legacy_votekick_votes: Vec::new(),
            legacy_practice_ongoing: false,
            legacy_practice_votes: Vec::new(),
            tournament_pending: false,
        }
    }
}

pub struct GameData {
    pub exe: i32,
    pub map: i8,
    pub started: bool,
    pub sudden_death: bool,
    pub start_timeout: f64,
    pub time: f64,
    pub elapsed: f64,
    pub time_sec: u16,
    pub end: f64,
    pub ending: Ending,
    pub bring_state: BigRingState,
    pub bring_loc: u8,
    pub entid: u16,
    pub entities: Vec<Box<dyn Entity>>,
    pub rings: [bool; 256],
    pub ring_coff: u8,
    pub left: Vec<PeerData>,
}

impl Default for GameData {
    fn default() -> Self {
        Self {
            exe: -1,
            map: 0,
            started: false,
            sudden_death: false,
            start_timeout: 0.0,
            time: 0.0,
            elapsed: 0.0,
            time_sec: 0,
            end: 0.0,
            ending: Ending::TimeOver,
            bring_state: BigRingState::None,
            bring_loc: 0,
            entid: 0,
            entities: Vec::new(),
            rings: [false; 256],
            ring_coff: 5,
            left: Vec::new(),
        }
    }
}

pub struct ResultsData {
    pub countdown: f64,
    /// Decided once in results_init and read again in results_uninit's
    /// tournament_advance, so the two stay in sync: whether spectators will be
    /// let into Lobby once this Results screen ends. Always true outside
    /// tournament_mode.
    pub open_lobby_after: bool,
}

impl Default for ResultsData {
    fn default() -> Self {
        Self { countdown: 0.0, open_lobby_after: true }
    }
}

pub struct Server {
    pub id: u16,
    pub running: bool,
    pub state: GameState,
    pub lobby: LobbyData,
    pub game: GameData,
    pub results: ResultsData,
    pub last_map: i8,
    pub map_pickrates: [i16; 30],
    pub delta: f64,
    pub peers: Vec<PeerData>,
}

impl Server {
    pub fn new(id: u16) -> Self {
        Self {
            id,
            running: true,
            state: GameState::Lobby,
            lobby: LobbyData::default(),
            game: GameData::default(),
            results: ResultsData::default(),
            last_map: -1,
            map_pickrates: [255i16; 30],
            delta: 1.0 / 60.0,
            peers: Vec::new(),
        }
    }

    pub fn ingame_count(&self) -> usize {
        self.peers.iter().filter(|p| p.in_game).count()
    }

    pub fn total_count(&self) -> usize {
        self.peers.len()
    }

    pub fn find_peer(&self, id: u16) -> Option<&PeerData> {
        self.peers.iter().find(|p| p.id == id)
    }

    pub fn find_peer_mut(&mut self, id: u16) -> Option<&mut PeerData> {
        self.peers.iter_mut().find(|p| p.id == id)
    }

    /// Whether `id` is allowed to send a chat message right now. Operators at or
    /// above the configured `exempt_op_level` bypass the limit so moderation messaging is
    /// never throttled. When the limit is disabled this is a no-op (always allows).
    pub fn chat_rate_allow(&mut self, id: u16) -> bool {
        let rl = &crate::config::cfg().states.lobby_misc.chat_rate_limit;
        if !rl.enable {
            return true;
        }
        let (burst, refill, exempt) = (rl.burst, rl.messages_per_second, rl.exempt_op_level);
        match self.find_peer_mut(id) {
            Some(p) => p.op >= exempt || p.chat_token_take(burst, refill),
            None => false,
        }
    }

    pub fn find_peer_idx(&self, id: u16) -> Option<usize> {
        self.peers.iter().position(|p| p.id == id)
    }
}

pub struct PendingPeer {
    pub ip: String,
    pub timeout: f64,
    pub auth: AuthData,
}

pub struct PeerSummary {
    pub id: u16,
    pub ip: String,
    pub udid: String,
    pub nickname: String,
    pub op: u8,
    pub mod_tool: bool,
    pub is_mobile: bool,
    pub in_game: bool,
}

pub struct ServerShared {
    pub peers: RwLock<Vec<PeerSummary>>,
}

impl ServerShared {
    pub fn new() -> Self {
        Self {
            peers: RwLock::new(Vec::new()),
        }
    }
}

pub fn send_chat(outbox: &mut Vec<OutboxMsg>, target: u16, msg: &str) {
    let mut pkt = GamePacket::new(PacketType::CLIENT_CHAT_MESSAGE);
    let _ = pkt.write_u16(0);
    let _ = pkt.write_str(&msg.to_lowercase());
    outbox.push(OutboxMsg::SendTo(target, pkt.data().to_vec(), true));
}

pub fn broadcast_chat(outbox: &mut Vec<OutboxMsg>, msg: &str) {
    let mut pkt = GamePacket::new(PacketType::CLIENT_CHAT_MESSAGE);
    let _ = pkt.write_u16(0);
    let _ = pkt.write_str(&msg.to_lowercase());
    outbox.push(OutboxMsg::Broadcast(pkt.data().to_vec(), true));
}

pub fn apply_outbox(
    host: &mut enet::Host<UdpSocket>,
    server: &mut Server,
    outbox: &mut Vec<OutboxMsg>,
) {
    for msg in outbox.drain(..) {
        match msg {
            OutboxMsg::Broadcast(data, reliable) => {
                let pkt = make_enet_packet(&data, reliable);
                host.broadcast(0, &pkt);
            }
            OutboxMsg::BroadcastEx(data, reliable, exclude_id) => {
                let pkt = make_enet_packet(&data, reliable);
                let exclude_idx = (exclude_id as usize).wrapping_sub(1);
                for peer in host.connected_peers_mut() {
                    if peer.id().0 != exclude_idx {
                        let _ = peer.send(0, &pkt);
                    }
                }
            }
            OutboxMsg::SendTo(id, data, reliable) => {
                let idx = (id as usize).wrapping_sub(1);
                let pkt = make_enet_packet(&data, reliable);
                if let Some(peer) = host.get_peer_mut(enet::PeerID(idx)) {
                    let _ = peer.send(0, &pkt);
                }
            }
            OutboxMsg::Disconnect(id, reason) => {
                let idx = (id as usize).wrapping_sub(1);
                if let Some(peer) = host.get_peer_mut(enet::PeerID(idx)) {
                    peer.disconnect(reason);
                }
            }
        }
    }
}

fn make_enet_packet(data: &[u8], reliable: bool) -> enet::Packet {
    if reliable {
        enet::Packet::reliable(data)
    } else {
        enet::Packet::unreliable(data)
    }
}
