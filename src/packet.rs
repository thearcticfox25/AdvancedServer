
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types, dead_code)]
pub enum PacketType {
    IDENTITY = 0,
    SERVER_IDENTITY_RESPONSE,
    SERVER_PLAYER_JOINED,
    SERVER_PLAYER_LEFT,
    SERVER_PLAYER_FORCE_DISCONNECT,
    SERVER_WAITING_PLAYER_INFO,
    SERVER_LOBBY_READY_STATE,
    SERVER_LOBBY_EXE,
    SERVER_LOBBY_COUNTDOWN,
    SERVER_LOBBY_EXE_CHANGE,
    SERVER_LOBBY_CHARACTER_CHANGE,
    SERVER_LOBBY_CHARACTER_RESPONSE,
    SERVER_LOBBY_EXECHARACTER_RESPONSE,
    SERVER_LOBBY_GAME_START,
    SERVER_LOBBY_PLAYER,
    SERVER_LOBBY_EXE_CHANCE,
    SERVER_LOBBY_CORRECT,
    SERVER_LOBBY_CHOOSEVOTEKICK,
    SERVER_LOBBY_CHOOSEBAN,
    SERVER_LOBBY_CHOOSEKICK,
    SERVER_LOBBY_CHOOSEOP,
    SERVER_LOBBY_CHANGELOBBY,
    SERVER_CHAR_TIME_SYNC,
    SERVER_VOTE_MAPS,
    SERVER_VOTE_SET,
    SERVER_VOTE_TIME_SYNC,
    SERVER_GAME_PLAYERS_READY,
    SERVER_GAME_EXE_WINS,
    SERVER_GAME_SURVIVOR_WIN,
    SERVER_GAME_SPAWN_RING,
    SERVER_GAME_PLAYER_ESCAPED,
    SERVER_GAME_BACK_TO_LOBBY,
    SERVER_GAME_TIME_SYNC,
    SERVER_GAME_TIME_OVER,
    SERVER_GAME_PING,
    SERVER_PLAYER_DEATH_STATE,
    SERVER_GAME_DEATHTIMER_TICK,
    SERVER_GAME_DEATHTIMER_END,
    SERVER_REQUEST_INFO,
    SERVER_HEARTBEAT,
    SERVER_PONG,
    SERVER_FORCE_DAMAGE,
    SERVER_GAME_RING_READY,
    SERVER_PLAYER_BACKTRACK,

    SERVER_TPROJECTILE_STATE,
    SERVER_ETRACKER_STATE,
    SERVER_ERECTOR_BRING_SPAWN,
    SERVER_RMZSLIME_STATE,
    SERVER_RMZSLIME_RINGBONUS,
    SERVER_RMZSHARD_STATE,
    SERVER_LCEYE_STATE,
    SERVER_LCCHAIN_STATE,
    SERVER_NPCONTROLLER_STATE,
    SERVER_KAFMONITOR_STATE,
    SERVER_YCRSMOKE_STATE,
    SERVER_YCRSMOKE_READY,
    SERVER_MOVINGSPIKE_STATE,
    SERVER_RING_STATE,
    SERVER_RING_COLLECTED,
    SERVER_ACT9WALL_STATE,
    SERVER_NAPBALL_STATE,
    SERVER_NAPICE_STATE,
    SERVER_PFLIFT_STATE,
    SERVER_BRING_STATE,
    SERVER_BRING_COLLECTED,
    SERVER_VVLCOLUMN_STATE,
    SERVER_VVVASE_STATE,
    SERVER_GHZTHUNDER_STATE,
    SERVER_TCGOM_STATE,
    SERVER_EXELLERCLONE_STATE,
    SERVER_DTTAILSDOLL_STATE,
    SERVER_DTBALL_STATE,
    SERVER_DTASS_STATE,
    SERVER_HDDOOR_STATE,
    SERVER_WDLATERN_ACTIVATE,
    SERVER_FART_STATE,
    SERVER_MJLAVA_STATE,
    SERVER_MJJUDGER_STATE,
    SERVER_MJCRYSTAL_STATE,

    CLIENT_ETRACKER,
    CLIENT_ETRACKER_ACTIVATED,
    CLIENT_TPROJECTILE,
    CLIENT_TPROJECTILE_HIT,
    CLIENT_TPROJECTILE_STARTCHARGE,
    CLIENT_ERECTOR_BALLS,
    CLIENT_ERECTOR_BRING_SPAWN,
    CLIENT_EXELLER_SPAWN_CLONE,
    CLIENT_EXELLER_TELEPORT_CLONE,
    CLIENT_MERCOIN_BONUS,
    CLIENT_RMZSLIME_HIT,
    CLIENT_LCEYE_REQUEST_ACTIVATE,
    CLIENT_KAFMONITOR_ACTIVATE,
    CLIENT_RING_COLLECTED,
    CLIENT_RING_BROKE,
    CLIENT_BRING_COLLECTED,
    CLIENT_NAPICE_ACTIVATE,
    CLIENT_SPRING_USE,
    CLIENT_PFLIT_ACTIVATE,
    CLIENT_VVVASE_BREAK,
    CLIENT_RMZSHARD_COLLECT,
    CLIENT_RMZSHARD_LAND,
    CLIENT_DTASS_ACTIVATE,
    CLIENT_HDDOOR_TOGGLE,
    CLIENT_FART_PUSH,
    CLIENT_LOBBY_READY_STATE,
    CLIENT_REQUESTED_INFO,
    CLIENT_PLAYER_DATA,
    CLIENT_PLAYER_HURT,
    CLIENT_SOUND_EMIT,
    CLIENT_PING,
    CLIENT_REVIVAL_PROGRESS,
    CLIENT_PLAYER_HEAL,
    CLIENT_PLAYER_HEAL_PART,
    SERVER_REVIVAL_PROGRESS,
    SERVER_REVIVAL_STATUS,
    SERVER_REVIVAL_RINGSUB,
    SERVER_REVIVAL_REVIVED,
    CLIENT_REQUEST_CHARACTER,
    CLIENT_REQUEST_EXECHARACTER,
    CLIENT_VOTE_REQUEST,
    CLIENT_PLAYER_DEATH_STATE,
    CLIENT_PLAYER_ESCAPED,
    SERVER_PLAYER_ESCAPED,
    CLIENT_LOBBY_PLAYERS_REQUEST,
    CLIENT_CREAM_SPAWN_RINGS,
    CLIENT_SPAWN_EFFECT,
    CLIENT_CHAT_MESSAGE,
    CLIENT_LOBBY_CHOOSEVOTEKICK,
    CLIENT_LOBBY_CHOOSEBAN,
    CLIENT_LOBBY_CHOOSEKICK,
    CLIENT_LOBBY_CHOOSEOP,
    CLIENT_PLAYER_PALETTE,
    CLIENT_PET_PALETTE,
    SERVER_RESULTS,
    SERVER_RESULTS_DATA,
    CLIENT_RESULTS_REQUEST,
    CLIENT_STATS_REPORT,
    SERVER_PREIDENTITY,
    SERVER_FELLA,
    CLIENT_PLAYER_POTATER,
}

impl PacketType {
    /// Largest valid discriminant. The enum is `#[repr(u8)]` and contiguous from 0,
    /// so any byte in `0..=MAX` maps to a variant.
    const MAX: u8 = PacketType::CLIENT_PLAYER_POTATER as u8;

    /// Safe `u8 -> PacketType`. Replaces a former `mem::transmute`, which would have
    /// become UB the moment anyone introduced a gap in the discriminants. This relies
    /// only on the layout being contiguous for validity (no `unsafe`).
    pub fn from_u8(v: u8) -> Option<PacketType> {
        if v <= Self::MAX {
            Some(PACKET_TYPE_TABLE[v as usize])
        } else {
            None
        }
    }
}

/// Lookup table from raw byte to `PacketType`, materialized at compile time (zero
/// runtime cost). Entry `i` is the variant whose discriminant is `i`. Construction
/// `assert!`s contiguity, so a future gap in the enum is a compile error rather than
/// a silently wrong/invalid variant (the failure mode of the old `mem::transmute`).
static PACKET_TYPE_TABLE: [PacketType; PacketType::MAX as usize + 1] = build_packet_type_table();

const fn build_packet_type_table() -> [PacketType; PacketType::MAX as usize + 1] {
    // All variants in discriminant order. If a variant is added, append it here;
    // the asserts below catch any mismatch between this list and the repr.
    const VARIANTS: &[PacketType] = &[
        PacketType::IDENTITY,
        PacketType::SERVER_IDENTITY_RESPONSE,
        PacketType::SERVER_PLAYER_JOINED,
        PacketType::SERVER_PLAYER_LEFT,
        PacketType::SERVER_PLAYER_FORCE_DISCONNECT,
        PacketType::SERVER_WAITING_PLAYER_INFO,
        PacketType::SERVER_LOBBY_READY_STATE,
        PacketType::SERVER_LOBBY_EXE,
        PacketType::SERVER_LOBBY_COUNTDOWN,
        PacketType::SERVER_LOBBY_EXE_CHANGE,
        PacketType::SERVER_LOBBY_CHARACTER_CHANGE,
        PacketType::SERVER_LOBBY_CHARACTER_RESPONSE,
        PacketType::SERVER_LOBBY_EXECHARACTER_RESPONSE,
        PacketType::SERVER_LOBBY_GAME_START,
        PacketType::SERVER_LOBBY_PLAYER,
        PacketType::SERVER_LOBBY_EXE_CHANCE,
        PacketType::SERVER_LOBBY_CORRECT,
        PacketType::SERVER_LOBBY_CHOOSEVOTEKICK,
        PacketType::SERVER_LOBBY_CHOOSEBAN,
        PacketType::SERVER_LOBBY_CHOOSEKICK,
        PacketType::SERVER_LOBBY_CHOOSEOP,
        PacketType::SERVER_LOBBY_CHANGELOBBY,
        PacketType::SERVER_CHAR_TIME_SYNC,
        PacketType::SERVER_VOTE_MAPS,
        PacketType::SERVER_VOTE_SET,
        PacketType::SERVER_VOTE_TIME_SYNC,
        PacketType::SERVER_GAME_PLAYERS_READY,
        PacketType::SERVER_GAME_EXE_WINS,
        PacketType::SERVER_GAME_SURVIVOR_WIN,
        PacketType::SERVER_GAME_SPAWN_RING,
        PacketType::SERVER_GAME_PLAYER_ESCAPED,
        PacketType::SERVER_GAME_BACK_TO_LOBBY,
        PacketType::SERVER_GAME_TIME_SYNC,
        PacketType::SERVER_GAME_TIME_OVER,
        PacketType::SERVER_GAME_PING,
        PacketType::SERVER_PLAYER_DEATH_STATE,
        PacketType::SERVER_GAME_DEATHTIMER_TICK,
        PacketType::SERVER_GAME_DEATHTIMER_END,
        PacketType::SERVER_REQUEST_INFO,
        PacketType::SERVER_HEARTBEAT,
        PacketType::SERVER_PONG,
        PacketType::SERVER_FORCE_DAMAGE,
        PacketType::SERVER_GAME_RING_READY,
        PacketType::SERVER_PLAYER_BACKTRACK,
        PacketType::SERVER_TPROJECTILE_STATE,
        PacketType::SERVER_ETRACKER_STATE,
        PacketType::SERVER_ERECTOR_BRING_SPAWN,
        PacketType::SERVER_RMZSLIME_STATE,
        PacketType::SERVER_RMZSLIME_RINGBONUS,
        PacketType::SERVER_RMZSHARD_STATE,
        PacketType::SERVER_LCEYE_STATE,
        PacketType::SERVER_LCCHAIN_STATE,
        PacketType::SERVER_NPCONTROLLER_STATE,
        PacketType::SERVER_KAFMONITOR_STATE,
        PacketType::SERVER_YCRSMOKE_STATE,
        PacketType::SERVER_YCRSMOKE_READY,
        PacketType::SERVER_MOVINGSPIKE_STATE,
        PacketType::SERVER_RING_STATE,
        PacketType::SERVER_RING_COLLECTED,
        PacketType::SERVER_ACT9WALL_STATE,
        PacketType::SERVER_NAPBALL_STATE,
        PacketType::SERVER_NAPICE_STATE,
        PacketType::SERVER_PFLIFT_STATE,
        PacketType::SERVER_BRING_STATE,
        PacketType::SERVER_BRING_COLLECTED,
        PacketType::SERVER_VVLCOLUMN_STATE,
        PacketType::SERVER_VVVASE_STATE,
        PacketType::SERVER_GHZTHUNDER_STATE,
        PacketType::SERVER_TCGOM_STATE,
        PacketType::SERVER_EXELLERCLONE_STATE,
        PacketType::SERVER_DTTAILSDOLL_STATE,
        PacketType::SERVER_DTBALL_STATE,
        PacketType::SERVER_DTASS_STATE,
        PacketType::SERVER_HDDOOR_STATE,
        PacketType::SERVER_WDLATERN_ACTIVATE,
        PacketType::SERVER_FART_STATE,
        PacketType::SERVER_MJLAVA_STATE,
        PacketType::SERVER_MJJUDGER_STATE,
        PacketType::SERVER_MJCRYSTAL_STATE,
        PacketType::CLIENT_ETRACKER,
        PacketType::CLIENT_ETRACKER_ACTIVATED,
        PacketType::CLIENT_TPROJECTILE,
        PacketType::CLIENT_TPROJECTILE_HIT,
        PacketType::CLIENT_TPROJECTILE_STARTCHARGE,
        PacketType::CLIENT_ERECTOR_BALLS,
        PacketType::CLIENT_ERECTOR_BRING_SPAWN,
        PacketType::CLIENT_EXELLER_SPAWN_CLONE,
        PacketType::CLIENT_EXELLER_TELEPORT_CLONE,
        PacketType::CLIENT_MERCOIN_BONUS,
        PacketType::CLIENT_RMZSLIME_HIT,
        PacketType::CLIENT_LCEYE_REQUEST_ACTIVATE,
        PacketType::CLIENT_KAFMONITOR_ACTIVATE,
        PacketType::CLIENT_RING_COLLECTED,
        PacketType::CLIENT_RING_BROKE,
        PacketType::CLIENT_BRING_COLLECTED,
        PacketType::CLIENT_NAPICE_ACTIVATE,
        PacketType::CLIENT_SPRING_USE,
        PacketType::CLIENT_PFLIT_ACTIVATE,
        PacketType::CLIENT_VVVASE_BREAK,
        PacketType::CLIENT_RMZSHARD_COLLECT,
        PacketType::CLIENT_RMZSHARD_LAND,
        PacketType::CLIENT_DTASS_ACTIVATE,
        PacketType::CLIENT_HDDOOR_TOGGLE,
        PacketType::CLIENT_FART_PUSH,
        PacketType::CLIENT_LOBBY_READY_STATE,
        PacketType::CLIENT_REQUESTED_INFO,
        PacketType::CLIENT_PLAYER_DATA,
        PacketType::CLIENT_PLAYER_HURT,
        PacketType::CLIENT_SOUND_EMIT,
        PacketType::CLIENT_PING,
        PacketType::CLIENT_REVIVAL_PROGRESS,
        PacketType::CLIENT_PLAYER_HEAL,
        PacketType::CLIENT_PLAYER_HEAL_PART,
        PacketType::SERVER_REVIVAL_PROGRESS,
        PacketType::SERVER_REVIVAL_STATUS,
        PacketType::SERVER_REVIVAL_RINGSUB,
        PacketType::SERVER_REVIVAL_REVIVED,
        PacketType::CLIENT_REQUEST_CHARACTER,
        PacketType::CLIENT_REQUEST_EXECHARACTER,
        PacketType::CLIENT_VOTE_REQUEST,
        PacketType::CLIENT_PLAYER_DEATH_STATE,
        PacketType::CLIENT_PLAYER_ESCAPED,
        PacketType::SERVER_PLAYER_ESCAPED,
        PacketType::CLIENT_LOBBY_PLAYERS_REQUEST,
        PacketType::CLIENT_CREAM_SPAWN_RINGS,
        PacketType::CLIENT_SPAWN_EFFECT,
        PacketType::CLIENT_CHAT_MESSAGE,
        PacketType::CLIENT_LOBBY_CHOOSEVOTEKICK,
        PacketType::CLIENT_LOBBY_CHOOSEBAN,
        PacketType::CLIENT_LOBBY_CHOOSEKICK,
        PacketType::CLIENT_LOBBY_CHOOSEOP,
        PacketType::CLIENT_PLAYER_PALETTE,
        PacketType::CLIENT_PET_PALETTE,
        PacketType::SERVER_RESULTS,
        PacketType::SERVER_RESULTS_DATA,
        PacketType::CLIENT_RESULTS_REQUEST,
        PacketType::CLIENT_STATS_REPORT,
        PacketType::SERVER_PREIDENTITY,
        PacketType::SERVER_FELLA,
        PacketType::CLIENT_PLAYER_POTATER,
    ];

    let n = PacketType::MAX as usize + 1;
    assert!(
        VARIANTS.len() == n,
        "PACKET_TYPE_TABLE variant list out of sync with PacketType repr"
    );
    let mut table = [PacketType::IDENTITY; PacketType::MAX as usize + 1];
    let mut i = 0;
    while i < n {
        let v = VARIANTS[i];
        assert!(
            v as usize == i,
            "PacketType discriminants are not contiguous from 0 (gap detected)"
        );
        table[i] = v;
        i += 1;
    }
    table
}

pub const PACKET_MAXSIZE: usize = 256;

#[derive(Clone)]
pub struct Packet {
    pub buf: [u8; PACKET_MAXSIZE],
    pub pos: usize,
    pub len: usize,
}

impl Packet {

    pub fn new(ptype: PacketType) -> Self {
        let mut p = Self { buf: [0u8; PACKET_MAXSIZE], pos: 0, len: 0 };
        p.write_u8(0).ok();
        p.write_u8(ptype as u8).ok();
        p
    }


    pub fn from_data(data: &[u8]) -> Self {
        let len = data.len().min(PACKET_MAXSIZE);
        let mut p = Self { buf: [0u8; PACKET_MAXSIZE], pos: 0, len };
        p.buf[..len].copy_from_slice(&data[..len]);
        p
    }

    pub fn packet_type(&self) -> Option<PacketType> {
        if self.len >= 2 { PacketType::from_u8(self.buf[1]) } else { None }
    }

    pub fn raw_type_byte(&self) -> u8 {
        if self.len >= 2 { self.buf[1] } else { 0 }
    }

    pub fn seek(&mut self, pos: usize) -> bool {
        if pos < self.len {
            self.pos = pos;
            true
        } else {
            false
        }
    }

    pub fn data(&self) -> &[u8] {
        &self.buf[..self.len]
    }


    pub fn read_u8(&mut self) -> Option<u8> {
        if self.pos < self.len {
            let v = self.buf[self.pos];
            self.pos += 1;
            Some(v)
        } else {
            None
        }
    }

    pub fn read_u16(&mut self) -> Option<u16> {
        if self.pos + 2 > self.len { return None; }
        let v = u16::from_le_bytes([self.buf[self.pos], self.buf[self.pos + 1]]);
        self.pos += 2;
        Some(v)
    }

    pub fn read_i16(&mut self) -> Option<i16> {
        self.read_u16().map(|v| v as i16)
    }

    pub fn read_u32(&mut self) -> Option<u32> {
        if self.pos + 4 > self.len { return None; }
        let v = u32::from_le_bytes(self.buf[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        Some(v)
    }

    pub fn read_i32(&mut self) -> Option<i32> {
        self.read_u32().map(|v| v as i32)
    }

    pub fn read_u64(&mut self) -> Option<u64> {
        if self.pos + 8 > self.len { return None; }
        let v = u64::from_le_bytes(self.buf[self.pos..self.pos + 8].try_into().unwrap());
        self.pos += 8;
        Some(v)
    }

    pub fn read_f32(&mut self) -> Option<f32> {
        if self.pos + 4 > self.len { return None; }
        let v = f32::from_le_bytes(self.buf[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        Some(v)
    }


    pub fn read_double_compat(&mut self) -> Option<f64> {
        if self.pos + 8 > self.len { return None; }
        let v = f32::from_le_bytes(self.buf[self.pos..self.pos + 4].try_into().unwrap()) as f64;
        self.pos += 8;
        Some(v)
    }


    pub fn read_f64(&mut self) -> Option<f64> {
        if self.pos + 8 > self.len { return None; }
        let v = f64::from_le_bytes(self.buf[self.pos..self.pos + 8].try_into().unwrap());
        self.pos += 8;
        Some(v)
    }

    pub fn read_i8(&mut self) -> Option<i8> {
        self.read_u8().map(|v| v as i8)
    }


    pub fn read_str(&mut self) -> Option<String> {
        let mut bytes = Vec::new();
        loop {
            if self.pos >= self.len { return None; }
            if bytes.len() >= 128 { return None; }
            let ch = self.buf[self.pos];
            self.pos += 1;
            if ch == 0 { break; }
            bytes.push(ch);
        }
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }


    pub fn write_u8(&mut self, v: u8) -> Result<(), ()> {
        if self.pos + 1 >= PACKET_MAXSIZE { return Err(()); }
        if self.pos + 1 >= self.len { self.len += 1; }
        self.buf[self.pos] = v;
        self.pos += 1;
        Ok(())
    }

    pub fn write_i8(&mut self, v: i8) -> Result<(), ()> {
        self.write_u8(v as u8)
    }

    pub fn write_u16(&mut self, v: u16) -> Result<(), ()> {
        if self.pos + 2 >= PACKET_MAXSIZE { return Err(()); }
        if self.pos + 2 >= self.len { self.len += 2; }
        let bytes = v.to_le_bytes();
        self.buf[self.pos] = bytes[0];
        self.buf[self.pos + 1] = bytes[1];
        self.pos += 2;
        Ok(())
    }

    pub fn write_i16(&mut self, v: i16) -> Result<(), ()> {
        self.write_u16(v as u16)
    }

    pub fn write_u32(&mut self, v: u32) -> Result<(), ()> {
        if self.pos + 4 >= PACKET_MAXSIZE { return Err(()); }
        if self.pos + 4 >= self.len { self.len += 4; }
        let bytes = v.to_le_bytes();
        self.buf[self.pos..self.pos + 4].copy_from_slice(&bytes);
        self.pos += 4;
        Ok(())
    }

    pub fn write_u64(&mut self, v: u64) -> Result<(), ()> {
        if self.pos + 8 >= PACKET_MAXSIZE { return Err(()); }
        if self.pos + 8 >= self.len { self.len += 8; }
        let bytes = v.to_le_bytes();
        self.buf[self.pos..self.pos + 8].copy_from_slice(&bytes);
        self.pos += 8;
        Ok(())
    }

    pub fn write_f32(&mut self, v: f32) -> Result<(), ()> {
        if self.pos + 4 >= PACKET_MAXSIZE { return Err(()); }
        if self.pos + 4 >= self.len { self.len += 4; }
        let bytes = v.to_le_bytes();
        self.buf[self.pos..self.pos + 4].copy_from_slice(&bytes);
        self.pos += 4;
        Ok(())
    }

    pub fn write_f64(&mut self, v: f64) -> Result<(), ()> {
        if self.pos + 8 >= PACKET_MAXSIZE { return Err(()); }
        if self.pos + 8 >= self.len { self.len += 8; }
        let bytes = v.to_le_bytes();
        self.buf[self.pos..self.pos + 8].copy_from_slice(&bytes);
        self.pos += 8;
        Ok(())
    }


    pub fn write_str(&mut self, s: &str) -> Result<(), ()> {
        for b in s.bytes() {
            self.write_u8(b)?;
        }
        self.write_u8(0)?;
        Ok(())
    }


    pub fn write_str_lower(&mut self, s: &str) -> Result<(), ()> {
        let lower = s.to_lowercase();
        self.write_str(&lower)
    }
}

pub fn str_unicode_len(s: &str) -> usize {
    s.chars().count()
}

pub fn str_to_lower(s: &str) -> String {
    s.to_lowercase()
}
