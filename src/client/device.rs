//! os_get_info()[? "udid"] of the original: an id of this computer, sent to servers,
//! which bans, timeouts and the one-connection-per-device rule use.
//!
//! It is the system's own machine id, sent as it is, so it is kept in no file of the
//! game (where anyone could edit it away) and is the same for every copy of the game on
//! the computer.

use std::sync::OnceLock;

/// The id this client sends in IDENTITY, the same for as long as the game runs.
pub fn device_id() -> String {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| {
        machine_id().unwrap_or_else(|| {
            // Nothing to take it from: an id for this run only, as a new computer would have.
            eprintln!("no machine id found: the device id changes every start");
            format!("{:x}{:x}", macroquad::miniquad::date::now().to_bits(), std::process::id())
        })
    })
    .clone()
}

/// systemd's machine id, or D-Bus's where there is no systemd.
#[cfg(target_os = "linux")]
fn machine_id() -> Option<String> {
    ["/etc/machine-id", "/var/lib/dbus/machine-id"]
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok())
        .map(|text| text.trim().to_string())
        .filter(|id| !id.is_empty())
}

/// MachineGuid of HKLM\SOFTWARE\Microsoft\Cryptography, which Windows makes on install.
#[cfg(windows)]
fn machine_id() -> Option<String> {
    let output = std::process::Command::new("reg")
        .args(["query", r"HKLM\SOFTWARE\Microsoft\Cryptography", "/v", "MachineGuid"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().find(|line| line.contains("MachineGuid"))?;
    line.split_whitespace().last().map(str::to_string)
}

#[cfg(not(any(target_os = "linux", windows)))]
fn machine_id() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_id_stays_the_same_while_the_game_runs() {
        let id = device_id();
        assert!(!id.is_empty());
        assert_eq!(id, device_id());
        #[cfg(target_os = "linux")]
        assert_eq!(Some(id), machine_id(), "the machine id itself");
    }
}
