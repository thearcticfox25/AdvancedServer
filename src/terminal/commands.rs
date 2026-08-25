use std::collections::HashMap;
use std::sync::{mpsc::SyncSender, Arc};
use crate::server::{PeerSummary, ServerShared};
use super::ConsoleCmd;

pub struct TerminalCmd {
    pub name: String,
    pub args: Vec<String>,
}

impl TerminalCmd {
    pub fn arg(&self, i: usize) -> &str {
        self.args.get(i).map(String::as_str).unwrap_or("")
    }

    pub fn rest(&self) -> String {
        self.args.join(" ")
    }
}

pub fn parse_terminal_cmd(input: &str) -> Option<TerminalCmd> {
    let s = input.trim();
    if !s.starts_with('?') {
        return None;
    }
    let after = s[1..].trim_start();
    if after.is_empty() {
        return None;
    }
    let (name_part, rest) = match after.find(char::is_whitespace) {
        Some(i) => (&after[..i], after[i + 1..].trim()),
        None => (after, ""),
    };
    let name = name_part.to_lowercase();
    if name.is_empty() {
        return None;
    }
    let args: Vec<String> = rest.split_whitespace().map(String::from).collect();
    Some(TerminalCmd { name, args })
}

pub struct TerminalCtx<'a> {
    pub senders: &'a [SyncSender<ConsoleCmd>],
    pub shared: &'a [Arc<ServerShared>],
    pub current_lobby: &'a mut usize,
    pub should_exit: &'a mut bool,
    pub print: &'a mut dyn FnMut(&str),
}

struct ParsedArgs {
    values: HashMap<String, String>,
}

impl ParsedArgs {
    fn parse(args: &[String], aliases: &[(&str, &str)]) -> Self {
        let alias_map: HashMap<&str, &str> = aliases.iter().map(|&(short, long)| (short, long)).collect();
        let mut values: HashMap<String, String> = HashMap::new();
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];
            let key = if arg.starts_with("--") {
                arg[2..].to_string()
            } else if arg.starts_with('-') && arg.len() >= 2 {
                let short = &arg[1..];
                alias_map.get(short).map(|s| s.to_string()).unwrap_or_else(|| short.to_string())
            } else {
                i += 1;
                continue;
            };

            i += 1;
            let mut parts = Vec::new();
            while i < args.len() && !args[i].starts_with('-') {
                parts.push(args[i].clone());
                i += 1;
            }
            values.insert(key, parts.join(" "));
        }

        Self { values }
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    fn has(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }
}

fn print_table(print: &mut dyn FnMut(&str), headers: &[&str], rows: &[Vec<String>]) {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if let Some(w) = widths.get_mut(i) {
                *w = (*w).max(cell.len());
            }
        }
    }

    let fmt_row = |cells: &[&str]| -> String {
        cells.iter().enumerate()
            .map(|(i, c)| format!("{:<width$}", c, width = widths.get(i).copied().unwrap_or(0)))
            .collect::<Vec<_>>()
            .join(" | ")
    };
    let sep: String = widths.iter().map(|&w| "-".repeat(w)).collect::<Vec<_>>().join("-+-");

    let header_strs: Vec<&str> = headers.to_vec();
    (print)(&fmt_row(&header_strs));
    (print)(&sep);

    if rows.is_empty() {
        (print)("  (empty)");
        return;
    }
    for row in rows {
        let strs: Vec<&str> = row.iter().map(String::as_str).collect();
        (print)(&fmt_row(&strs));
    }
}

fn strip(s: &str) -> String { crate::colors::strip(s) }

fn find_by_pid(pid: u16, peers: &[PeerSummary]) -> Result<&PeerSummary, String> {
    peers.iter().find(|p| p.id == pid)
        .ok_or_else(|| format!("no player with PID {}", pid))
}

fn find_by_ip<'a>(ip: &str, peers: &'a [PeerSummary]) -> Option<&'a PeerSummary> {
    peers.iter().find(|p| p.ip == ip)
}

fn find_by_username<'a>(name: &str, peers: &'a [PeerSummary]) -> Result<&'a PeerSummary, String> {
    let target = strip(name).to_lowercase();
    let matches: Vec<&PeerSummary> = peers.iter()
        .filter(|p| strip(&p.nickname).to_lowercase() == target)
        .collect();
    match matches.len() {
        0 => Err(format!("no player with username '{}'", name)),
        1 => Ok(matches[0]),
        _ => Err(format!("ambiguous: {} players share that username (stripped)", matches.len())),
    }
}

fn find_by_udid<'a>(udid: &str, peers: &'a [PeerSummary]) -> Result<&'a PeerSummary, String> {
    let matches: Vec<&PeerSummary> = peers.iter().filter(|p| p.udid == udid).collect();
    match matches.len() {
        0 => Err(format!("no player with UDID '{}'", udid)),
        1 => Ok(matches[0]),
        _ => Err(format!("ambiguous: {} players share that UDID", matches.len())),
    }
}

fn resolve_target<'a>(
    args: &ParsedArgs,
    peers: &'a [PeerSummary],
) -> Result<&'a PeerSummary, String> {
    let mut specified = 0u8;
    if args.has("pid")      { specified += 1; }
    if args.has("endpoint") { specified += 1; }
    if args.has("username") { specified += 1; }
    if args.has("udid")     { specified += 1; }

    if specified == 0 {
        return Err("no target specified".to_string());
    }
    if specified > 1 {
        return Err("only one target argument is allowed".to_string());
    }

    if let Some(v) = args.get("pid") {
        let pid: u16 = v.parse().map_err(|_| format!("invalid PID: '{}'", v))?;
        find_by_pid(pid, peers)
    } else if let Some(v) = args.get("endpoint") {
        find_by_ip(v, peers).ok_or_else(|| format!("no online player with IP '{}'", v))
    } else if let Some(v) = args.get("username") {
        find_by_username(v, peers)
    } else if let Some(v) = args.get("udid") {
        find_by_udid(v, peers)
    } else {
        Err("no target specified".to_string())
    }
}

const SELECT_PLAYER_ALIASES: &[(&str, &str)] = &[
    ("p", "pid"),
    ("e", "endpoint"),
    ("n", "username"),
    ("u", "udid"),
    ("a", "action"),
    ("l", "op-level"),
    ("N", "note"),
];

fn select_player_usage(print: &mut dyn FnMut(&str)) {
    (print)("usage: ?select-player <selector> --action/-a <action> [options]");
    (print)("  selectors (pick exactly one):");
    (print)("    --pid/-p <PID>          select by peer ID (online only)");
    (print)("    --endpoint/-e <ip>      select by IP address");
    (print)("    --username/-n <name>    select by username (stripped, case-insensitive)");
    (print)("    --udid/-u <UDID>        select by UDID");
    (print)("  actions:");
    (print)("    -a kick               kick (temp timeout)");
    (print)("    -a ban                ban the player");
    (print)("    -a ban-revoke         alias for pardon");
    (print)("    -a pardon             remove all bans and timeouts");
    (print)("    -a op                 grant operator status");
    (print)("    -a deop               revoke operator status  (shorthand for -a op -l 0)");
    (print)("    -a whitelist-append   add player to whitelist");
    (print)("    -a whitelist-revoke   remove player from whitelist");
    (print)("  options:");
    (print)("    --note/-N <text>        note for ban/kick/op/whitelist  (default: username or IP)");
    (print)("    --op-level/-l <0-255>   op level for -a op  (0 = revoke; default = server default)");
}

pub fn exec_terminal_cmd(cmd: &TerminalCmd, ctx: &mut TerminalCtx) {
    match cmd.name.as_str() {
        "help" => {
            (ctx.print)("--- terminal commands (prefix: ?) ---");
            (ctx.print)("  ?help                      show this list");
            (ctx.print)("  ?info                      show version and credits");
            (ctx.print)("  ?lobby <n>                 switch active lobby (1-indexed)");
            (ctx.print)("  ?playerlist                list players (table view)");
            (ctx.print)("  ?playerlist --banned/-b    list banned IPs / UDIDs / nicknames");
            (ctx.print)("  ?playerlist --timeouts/-t  list active timeouts");
            (ctx.print)("  ?playerlist --admins/-a    list administrators");
            (ctx.print)("  ?say <msg>                 send a server message to active lobby");
            (ctx.print)("  ?broadcast <msg>           send a server message to all lobbies");
            (ctx.print)("  ?select-player <selector> --action/-a <action>  [--note/-N <text>]  [--op-level/-l <0-255>]");
            (ctx.print)("    selectors:  --pid/-p <PID> | --endpoint/-e <ip> | --username/-n <name> | --udid/-u <UDID>");
            (ctx.print)("    actions:    ban | ban-revoke | kick | op | deop | pardon | whitelist-append | whitelist-revoke");
            (ctx.print)("  ?license                   print embedded license text");
            (ctx.print)("  ?halt                      kills the entire server");
            (ctx.print)("  ?start                     force-start a game in active lobby");
            (ctx.print)("  ?start --map/-m <id>       force-start, skipping MapVote straight to CharSelect with map <id>");
            (ctx.print)("  ?stop                      force-end the current round");
            (ctx.print)("---");
        }

        "lobby" => {
            let raw: usize = match cmd.arg(0).parse::<usize>() {
                Ok(v) if v >= 1 => v - 1,
                _ => {
                    (ctx.print)("usage: ?lobby <n>  (lobbies are 1-indexed)");
                    return;
                }
            };
            if raw >= ctx.senders.len() {
                (ctx.print)(&format!(
                    "lobby {} does not exist (valid: 1..{})",
                    raw + 1,
                    ctx.senders.len()
                ));
                return;
            }
            *ctx.current_lobby = raw;
            (ctx.print)(&format!("active lobby: {}", raw + 1));
        }

        "playerlist" => {
            let args = ParsedArgs::parse(&cmd.args, &[("b", "banned"), ("t", "timeouts"), ("a", "admins")]);

            if args.has("banned") {
                let entries = crate::moderation::list_blacklist();
                (ctx.print)(&format!("--- blacklist ({}) ---", entries.len()));
                let rows: Vec<Vec<String>> = entries.iter().map(|e| {
                    vec![e.ip.clone(), e.udid.clone(), e.nickname.clone(), e.note.clone()]
                }).collect();
                print_table(ctx.print, &["IP", "UDID", "Nickname", "Note"], &rows);
                return;
            }

            if args.has("timeouts") {
                use std::time::{SystemTime, UNIX_EPOCH};
                let entries = crate::moderation::list_timeouts();
                (ctx.print)(&format!("--- active timeouts ({}) ---", entries.len()));

                let rows: Vec<Vec<String>> = entries.iter().map(|e| {
                    let secs_left = e.expires_at.saturating_sub(
                        SystemTime::now().duration_since(UNIX_EPOCH)
                            .map(|d| d.as_secs()).unwrap_or(0) as i64
                    );
                    let exp = format!("{}s remaining", secs_left.max(0));
                    vec![e.ip.clone(), e.udid.clone(), e.nickname.clone(), exp, e.note.clone()]
                }).collect();

                print_table(ctx.print, &["IP", "UDID", "Nickname", "Expires", "Note"], &rows);
                return;
            }

            if args.has("admins") {
                let entries = crate::moderation::list_ops();
                (ctx.print)(&format!("--- administrators ({}) ---", entries.len()));

                let rows: Vec<Vec<String>> = entries.iter().map(|e| {
                    vec![e.ip.clone(), e.udid.clone(), e.nickname.clone(), e.level.to_string(), e.note.clone()]
                }).collect();

                print_table(ctx.print, &["IP", "UDID", "Nickname", "Level", "Note"], &rows);
                return;
            }

            if let Some(shared) = ctx.shared.get(*ctx.current_lobby) {
                if let Ok(peers) = shared.peers.read() {
                    (ctx.print)(&format!(
                        "--- lobby {} | {} player(s) ---",
                        *ctx.current_lobby + 1,
                        peers.len()
                    ));

                    let rows: Vec<Vec<String>> = peers.iter().map(|p| {
                        vec![
                            p.id.to_string(),
                            p.nickname.clone(),
                            p.ip.clone(),
                            p.udid.clone(),
                            p.op.to_string(),
                            if p.mod_tool { "yes" } else { "no" }.to_string(),
                            if p.is_mobile { "yes" } else { "no" }.to_string(),
                            if p.in_game { "in game" } else { "waiting" }.to_string(),
                        ]
                    }).collect();

                    print_table(
                        ctx.print,
                        &["PID", "Username", "IP Endpoint", "UDID", "OP Level", "Modified", "Mobile", "Status"],
                        &rows,
                    );
                }
            }
        }

        "select-player" => {
            if cmd.args.is_empty() {
                select_player_usage(ctx.print);
                return;
            }

            let args = ParsedArgs::parse(&cmd.args, SELECT_PLAYER_ALIASES);

            let action = match args.get("action") {
                Some(a) => a.to_string(),
                None => {
                    (ctx.print)("error: --action/-a is required");
                    select_player_usage(ctx.print);
                    return;
                }
            };

            match action.as_str() {
                "kick" => {
                    let shared = match ctx.shared.get(*ctx.current_lobby) {
                        Some(s) => s, None => return,
                    };
                    let peers = match shared.peers.read() {
                        Ok(g) => g, Err(_) => return,
                    };
                    let peer = match resolve_target(&args, &peers) {
                        Ok(p) => p,
                        Err(e) => { (ctx.print)(&format!("error: {}", e)); select_player_usage(ctx.print); return; }
                    };
                    let note = args.get("note").unwrap_or("").to_string();
                    let note = if note.is_empty() { peer.nickname.clone() } else { note };
                    let (pid, nick, udid, ip) = (peer.id, peer.nickname.clone(), peer.udid.clone(), peer.ip.clone());
                    drop(peers);

                    let kick_secs = crate::config::cfg().server_config.pairing.kick_timeout_window as u64;
                    crate::moderation::timeout_set(&nick, &udid, &ip,
                        crate::moderation::now_unix() + kick_secs, &note);
                    send_to_current(ctx, &format!(".kick_now {}", pid));
                    (ctx.print)(&format!("kicked '{}' (pid={}, ip={})", crate::colors::colorize(&nick), pid, ip));
                }

                "ban" => {
                    let endpoint_only = args.has("endpoint")
                        && !args.has("pid") && !args.has("username") && !args.has("udid");

                    let shared = match ctx.shared.get(*ctx.current_lobby) {
                        Some(s) => s, None => return,
                    };
                    let peers = match shared.peers.read() {
                        Ok(g) => g, Err(_) => return,
                    };

                    if endpoint_only {
                        let ip = args.get("endpoint").unwrap_or("").to_string();
                        if ip.is_empty() { (ctx.print)("error: --endpoint requires a value"); return; }
                        let note = args.get("note").unwrap_or("").to_string();
                        let (note, pid_opt) = if let Some(p) = find_by_ip(&ip, &peers) {
                            let n = if note.is_empty() { p.nickname.clone() } else { note };
                            let pid = p.id;
                            let (nick, udid) = (p.nickname.clone(), p.udid.clone());
                            drop(peers);
                            crate::moderation::ban_add(&nick, &udid, &ip, &n);
                            (n, Some(pid))
                        } else {
                            drop(peers);
                            let n = if note.is_empty() { ip.clone() } else { note };
                            crate::moderation::ban_ip_only(&ip, &n);
                            (n, None)
                        };
                        if let Some(pid) = pid_opt {
                            send_to_current(ctx, &format!(".ban_now {}", pid));
                            (ctx.print)(&format!("banned IP {} (player online, disconnected; note: {})", ip, note));
                        } else {
                            (ctx.print)(&format!("banned IP {} (player offline; note: {})", ip, note));
                        }
                        return;
                    }

                    if args.has("username") && crate::config::cfg().states.lobby_misc.moderation.aggressive_username_ban {
                        let name = args.get("username").unwrap_or("");
                        let target = strip(name).to_lowercase();
                        let victims: Vec<(u16, String, String, String)> = peers.iter()
                            .filter(|p| strip(&p.nickname).to_lowercase() == target)
                            .map(|p| (p.id, p.nickname.clone(), p.udid.clone(), p.ip.clone()))
                            .collect();
                        drop(peers);
                        if victims.is_empty() {
                            (ctx.print)(&format!("error: no online player with username '{}'", name));
                            return;
                        }
                        let note_arg = args.get("note").unwrap_or("").to_string();
                        for (pid, nick, udid, ip) in victims {
                            let note = if note_arg.is_empty() { nick.clone() } else { note_arg.clone() };
                            crate::moderation::ban_add(&nick, &udid, &ip, &note);
                            send_to_current(ctx, &format!(".ban_now {}", pid));
                            (ctx.print)(&format!("banned '{}' (pid={}, ip={})", crate::colors::colorize(&nick), pid, ip));
                        }
                        return;
                    }

                    let peer = match resolve_target(&args, &peers) {
                        Ok(p) => p,
                        Err(e) => { (ctx.print)(&format!("error: {}", e)); select_player_usage(ctx.print); return; }
                    };
                    let note = args.get("note").unwrap_or("").to_string();
                    let note = if note.is_empty() { peer.nickname.clone() } else { note };
                    let (pid, nick, udid, ip) = (peer.id, peer.nickname.clone(), peer.udid.clone(), peer.ip.clone());
                    drop(peers);

                    crate::moderation::ban_add(&nick, &udid, &ip, &note);
                    send_to_current(ctx, &format!(".ban_now {}", pid));
                    (ctx.print)(&format!("banned '{}' (pid={}, ip={})", crate::colors::colorize(&nick), pid, ip));
                }

                "op" => {
                    let level: u8 = if let Some(v) = args.get("op-level") {
                        match v.parse::<u8>() {
                            Ok(n) => n,
                            Err(_) => { (ctx.print)(&format!("error: invalid op-level '{}'", v)); return; }
                        }
                    } else {
                        crate::config::cfg().states.lobby_misc.moderation.op_default_level
                    };

                    let endpoint_only = args.has("endpoint")
                        && !args.has("pid") && !args.has("username") && !args.has("udid");

                    let shared = match ctx.shared.get(*ctx.current_lobby) {
                        Some(s) => s, None => return,
                    };
                    let peers = match shared.peers.read() {
                        Ok(g) => g, Err(_) => return,
                    };

                    if endpoint_only {
                        let ip = args.get("endpoint").unwrap_or("").to_string();
                        if ip.is_empty() { (ctx.print)("error: --endpoint requires a value"); return; }
                        let note = args.get("note").unwrap_or("").to_string();
                        let note = if note.is_empty() { ip.clone() } else { note };
                        let pid_opt = find_by_ip(&ip, &peers).map(|p| p.id);
                        drop(peers);

                        crate::moderation::op_set(&ip, level, &note);
                        if let Some(pid) = pid_opt {
                            send_to_current(ctx, &format!(".set_op_pid {} {}", pid, level));
                        }
                        let verb = if level == 0 { "revoked admin for".to_string() } else { format!("set op-level {} for", level) };
                        (ctx.print)(&format!("{} IP {} (note: {})", verb, ip, note));
                        return;
                    }

                    let peer = match resolve_target(&args, &peers) {
                        Ok(p) => p,
                        Err(e) => { (ctx.print)(&format!("error: {}", e)); select_player_usage(ctx.print); return; }
                    };
                    let note = args.get("note").unwrap_or("").to_string();
                    let note = if note.is_empty() { peer.nickname.clone() } else { note };
                    let (pid, nick, udid, ip) =
                        (peer.id, peer.nickname.clone(), peer.udid.clone(), peer.ip.clone());
                    drop(peers);

                    crate::moderation::op_set_full(&nick, &udid, &ip, level, &note);
                    send_to_current(ctx, &format!(".set_op_pid {} {}", pid, level));
                    let verb = if level == 0 { "revoked admin for".to_string() } else { format!("set op-level {} for", level) };
                    (ctx.print)(&format!("{} '{}' (pid={}, ip={}, note: {})",
                        verb, crate::colors::colorize(&nick), pid, ip, note));
                }

                "pardon" => {
                    let endpoint_only = args.has("endpoint")
                        && !args.has("pid") && !args.has("username") && !args.has("udid");

                    if endpoint_only {
                        let ip = args.get("endpoint").unwrap_or("").to_string();
                        if ip.is_empty() { (ctx.print)("error: --endpoint requires a value"); return; }
                        crate::moderation::ban_revoke_by_ip(&ip);
                        crate::moderation::timeout_revoke("", &ip);
                        (ctx.print)(&format!("pardoned IP {} (removed bans and timeouts)", ip));
                        return;
                    }

                    let shared = match ctx.shared.get(*ctx.current_lobby) {
                        Some(s) => s, None => return,
                    };
                    let peers = match shared.peers.read() {
                        Ok(g) => g, Err(_) => return,
                    };
                    let peer = match resolve_target(&args, &peers) {
                        Ok(p) => p,
                        Err(e) => { (ctx.print)(&format!("error: {}", e)); select_player_usage(ctx.print); return; }
                    };
                    let (nick, udid, ip) = (peer.nickname.clone(), peer.udid.clone(), peer.ip.clone());
                    drop(peers);

                    crate::moderation::ban_revoke_full(&nick, &udid, &ip);
                    crate::moderation::timeout_revoke(&udid, &ip);
                    (ctx.print)(&format!("pardoned '{}' (ip={}, removed bans and timeouts)", crate::colors::colorize(&nick), ip));
                }

                "ban-revoke" => {
                    let endpoint_only = args.has("endpoint")
                        && !args.has("pid") && !args.has("username") && !args.has("udid");

                    if endpoint_only {
                        let ip = args.get("endpoint").unwrap_or("").to_string();
                        if ip.is_empty() { (ctx.print)("error: --endpoint requires a value"); return; }
                        crate::moderation::ban_revoke_by_ip(&ip);
                        crate::moderation::timeout_revoke("", &ip);
                        (ctx.print)(&format!("pardoned IP {} (removed bans and timeouts)", ip));
                        return;
                    }

                    let shared = match ctx.shared.get(*ctx.current_lobby) {
                        Some(s) => s, None => return,
                    };
                    let peers = match shared.peers.read() {
                        Ok(g) => g, Err(_) => return,
                    };
                    let peer = match resolve_target(&args, &peers) {
                        Ok(p) => p,
                        Err(e) => { (ctx.print)(&format!("error: {}", e)); select_player_usage(ctx.print); return; }
                    };
                    let (nick, udid, ip) = (peer.nickname.clone(), peer.udid.clone(), peer.ip.clone());
                    drop(peers);

                    crate::moderation::ban_revoke_full(&nick, &udid, &ip);
                    crate::moderation::timeout_revoke(&udid, &ip);
                    (ctx.print)(&format!("pardoned '{}' (ip={}, removed bans and timeouts)", crate::colors::colorize(&nick), ip));
                }

                "deop" => {
                    let endpoint_only = args.has("endpoint")
                        && !args.has("pid") && !args.has("username") && !args.has("udid");

                    let shared = match ctx.shared.get(*ctx.current_lobby) {
                        Some(s) => s, None => return,
                    };
                    let peers = match shared.peers.read() {
                        Ok(g) => g, Err(_) => return,
                    };

                    if endpoint_only {
                        let ip = args.get("endpoint").unwrap_or("").to_string();
                        if ip.is_empty() { (ctx.print)("error: --endpoint requires a value"); return; }
                        let pid_opt = find_by_ip(&ip, &peers).map(|p| p.id);
                        drop(peers);
                        crate::moderation::op_set(&ip, 0, "");
                        if let Some(pid) = pid_opt {
                            send_to_current(ctx, &format!(".set_op_pid {} 0", pid));
                        }
                        (ctx.print)(&format!("revoked admin for IP {}", ip));
                        return;
                    }

                    let peer = match resolve_target(&args, &peers) {
                        Ok(p) => p,
                        Err(e) => { (ctx.print)(&format!("error: {}", e)); select_player_usage(ctx.print); return; }
                    };
                    let (pid, nick, udid, ip) =
                        (peer.id, peer.nickname.clone(), peer.udid.clone(), peer.ip.clone());
                    drop(peers);

                    crate::moderation::op_set_full(&nick, &udid, &ip, 0, "");
                    send_to_current(ctx, &format!(".set_op_pid {} 0", pid));
                    (ctx.print)(&format!("revoked admin for '{}' (pid={}, ip={})", crate::colors::colorize(&nick), pid, ip));
                }

                "whitelist-append" => {
                    let endpoint_only = args.has("endpoint")
                        && !args.has("pid") && !args.has("username") && !args.has("udid");

                    if endpoint_only {
                        let ip = args.get("endpoint").unwrap_or("").to_string();
                        if ip.is_empty() { (ctx.print)("error: --endpoint requires a value"); return; }
                        let note = args.get("note").unwrap_or("").to_string();
                        let note = if note.is_empty() { ip.clone() } else { note };
                        crate::moderation::whitelist_add_full("", "", &ip, &note);
                        (ctx.print)(&format!("whitelisted IP {} (note: {})", ip, note));
                        return;
                    }

                    let shared = match ctx.shared.get(*ctx.current_lobby) {
                        Some(s) => s, None => return,
                    };
                    let peers = match shared.peers.read() {
                        Ok(g) => g, Err(_) => return,
                    };
                    let peer = match resolve_target(&args, &peers) {
                        Ok(p) => p,
                        Err(e) => { (ctx.print)(&format!("error: {}", e)); select_player_usage(ctx.print); return; }
                    };
                    let note = args.get("note").unwrap_or("").to_string();
                    let note = if note.is_empty() { peer.nickname.clone() } else { note };
                    let (nick, udid, ip) = (peer.nickname.clone(), peer.udid.clone(), peer.ip.clone());
                    drop(peers);

                    crate::moderation::whitelist_add_full(&nick, &udid, &ip, &note);
                    (ctx.print)(&format!("whitelisted '{}' (ip={}, note: {})", crate::colors::colorize(&nick), ip, note));
                }

                "whitelist-revoke" => {
                    let endpoint_only = args.has("endpoint")
                        && !args.has("pid") && !args.has("username") && !args.has("udid");

                    if endpoint_only {
                        let ip = args.get("endpoint").unwrap_or("").to_string();
                        if ip.is_empty() { (ctx.print)("error: --endpoint requires a value"); return; }
                        crate::moderation::whitelist_revoke(&ip);
                        (ctx.print)(&format!("removed IP {} from whitelist", ip));
                        return;
                    }

                    let shared = match ctx.shared.get(*ctx.current_lobby) {
                        Some(s) => s, None => return,
                    };
                    let peers = match shared.peers.read() {
                        Ok(g) => g, Err(_) => return,
                    };
                    let peer = match resolve_target(&args, &peers) {
                        Ok(p) => p,
                        Err(e) => { (ctx.print)(&format!("error: {}", e)); select_player_usage(ctx.print); return; }
                    };
                    let (nick, udid, ip) = (peer.nickname.clone(), peer.udid.clone(), peer.ip.clone());
                    drop(peers);

                    crate::moderation::whitelist_revoke_full(&nick, &udid, &ip);
                    (ctx.print)(&format!("removed '{}' (ip={}) from whitelist", crate::colors::colorize(&nick), ip));
                }

                other => {
                    (ctx.print)(&format!(
                        "error: unknown action '{}' (valid: ban, ban-revoke, kick, op, deop, pardon, whitelist-append, whitelist-revoke)",
                        other
                    ));
                    select_player_usage(ctx.print);
                }
            }
        }

        "info" => {
            (ctx.print)(&format!("AdvancedServer {}", crate::server::SERVER_VERSION));
            (ctx.print)("Created by: The Arctic Fox");
            (ctx.print)("Contains code referenced from: BetterServer (MIT)");
            (ctx.print)("");
            (ctx.print)("Rust rewrite additionally references, for comparison/porting purposes:");
            (ctx.print)("  - AdvancedServer's own C predecessor (AGPL-3.0)");
            (ctx.print)("  - disasterserver by vinny (used with permission) -- source of the");
            (ctx.print)("    per-tick event/packet flood cap in this build");
        }

        "license" => {
            for line in crate::license::license_text().lines() {
                (ctx.print)(line);
            }
        }

        "halt" => {
            (ctx.print)("halting all server instances...");
            for tx in ctx.senders.iter() {
                let _ = tx.send(ConsoleCmd::ExecAsServer(":stop".to_string()));
            }
            *ctx.should_exit = true;
        }

        "start" => {
            let args = ParsedArgs::parse(&cmd.args, &[("m", "map")]);
            match args.get("map") {
                Some(m) => send_to_current(ctx, &format!(":start {}", m)),
                None => send_to_current(ctx, ":start"),
            }
        }
        "stop"   => send_to_current(ctx, ":stop"),

        "say" => {
            let msg = cmd.rest();
            if msg.is_empty() {
                (ctx.print)("usage: ?say <message>");
                return;
            }
            if let Some(tx) = ctx.senders.get(*ctx.current_lobby) {
                let _ = tx.send(ConsoleCmd::Chat(msg.clone()));
            }
            (ctx.print)(&format!(
                "[say -> lobby {}] {}",
                *ctx.current_lobby + 1,
                crate::colors::colorize(&msg)
            ));
        }

        "broadcast" => {
            let msg = cmd.rest();
            if msg.is_empty() {
                (ctx.print)("usage: ?broadcast <message>");
                return;
            }
            for tx in ctx.senders.iter() {
                let _ = tx.send(ConsoleCmd::Chat(msg.clone()));
            }
            (ctx.print)(&format!(
                "[broadcast -> all] {}",
                crate::colors::colorize(&msg)
            ));
        }

        _ => {
            (ctx.print)(&format!("unknown terminal command '?{}'. type ?help for the list.", cmd.name));
        }
    }
}

fn send_to_current(ctx: &mut TerminalCtx, cmd: &str) {
    if let Some(tx) = ctx.senders.get(*ctx.current_lobby) {
        let _ = tx.send(ConsoleCmd::ExecAsServer(cmd.to_string()));
    }
}
