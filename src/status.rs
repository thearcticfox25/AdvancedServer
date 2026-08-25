use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Status {
    pub surv_win_rounds: u32,
    pub exe_win_rounds: u32,
    pub exe_crashed_rounds: u32,
    pub draw_rounds: u32,
    pub tails_shots: u32,
    pub exeller_clones_placed: u32,
    pub exeller_clones_activated: u32,
    pub timeouts: u32,
    pub damage_taken: u32,
    pub total_escaped: u32,
    pub total_died: u32,
    pub total_demonised: u32,
    pub eggman_mines_placed: u32,
    pub cream_rings_spawned: u32,
    pub tails_hits: u32,
    pub total_stuns: u32,
    pub exetior_bring_spawned: u32,
}

static STATUS: OnceLock<Mutex<Status>> = OnceLock::new();

pub fn init_status() {
    let status = load_status();
    STATUS.get_or_init(|| Mutex::new(status));
}

fn load_status() -> Status {
    match std::fs::read_to_string(STATUS_FILE) {
        Ok(s) => match serde_json::from_str(&s) {
            Ok(status) => status,
            Err(e) => {
                log::error!("Failed to parse status file: {}", e);
                Status::default()
            }
        },
        Err(_) => Status::default(),
    }
}

const STATUS_FILE: &str = "Status.json";

/// Shared by save_status/save_status_best_effort. `log_errors` is a caller
/// choice, not an oversight: log::warn! itself isn't async-signal-safe, so
/// the crash-handler path must stay silent (see save_status_best_effort).
fn write_status(s: &Status, log_errors: bool) {
    if let Ok(json) = serde_json::to_string_pretty(s) {
        if let Err(e) = std::fs::write(STATUS_FILE, json) {
            if log_errors {
                log::warn!("Could not open status file {} for writing: {}", STATUS_FILE, e);
            }
        }
    }
}

pub fn save_status() {
    if let Some(mutex) = STATUS.get() {
        if let Ok(s) = mutex.lock() {
            write_status(&s, true);
        }
    }
}

/// Best-effort save for use from a crash-signal handler: never blocks (try_lock,
/// so a crash that happened while this thread already held the status lock
/// can't deadlock the exit path -- std::sync::Mutex isn't reentrant), and
/// never logs on failure (see write_status).
pub fn save_status_best_effort() {
    if let Some(mutex) = STATUS.get() {
        if let Ok(s) = mutex.try_lock() {
            write_status(&s, false);
        }
    }
}

pub fn with_status<F, R>(f: F) -> R
where
    F: FnOnce(&mut Status) -> R,
{
    let mutex = STATUS.get().expect("Status not initialized");
    let mut s = mutex.lock().unwrap();
    f(&mut s)
}