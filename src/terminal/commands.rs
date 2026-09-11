use std::collections::HashMap;
use std::sync::{mpsc::SyncSender, Arc};
use crate::server::{PeerSummary, ServerShared};
use super::ConsoleCmd;

pub struct TerminalCmd {
    pub name: String,
    pub args: Vec<String>,
}

impl TerminalCmd {
    pub fn arg(&self, index: usize) -> &str {
        self.args.get(index).map(String::as_str).unwrap_or("")
    }

    pub fn rest(&self) -> String {
        self.args.join(" ")
    }
}

pub fn parse_terminal_cmd(input: &str) -> Option<TerminalCmd> {
    let trimmed = input.trim();
    if !trimmed.starts_with('?') {
        return None;
    }
    let after = trimmed[1..].trim_start();
    if after.is_empty() {
        return None;
    }
    let (name_part, rest) = match after.find(char::is_whitespace) {
        Some(space) => (&after[..space], after[space + 1..].trim()),
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

pub fn exec_terminal_cmd(cmd: &TerminalCmd, ctx: &mut TerminalCtx) {
    match cmd.name.as_str() {
        "help"          => print_help(ctx),
        "info"          => print_info(ctx),
        "license"       => print_license(ctx),
        "lobby"         => switch_lobby(cmd, ctx),
        "playerlist"    => print_playerlist(cmd, ctx),
        "select-player" => select_player(cmd, ctx),
        "halt"          => halt_all(ctx),
        "start"         => force_start(cmd, ctx),
        "stop"          => send_to_current(ctx, ":stop"),
        "say"           => say(cmd, ctx),
        "broadcast"     => broadcast(cmd, ctx),
        other => (ctx.print)(&format!(
            "unknown terminal command '?{}'. type ?help for the list.", other
        )),
    }
}

// ---------------------------------------------------------------- argument parsing

/// The `--long value` / `-s value` options of one command.
///
/// A value may be several words: everything up to the next option belongs to the
/// option before it, so `-N banned for spamming` keeps its whole note.
struct ParsedArgs {
    values: HashMap<String, String>,
}

impl ParsedArgs {
    fn parse(args: &[String], aliases: &[(&str, &str)]) -> Self {
        let alias_map: HashMap<&str, &str> = aliases.iter().copied().collect();
        let mut values: HashMap<String, String> = HashMap::new();
        let mut index = 0;

        while index < args.len() {
            let arg = &args[index];
            let key = if let Some(long) = arg.strip_prefix("--") {
                long.to_string()
            } else if let Some(short) = arg.strip_prefix('-').filter(|flag| !flag.is_empty()) {
                alias_map.get(short).map(|long| long.to_string()).unwrap_or_else(|| short.to_string())
            } else {
                index += 1;
                continue;
            };

            index += 1;
            let mut parts = Vec::new();
            while index < args.len() && !args[index].starts_with('-') {
                parts.push(args[index].clone());
                index += 1;
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

// ---------------------------------------------------------------- output helpers

/// Prints `rows` under `headers`, every column padded to its widest cell.
fn print_table(print: &mut dyn FnMut(&str), headers: &[&str], rows: &[Vec<String>]) {
    let mut widths: Vec<usize> = headers.iter().map(|header| header.len()).collect();
    for row in rows {
        for (column, cell) in row.iter().enumerate() {
            if let Some(width) = widths.get_mut(column) {
                *width = (*width).max(cell.len());
            }
        }
    }

    let fmt_row = |cells: &[&str]| -> String {
        cells.iter().enumerate()
            .map(|(column, cell)| format!("{:<width$}", cell, width = widths.get(column).copied().unwrap_or(0)))
            .collect::<Vec<_>>()
            .join(" | ")
    };

    (print)(&fmt_row(headers));
    (print)(&widths.iter().map(|&width| "-".repeat(width)).collect::<Vec<_>>().join("-+-"));

    if rows.is_empty() {
        (print)("  (empty)");
        return;
    }
    for row in rows {
        let cells: Vec<&str> = row.iter().map(String::as_str).collect();
        (print)(&fmt_row(&cells));
    }
}

fn send_to_current(ctx: &mut TerminalCtx, cmd: &str) {
    if let Some(tx) = ctx.senders.get(*ctx.current_lobby) {
        let _ = tx.send(ConsoleCmd::ExecAsServer(cmd.to_string()));
    }
}

// ---------------------------------------------------------------- simple commands

fn print_help(ctx: &mut TerminalCtx) {
    for line in [
        "--- terminal commands (prefix: ?) ---",
        "  ?help                      show this list",
        "  ?info                      show version and credits",
        "  ?lobby <n>                 switch active lobby (1-indexed)",
        "  ?playerlist                list players (table view)",
        "  ?playerlist --banned/-b    list banned IPs / UDIDs / nicknames",
        "  ?playerlist --timeouts/-t  list active timeouts",
        "  ?playerlist --admins/-a    list administrators",
        "  ?say <msg>                 send a server message to active lobby",
        "  ?broadcast <msg>           send a server message to all lobbies",
        "  ?select-player <selector> --action/-a <action>  [--note/-N <text>]  [--op-level/-l <0-255>]",
        "    selectors:  --pid/-p <PID> | --endpoint/-e <ip> | --username/-n <name> | --udid/-u <UDID>",
        "    actions:    ban | ban-revoke | kick | op | deop | pardon | whitelist-append | whitelist-revoke",
        "  ?license                   print embedded license text",
        "  ?halt                      kills the entire server",
        "  ?start                     force-start a game in active lobby",
        "  ?start --map/-m <id>       force-start, skipping MapVote straight to CharSelect with map <id>",
        "  ?stop                      force-end the current round",
        "---",
    ] {
        (ctx.print)(line);
    }
}

fn print_info(ctx: &mut TerminalCtx) {
    (ctx.print)(&format!("AdvancedServer {}", crate::server::SERVER_VERSION));
    for line in [
        "Created by: The Arctic Fox",
        "Contains code referenced from: BetterServer (MIT)",
        "",
        "Rust rewrite additionally references, for comparison/porting purposes:",
        "  - AdvancedServer's own C predecessor (AGPL-3.0)",
        "  - disasterserver by vinny (used with permission) -- source of the",
        "    per-tick event/packet flood cap in this build",
    ] {
        (ctx.print)(line);
    }
}

fn print_license(ctx: &mut TerminalCtx) {
    for line in crate::license::license_text().lines() {
        (ctx.print)(line);
    }
}

fn switch_lobby(cmd: &TerminalCmd, ctx: &mut TerminalCtx) {
    // Lobbies are 1-indexed for the admin, 0-indexed internally.
    let index = match cmd.arg(0).parse::<usize>() {
        Ok(number) if number >= 1 => number - 1,
        _ => {
            (ctx.print)("usage: ?lobby <n>  (lobbies are 1-indexed)");
            return;
        }
    };
    if index >= ctx.senders.len() {
        (ctx.print)(&format!(
            "lobby {} does not exist (valid: 1..{})", index + 1, ctx.senders.len()
        ));
        return;
    }
    *ctx.current_lobby = index;
    (ctx.print)(&format!("active lobby: {}", index + 1));
}

fn halt_all(ctx: &mut TerminalCtx) {
    (ctx.print)("halting all server instances...");
    for tx in ctx.senders.iter() {
        let _ = tx.send(ConsoleCmd::ExecAsServer(":stop".to_string()));
    }
    *ctx.should_exit = true;
}

fn force_start(cmd: &TerminalCmd, ctx: &mut TerminalCtx) {
    let args = ParsedArgs::parse(&cmd.args, &[("m", "map")]);
    match args.get("map") {
        Some(map) => send_to_current(ctx, &format!(":start {}", map)),
        None      => send_to_current(ctx, ":start"),
    }
}

fn say(cmd: &TerminalCmd, ctx: &mut TerminalCtx) {
    let msg = cmd.rest();
    if msg.is_empty() {
        (ctx.print)("usage: ?say <message>");
        return;
    }
    if let Some(tx) = ctx.senders.get(*ctx.current_lobby) {
        let _ = tx.send(ConsoleCmd::Chat(msg.clone()));
    }
    (ctx.print)(&format!(
        "[say -> lobby {}] {}", *ctx.current_lobby + 1, crate::colors::colorize(&msg)
    ));
}

fn broadcast(cmd: &TerminalCmd, ctx: &mut TerminalCtx) {
    let msg = cmd.rest();
    if msg.is_empty() {
        (ctx.print)("usage: ?broadcast <message>");
        return;
    }
    for tx in ctx.senders.iter() {
        let _ = tx.send(ConsoleCmd::Chat(msg.clone()));
    }
    (ctx.print)(&format!("[broadcast -> all] {}", crate::colors::colorize(&msg)));
}

// ---------------------------------------------------------------- ?playerlist

fn print_playerlist(cmd: &TerminalCmd, ctx: &mut TerminalCtx) {
    let args = ParsedArgs::parse(
        &cmd.args,
        &[("b", "banned"), ("t", "timeouts"), ("a", "admins")],
    );

    if args.has("banned")   { return print_blacklist(ctx); }
    if args.has("timeouts") { return print_timeouts(ctx); }
    if args.has("admins")   { return print_admins(ctx); }

    let peers = peers_of_current_lobby(ctx);
    (ctx.print)(&format!(
        "--- lobby {} | {} player(s) ---", *ctx.current_lobby + 1, peers.len()
    ));

    let rows: Vec<Vec<String>> = peers.iter().map(|peer| vec![
        peer.id.to_string(),
        peer.nickname.clone(),
        peer.ip.clone(),
        peer.udid.clone(),
        peer.op.to_string(),
        yes_no(peer.mod_tool),
        yes_no(peer.is_mobile),
        if peer.in_game { "in game" } else { "waiting" }.to_string(),
    ]).collect();

    print_table(
        ctx.print,
        &["PID", "Username", "IP Endpoint", "UDID", "OP Level", "Modified", "Mobile", "Status"],
        &rows,
    );
}

fn yes_no(flag: bool) -> String {
    if flag { "yes" } else { "no" }.to_string()
}

fn print_blacklist(ctx: &mut TerminalCtx) {
    let entries = crate::moderation::list_blacklist();
    (ctx.print)(&format!("--- blacklist ({}) ---", entries.len()));
    let rows: Vec<Vec<String>> = entries.iter()
        .map(|entry| vec![entry.ip.clone(), entry.udid.clone(), entry.nickname.clone(), entry.note.clone()])
        .collect();
    print_table(ctx.print, &["IP", "UDID", "Nickname", "Note"], &rows);
}

fn print_timeouts(ctx: &mut TerminalCtx) {
    let now = crate::moderation::now_unix() as i64;
    let entries = crate::moderation::list_timeouts();
    (ctx.print)(&format!("--- active timeouts ({}) ---", entries.len()));

    let rows: Vec<Vec<String>> = entries.iter().map(|entry| {
        let secs_left = entry.expires_at.saturating_sub(now).max(0);
        vec![
            entry.ip.clone(), entry.udid.clone(), entry.nickname.clone(),
            format!("{}s remaining", secs_left), entry.note.clone(),
        ]
    }).collect();

    print_table(ctx.print, &["IP", "UDID", "Nickname", "Expires", "Note"], &rows);
}

fn print_admins(ctx: &mut TerminalCtx) {
    let entries = crate::moderation::list_ops();
    (ctx.print)(&format!("--- administrators ({}) ---", entries.len()));
    let rows: Vec<Vec<String>> = entries.iter().map(|entry| vec![
        entry.ip.clone(), entry.udid.clone(), entry.nickname.clone(),
        entry.level.to_string(), entry.note.clone(),
    ]).collect();
    print_table(ctx.print, &["IP", "UDID", "Nickname", "Level", "Note"], &rows);
}

// ---------------------------------------------------------------- ?select-player

const SELECT_PLAYER_ALIASES: &[(&str, &str)] = &[
    ("p", "pid"),
    ("e", "endpoint"),
    ("n", "username"),
    ("u", "udid"),
    ("a", "action"),
    ("l", "op-level"),
    ("N", "note"),
];

const SELECTORS: [&str; 4] = ["pid", "endpoint", "username", "udid"];

/// The player a ?select-player action applies to.
///
/// Copied out of the peer list, so the read lock is released again before any
/// moderation call runs.
struct Target {
    /// Present only while the player is connected to the active lobby.
    pid: Option<u16>,
    nickname: String,
    udid: String,
    ip: String,
    /// True when --endpoint picked this target. Moderation then keys on the IP
    /// alone, which is also the only selector that works for an offline player.
    by_endpoint: bool,
}

impl Target {
    /// The note to record when the admin did not write one: whoever this is,
    /// named the clearest way we can.
    fn note_or_default(&self, note: &str) -> String {
        if !note.is_empty() {
            note.to_string()
        } else if self.nickname.is_empty() {
            self.ip.clone()
        } else {
            self.nickname.clone()
        }
    }

    /// How to name this target in a reply to the admin.
    fn describe(&self) -> String {
        let who = if self.nickname.is_empty() {
            format!("IP {}", self.ip)
        } else {
            format!("'{}' (ip={})", crate::colors::colorize(&self.nickname), self.ip)
        };
        match self.pid {
            Some(pid) => format!("{} (pid={})", who, pid),
            None      => who,
        }
    }
}

fn select_player(cmd: &TerminalCmd, ctx: &mut TerminalCtx) {
    if cmd.args.is_empty() {
        return print_select_player_usage(ctx);
    }

    let args = ParsedArgs::parse(&cmd.args, SELECT_PLAYER_ALIASES);
    let action = match args.get("action") {
        Some(name) => name.to_string(),
        None => {
            (ctx.print)("error: --action/-a is required");
            return print_select_player_usage(ctx);
        }
    };

    // Banning a username can mean "everyone currently using it", which is the one
    // action that is not about a single target.
    let aggressive_name_ban = crate::config::cfg()
        .states.lobby_misc.moderation.aggressive_username_ban;
    if action == "ban" && args.has("username") && aggressive_name_ban {
        return ban_everyone_named(&args, ctx);
    }

    let target = match resolve_target(&args, ctx) {
        Ok(found) => found,
        Err(reason) => {
            (ctx.print)(&format!("error: {}", reason));
            return print_select_player_usage(ctx);
        }
    };
    let note = args.get("note").unwrap_or("");

    match action.as_str() {
        "kick"                    => kick(&target, note, ctx),
        "ban"                     => ban(&target, note, ctx),
        "pardon" | "ban-revoke"   => pardon(&target, ctx),
        "op"                      => set_op(&target, note, &args, ctx),
        "deop"                    => set_op_level(&target, 0, "", ctx),
        "whitelist-append"        => whitelist_add(&target, note, ctx),
        "whitelist-revoke"        => whitelist_remove(&target, ctx),
        other => {
            (ctx.print)(&format!(
                "error: unknown action '{}' (valid: ban, ban-revoke, kick, op, deop, \
                 pardon, whitelist-append, whitelist-revoke)",
                other
            ));
            print_select_player_usage(ctx);
        }
    }
}

/// Resolves the selector into a single target.
///
/// `--endpoint` is deliberately the odd one out: it names an IP rather than a
/// connection, so it also resolves for a player who is not online, with only the
/// IP filled in.
fn resolve_target(args: &ParsedArgs, ctx: &TerminalCtx) -> Result<Target, String> {
    let given: Vec<&str> = SELECTORS.iter().copied().filter(|selector| args.has(selector)).collect();
    match given.len() {
        0 => return Err("no target specified".to_string()),
        1 => {}
        _ => return Err("only one target argument is allowed".to_string()),
    }

    let peers = peers_of_current_lobby(ctx);

    if let Some(ip) = args.get("endpoint") {
        if ip.is_empty() {
            return Err("--endpoint requires a value".to_string());
        }
        let online = peers.iter().find(|peer| peer.ip == ip);
        return Ok(Target {
            pid:         online.map(|peer| peer.id),
            nickname:    online.map(|peer| peer.nickname.clone()).unwrap_or_default(),
            udid:        online.map(|peer| peer.udid.clone()).unwrap_or_default(),
            ip:          ip.to_string(),
            by_endpoint: true,
        });
    }

    let peer = if let Some(raw) = args.get("pid") {
        let pid: u16 = raw.parse().map_err(|_| format!("invalid PID: '{}'", raw))?;
        peers.iter().find(|peer| peer.id == pid)
            .ok_or_else(|| format!("no player with PID {}", pid))?
    } else if let Some(name) = args.get("username") {
        let wanted = crate::colors::strip(name).to_lowercase();
        find_unique(&peers, &format!("username '{}'", name),
            |peer| crate::colors::strip(&peer.nickname).to_lowercase() == wanted)?
    } else if let Some(udid) = args.get("udid") {
        find_unique(&peers, &format!("UDID '{}'", udid), |peer| peer.udid == udid)?
    } else {
        return Err("no target specified".to_string());
    };

    Ok(Target {
        pid:         Some(peer.id),
        nickname:    peer.nickname.clone(),
        udid:        peer.udid.clone(),
        ip:          peer.ip.clone(),
        by_endpoint: false,
    })
}

/// Finds the one peer matching `matches`, or explains why it could not.
fn find_unique<'a>(
    peers: &'a [PeerSummary],
    what: &str,
    matches: impl Fn(&PeerSummary) -> bool,
) -> Result<&'a PeerSummary, String> {
    let mut found = peers.iter().filter(|peer| matches(peer));
    let first = found.next().ok_or_else(|| format!("no player with {}", what))?;
    match found.next() {
        None    => Ok(first),
        Some(_) => Err(format!("ambiguous: more than one player shares that {}", what)),
    }
}

/// A snapshot of the active lobby's players. An unreachable lobby reads as empty,
/// which is what every caller would do with it anyway.
fn peers_of_current_lobby(ctx: &TerminalCtx) -> Vec<PeerSummary> {
    ctx.shared
        .get(*ctx.current_lobby)
        .and_then(|shared| shared.peers.read().ok())
        .map(|guard| guard.iter().map(clone_summary).collect())
        .unwrap_or_default()
}

fn clone_summary(peer: &PeerSummary) -> PeerSummary {
    PeerSummary {
        id: peer.id,
        ip: peer.ip.clone(),
        udid: peer.udid.clone(),
        nickname: peer.nickname.clone(),
        op: peer.op,
        mod_tool: peer.mod_tool,
        is_mobile: peer.is_mobile,
        in_game: peer.in_game,
    }
}

// ---------------------------------------------------------------- the actions

fn kick(target: &Target, note: &str, ctx: &mut TerminalCtx) {
    let pid = match target.pid {
        Some(pid) => pid,
        None => {
            (ctx.print)("error: cannot kick a player who is not online");
            return;
        }
    };
    let window = crate::config::cfg().server_config.pairing.kick_timeout_window as u64;
    crate::moderation::timeout_set(
        &target.nickname, &target.udid, &target.ip,
        crate::moderation::now_unix() + window,
        &target.note_or_default(note),
    );
    send_to_current(ctx, &format!(".kick_now {}", pid));
    (ctx.print)(&format!("kicked {}", target.describe()));
}

fn ban(target: &Target, note: &str, ctx: &mut TerminalCtx) {
    let note = target.note_or_default(note);

    // An offline target selected by IP can only be banned by that IP; there is no
    // nickname or UDID to record for them.
    match target.pid {
        Some(pid) => {
            crate::moderation::ban_add(&target.nickname, &target.udid, &target.ip, &note);
            send_to_current(ctx, &format!(".ban_now {}", pid));
        }
        None => { crate::moderation::ban_ip_only(&target.ip, &note); }
    }
    (ctx.print)(&format!("banned {} (note: {})", target.describe(), note));
}

fn pardon(target: &Target, ctx: &mut TerminalCtx) {
    if target.by_endpoint {
        crate::moderation::ban_revoke_by_ip(&target.ip);
        crate::moderation::timeout_revoke("", &target.ip);
    } else {
        crate::moderation::ban_revoke_full(&target.nickname, &target.udid, &target.ip);
        crate::moderation::timeout_revoke(&target.udid, &target.ip);
    }
    (ctx.print)(&format!(
        "pardoned {} (removed bans and timeouts)", target.describe()
    ));
}

fn set_op(target: &Target, note: &str, args: &ParsedArgs, ctx: &mut TerminalCtx) {
    let level: u8 = match args.get("op-level") {
        Some(raw) => match raw.parse() {
            Ok(level) => level,
            Err(_) => {
                (ctx.print)(&format!("error: invalid op-level '{}'", raw));
                return;
            }
        },
        None => crate::config::cfg().states.lobby_misc.moderation.op_default_level,
    };
    set_op_level(target, level, note, ctx);
}

/// Level 0 means "no longer an operator", which is what ?select-player -a deop does.
fn set_op_level(target: &Target, level: u8, note: &str, ctx: &mut TerminalCtx) {
    let note = target.note_or_default(note);

    if target.by_endpoint {
        crate::moderation::op_set(&target.ip, level, &note);
    } else {
        crate::moderation::op_set_full(
            &target.nickname, &target.udid, &target.ip, level, &note,
        );
    }
    if let Some(pid) = target.pid {
        send_to_current(ctx, &format!(".set_op_pid {} {}", pid, level));
    }

    if level == 0 {
        (ctx.print)(&format!("revoked admin for {}", target.describe()));
    } else {
        (ctx.print)(&format!(
            "set op-level {} for {} (note: {})", level, target.describe(), note
        ));
    }
}

fn whitelist_add(target: &Target, note: &str, ctx: &mut TerminalCtx) {
    let note = target.note_or_default(note);
    crate::moderation::whitelist_add_full(
        &target.nickname, &target.udid, &target.ip, &note,
    );
    (ctx.print)(&format!("whitelisted {} (note: {})", target.describe(), note));
}

fn whitelist_remove(target: &Target, ctx: &mut TerminalCtx) {
    if target.by_endpoint {
        crate::moderation::whitelist_revoke(&target.ip);
    } else {
        crate::moderation::whitelist_revoke_full(
            &target.nickname, &target.udid, &target.ip,
        );
    }
    (ctx.print)(&format!("removed {} from whitelist", target.describe()));
}

/// With aggressive_username_ban on, banning a username bans every player
/// currently connected under it, not just one.
fn ban_everyone_named(args: &ParsedArgs, ctx: &mut TerminalCtx) {
    let name = args.get("username").unwrap_or("");
    let wanted = crate::colors::strip(name).to_lowercase();

    let victims: Vec<PeerSummary> = peers_of_current_lobby(ctx).into_iter()
        .filter(|peer| crate::colors::strip(&peer.nickname).to_lowercase() == wanted)
        .collect();

    if victims.is_empty() {
        (ctx.print)(&format!("error: no online player with username '{}'", name));
        return;
    }

    let note_arg = args.get("note").unwrap_or("").to_string();
    for peer in victims {
        let target = Target {
            pid: Some(peer.id),
            nickname: peer.nickname,
            udid: peer.udid,
            ip: peer.ip,
            by_endpoint: false,
        };
        ban(&target, &note_arg, ctx);
    }
}

fn print_select_player_usage(ctx: &mut TerminalCtx) {
    for line in [
        "usage: ?select-player <selector> --action/-a <action> [options]",
        "  selectors (pick exactly one):",
        "    --pid/-p <PID>          select by peer ID (online only)",
        "    --endpoint/-e <ip>      select by IP address (works offline too)",
        "    --username/-n <name>    select by username (stripped, case-insensitive)",
        "    --udid/-u <UDID>        select by UDID",
        "  actions:",
        "    -a kick               kick (temp timeout)",
        "    -a ban                ban the player",
        "    -a ban-revoke         alias for pardon",
        "    -a pardon             remove all bans and timeouts",
        "    -a op                 grant operator status",
        "    -a deop               revoke operator status  (shorthand for -a op -l 0)",
        "    -a whitelist-append   add player to whitelist",
        "    -a whitelist-revoke   remove player from whitelist",
        "  options:",
        "    --note/-N <text>        note for ban/kick/op/whitelist  (default: username or IP)",
        "    --op-level/-l <0-255>   op level for -a op  (0 = revoke; default = server default)",
    ] {
        (ctx.print)(line);
    }
}
