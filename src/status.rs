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
    const STATUS_FILE: &str = "Status.json";
    std::fs::read_to_string(STATUS_FILE)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_status() {
    const STATUS_FILE: &str = "Status.json";
    if let Some(mutex) = STATUS.get() {
        if let Ok(s) = mutex.lock() {
            if let Ok(json) = serde_json::to_string_pretty(&*s) {
                let _ = std::fs::write(STATUS_FILE, json);
            }
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