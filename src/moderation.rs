use rusqlite::{params, Connection};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const DB_FILE: &str = "moderation.db";

static DB: OnceLock<Mutex<Connection>> = OnceLock::new();

fn db() -> &'static Mutex<Connection> {
    DB.get().expect("moderation::init_moderation() not called")
}

/// Acquire the moderation DB lock, recovering from poisoning instead of panicking.
///
/// If a worker thread panics while holding this lock the mutex becomes poisoned;
/// a plain `.lock().unwrap()` would then panic in *every other* worker that touches
/// moderation, cascading a single instance's panic across all instances. The DB
/// connection itself stays valid after a panic, so we take the inner guard and
/// carry on. (`shared.peers` is handled the same way in worker.rs.)
fn db_lock() -> MutexGuard<'static, Connection> {
    db().lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn init_moderation() {
    if DB.get().is_some() {
        return;
    }

    let is_new_db = !std::path::Path::new(DB_FILE).exists();

    let conn = Connection::open(DB_FILE).expect("failed to open moderation.db");

    conn.execute_batch(
        "PRAGMA journal_mode = DELETE;
         CREATE TABLE IF NOT EXISTS blacklist (
             id       INTEGER PRIMARY KEY AUTOINCREMENT,
             ip       TEXT NOT NULL DEFAULT '',
             udid     TEXT NOT NULL DEFAULT '',
             nickname TEXT NOT NULL DEFAULT '',
             note     TEXT NOT NULL DEFAULT ''
         );
         CREATE TABLE IF NOT EXISTS whitelist (
             id       INTEGER PRIMARY KEY AUTOINCREMENT,
             ip       TEXT NOT NULL DEFAULT '',
             udid     TEXT NOT NULL DEFAULT '',
             nickname TEXT NOT NULL DEFAULT '',
             note     TEXT NOT NULL DEFAULT ''
         );
         CREATE TABLE IF NOT EXISTS timeouts (
             id         INTEGER PRIMARY KEY AUTOINCREMENT,
             ip         TEXT NOT NULL DEFAULT '',
             udid       TEXT NOT NULL DEFAULT '',
             nickname   TEXT NOT NULL DEFAULT '',
             note       TEXT NOT NULL DEFAULT '',
             expires_at INTEGER NOT NULL DEFAULT 0
         );
         CREATE TABLE IF NOT EXISTS ops (
             id       INTEGER PRIMARY KEY AUTOINCREMENT,
             ip       TEXT NOT NULL DEFAULT '',
             udid     TEXT NOT NULL DEFAULT '',
             nickname TEXT NOT NULL DEFAULT '',
             note     TEXT NOT NULL DEFAULT '',
             level    INTEGER NOT NULL DEFAULT 1
         );",
    )
    .expect("failed to initialize moderation tables");

    // Seed the default localhost op only on a brand-new database.
    // If the file already existed and localhost was removed intentionally, do not restore it.
    if is_new_db {
        conn.execute(
            "INSERT INTO ops (ip, udid, nickname, note, level) VALUES ('127.0.0.1', '', '', 'localhost', 3)",
            [],
        ).expect("failed to insert default localhost op");
    }

    DB.set(Mutex::new(conn)).ok();
}

pub fn ban_add(nickname: &str, udid: &str, ip: &str, note: &str) -> bool {
    let cfg = crate::config::cfg();
    let conn = db_lock();

    let _ = conn.execute(
        "DELETE FROM blacklist WHERE (ip != '' AND ip = ?1) OR (udid != '' AND udid = ?2) OR (nickname != '' AND nickname = ?3)",
        params![ip, udid, nickname],
    );

    let stored_ip       = if cfg.states.lobby_misc.moderation.ban_ip       { ip }       else { "" };
    let stored_udid     = if cfg.states.lobby_misc.moderation.ban_udid     { udid }     else { "" };
    let stored_nickname = if cfg.states.lobby_misc.moderation.ban_nickname { nickname } else { "" };

    if stored_ip.is_empty() && stored_udid.is_empty() && stored_nickname.is_empty() {
        return true;
    }

    let _ = conn.execute(
        "INSERT INTO blacklist (ip, udid, nickname, note) VALUES (?1, ?2, ?3, ?4)",
        params![stored_ip, stored_udid, stored_nickname, note],
    );

    let _ = conn.execute(
        "DELETE FROM whitelist WHERE (ip != '' AND ip = ?1) OR (udid != '' AND udid = ?2) OR (nickname != '' AND nickname = ?3)",
        params![ip, udid, nickname],
    );
    let _ = conn.execute(
        "DELETE FROM ops WHERE (ip != '' AND ip = ?1) OR (udid != '' AND udid = ?2) OR (nickname != '' AND nickname = ?3)",
        params![ip, udid, nickname],
    );
    true
}

pub fn ban_ip_only(ip: &str, note: &str) -> bool {
    let conn = db_lock();
    let _ = conn.execute("DELETE FROM blacklist WHERE ip = ?1 AND ip != ''", params![ip]);
    let ok = conn.execute(
        "INSERT INTO blacklist (ip, udid, nickname, note) VALUES (?1, '', '', ?2)",
        params![ip, note],
    ).is_ok();
    if ok {
        let _ = conn.execute("DELETE FROM whitelist WHERE ip = ?1 AND ip != ''", params![ip]);
        let _ = conn.execute("DELETE FROM ops WHERE ip = ?1 AND ip != ''", params![ip]);
    }
    ok
}

pub fn ban_revoke_full(nickname: &str, udid: &str, ip: &str) -> bool {
    let conn = db_lock();
    conn.execute(
        "DELETE FROM blacklist WHERE (ip != '' AND ip = ?1) OR (udid != '' AND udid = ?2) OR (nickname != '' AND nickname = ?3)",
        params![ip, udid, nickname],
    ).unwrap_or(0) > 0
}

pub fn ban_revoke_by_ip(ip: &str) -> bool {
    let conn = db_lock();
    conn.execute("DELETE FROM blacklist WHERE ip = ?1 AND ip != ''", params![ip]).unwrap_or(0) > 0
}

pub fn ban_check(nickname: &str, udid: &str, ip: &str) -> bool {
    let cfg = crate::config::cfg();
    let conn = db_lock();

    if cfg.states.lobby_misc.moderation.ban_ip && !ip.is_empty() {
        if conn.query_row("SELECT 1 FROM blacklist WHERE ip = ?1 LIMIT 1",
            params![ip], |_| Ok(true)).unwrap_or(false) { return true; }
    }
    if cfg.states.lobby_misc.moderation.ban_udid && !udid.is_empty() {
        if conn.query_row("SELECT 1 FROM blacklist WHERE udid = ?1 LIMIT 1",
            params![udid], |_| Ok(true)).unwrap_or(false) { return true; }
    }
    if cfg.states.lobby_misc.moderation.ban_nickname && !nickname.is_empty() {
        if conn.query_row("SELECT 1 FROM blacklist WHERE nickname = ?1 LIMIT 1",
            params![nickname], |_| Ok(true)).unwrap_or(false) { return true; }
    }
    false
}

pub struct BlacklistEntry {
    pub ip:       String,
    pub udid:     String,
    pub nickname: String,
    pub note:     String,
}

pub fn list_blacklist() -> Vec<BlacklistEntry> {
    let conn = db_lock();
    let mut stmt = match conn.prepare(
        "SELECT ip, udid, nickname, note FROM blacklist ORDER BY id DESC"
    ) { Ok(stmt) => stmt, Err(_) => return vec![] };
    let Ok(rows) = stmt.query_map([], |row| Ok(BlacklistEntry {
        ip:       row.get(0)?,
        udid:     row.get(1)?,
        nickname: row.get(2)?,
        note:     row.get(3)?,
    })) else { return vec![]; };
    rows.flatten().collect()
}

pub fn whitelist_add_full(nickname: &str, udid: &str, ip: &str, note: &str) -> bool {
    let conn = db_lock();
    let _ = conn.execute(
        "DELETE FROM whitelist WHERE (ip != '' AND ip = ?1) OR (udid != '' AND udid = ?2) OR (nickname != '' AND nickname = ?3)",
        params![ip, udid, nickname],
    );
    conn.execute(
        "INSERT INTO whitelist (ip, udid, nickname, note) VALUES (?1, ?2, ?3, ?4)",
        params![ip, udid, nickname, note],
    ).is_ok()
}

pub fn whitelist_revoke(ip: &str) -> bool {
    let conn = db_lock();
    conn.execute("DELETE FROM whitelist WHERE ip = ?1 AND ip != ''", params![ip]).unwrap_or(0) > 0
}

pub fn whitelist_revoke_full(nickname: &str, udid: &str, ip: &str) -> bool {
    let conn = db_lock();
    conn.execute(
        "DELETE FROM whitelist WHERE (ip != '' AND ip = ?1) OR (udid != '' AND udid = ?2) OR (nickname != '' AND nickname = ?3)",
        params![ip, udid, nickname],
    ).unwrap_or(0) > 0
}

/// Matched only by udid and ip, never by nickname - see op_check's doc
/// comment. Matching on nickname would let anyone bypass the whitelist gate by
/// connecting under a whitelisted player's unauthenticated display name.
pub fn whitelist_check(udid: &str, ip: &str) -> bool {
    let conn = db_lock();
    conn.query_row(
        "SELECT 1 FROM whitelist WHERE (ip != '' AND ip = ?1) OR (udid != '' AND udid = ?2) LIMIT 1",
        params![ip, udid],
        |_| Ok(true),
    ).unwrap_or(false)
}

pub fn timeout_set(nickname: &str, udid: &str, ip: &str, expires_at: u64, note: &str) -> bool {
    let conn = db_lock();
    let _ = conn.execute(
        "DELETE FROM timeouts WHERE (udid != '' AND udid = ?1) OR (ip != '' AND ip = ?2) OR (nickname != '' AND nickname = ?3)",
        params![udid, ip, nickname],
    );
    conn.execute(
        "INSERT INTO timeouts (ip, udid, nickname, note, expires_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![ip, udid, nickname, note, expires_at as i64],
    ).is_ok()
}

pub fn timeout_revoke(udid: &str, ip: &str) -> bool {
    let conn = db_lock();
    conn.execute(
        "DELETE FROM timeouts WHERE (udid != '' AND udid = ?1) OR (ip != '' AND ip = ?2)",
        params![udid, ip],
    ).unwrap_or(0) > 0
}

pub fn timeout_check(udid: &str, ip: &str) -> Option<u64> {
    let conn = db_lock();
    let now = now_unix_i64();

    let result: Option<i64> = conn.query_row(
        "SELECT expires_at FROM timeouts WHERE (udid != '' AND udid = ?1) OR (ip != '' AND ip = ?2) ORDER BY expires_at DESC LIMIT 1",
        params![udid, ip],
        |row| row.get(0),
    ).ok();

    if let Some(ts) = result {
        if ts > now {
            return Some(ts as u64);
        }
        let _ = conn.execute(
            "DELETE FROM timeouts WHERE ((udid != '' AND udid = ?1) OR (ip != '' AND ip = ?2)) AND expires_at <= ?3",
            params![udid, ip, now],
        );
    }
    None
}

pub fn cleanup_expired_timeouts() {
    let conn = db_lock();
    let _ = conn.execute("DELETE FROM timeouts WHERE expires_at <= ?1", params![now_unix_i64()]);
}

pub struct TimeoutEntry {
    pub ip:         String,
    pub udid:       String,
    pub nickname:   String,
    pub note:       String,
    pub expires_at: i64,
}

pub fn list_timeouts() -> Vec<TimeoutEntry> {
    let conn = db_lock();
    let mut stmt = match conn.prepare(
        "SELECT ip, udid, nickname, note, expires_at FROM timeouts WHERE expires_at > ?1 ORDER BY expires_at"
    ) { Ok(stmt) => stmt, Err(_) => return vec![] };
    let Ok(rows) = stmt.query_map(params![now_unix_i64()], |row| Ok(TimeoutEntry {
        ip:         row.get(0)?,
        udid:       row.get(1)?,
        nickname:   row.get(2)?,
        note:       row.get(3)?,
        expires_at: row.get(4)?,
    })) else { return vec![]; };
    rows.flatten().collect()
}

pub fn op_add(nickname: &str, udid: &str, ip: &str, note: &str) -> bool {
    let cfg = crate::config::cfg();
    let level = cfg.states.lobby_misc.moderation.op_default_level as i64;
    let conn = db_lock();
    // Match existing operators by udid/ip only - never by nickname. The op grant is
    // anchored to udid+ip (the same keys op_check uses); the nickname column is stored
    // for display only and must not influence privilege (see op_check).
    let updated = conn.execute(
        "UPDATE ops SET level = ?1, note = ?2, nickname = ?3 WHERE (udid != '' AND udid = ?4) OR (ip != '' AND ip = ?5)",
        params![level, note, nickname, udid, ip],
    ).unwrap_or(0);
    if updated == 0 {
        let _ = conn.execute(
            "INSERT INTO ops (ip, udid, nickname, note, level) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![ip, udid, nickname, note, level],
        );
    }
    true
}

pub fn op_set(ip: &str, level: u8, note: &str) -> bool {
    let conn = db_lock();
    if level == 0 {
        conn.execute("DELETE FROM ops WHERE ip = ?1 AND ip != ''", params![ip]).unwrap_or(0) > 0
    } else {
        let updated = conn.execute(
            "UPDATE ops SET level = ?1, note = ?2 WHERE ip = ?3 AND ip != ''",
            params![level as i64, note, ip],
        ).unwrap_or(0);
        if updated == 0 {
            conn.execute(
                "INSERT INTO ops (ip, udid, nickname, note, level) VALUES (?1, '', '', ?2, ?3)",
                params![ip, note, level as i64],
            ).is_ok()
        } else {
            true
        }
    }
}

/// Set (or, with `level == 0`, remove) an operator anchored on udid+ip.
///
/// Privilege is keyed on udid/ip only; `nickname` is stored for display and
/// never used for matching, since the nickname is unauthenticated (see op_check).
pub fn op_set_full(nickname: &str, udid: &str, ip: &str, level: u8, note: &str) -> bool {
    let conn = db_lock();
    if level == 0 {
        conn.execute(
            "DELETE FROM ops WHERE (udid != '' AND udid = ?1) OR (ip != '' AND ip = ?2)",
            params![udid, ip],
        ).unwrap_or(0) > 0
    } else {
        let updated = conn.execute(
            "UPDATE ops SET level = ?1, note = ?2, nickname = ?3 WHERE (udid != '' AND udid = ?4) OR (ip != '' AND ip = ?5)",
            params![level as i64, note, nickname, udid, ip],
        ).unwrap_or(0);
        if updated == 0 {
            conn.execute(
                "INSERT INTO ops (ip, udid, nickname, note, level) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![ip, udid, nickname, note, level as i64],
            ).is_ok()
        } else {
            true
        }
    }
}

/// Resolve a connecting player's operator level.
///
/// Operator status is matched **only by udid and ip**, never by nickname.
/// The nickname is an unauthenticated, publicly-visible string from the identity
/// packet - matching on it would let anyone inherit an operator's level simply by
/// connecting under that operator's display name (privilege escalation, including
/// op>=2 which bypasses ip_validation and grants ban/kick). udid is per-device and
/// not publicly displayed, so udid+ip is the trust anchor. The default localhost op
/// (ip 127.0.0.1) is unaffected.
pub fn op_check(udid: &str, ip: &str) -> u8 {
    let conn = db_lock();
    conn.query_row(
        "SELECT MAX(level) FROM ops WHERE (ip != '' AND ip = ?1) OR (udid != '' AND udid = ?2)",
        params![ip, udid],
        |row| row.get::<_, Option<i64>>(0),
    )
    .ok()
    .flatten()
    .map(|level| level.clamp(0, 255) as u8)
    .unwrap_or(0)
}

pub struct OpEntry {
    pub ip:       String,
    pub udid:     String,
    pub nickname: String,
    pub note:     String,
    pub level:    i64,
}

pub fn list_ops() -> Vec<OpEntry> {
    let conn = db_lock();
    let mut stmt = match conn.prepare(
        "SELECT ip, udid, nickname, note, level FROM ops ORDER BY level DESC, ip"
    ) { Ok(stmt) => stmt, Err(_) => return vec![] };
    let Ok(rows) = stmt.query_map([], |row| Ok(OpEntry {
        ip:       row.get(0)?,
        udid:     row.get(1)?,
        nickname: row.get(2)?,
        note:     row.get(3)?,
        level:    row.get(4)?,
    })) else { return vec![]; };
    rows.flatten().collect()
}

fn now_unix_i64() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since_epoch| since_epoch.as_secs())
        .unwrap_or(0) as i64
}

pub fn now_unix() -> u64 {
    now_unix_i64() as u64
}
