use std::fs::File;
use std::io::Write;
use std::sync::{mpsc::SyncSender, Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal,
};
use chrono::{Datelike, Timelike};
use log::{LevelFilter, Log, Metadata, Record};

use crate::server::ServerShared;
use super::ConsoleCmd;
use super::commands::{exec_terminal_cmd, parse_terminal_cmd, TerminalCtx};

static INPUT_LINE: Mutex<String> = Mutex::new(String::new());
static CURSOR_POS: AtomicUsize = AtomicUsize::new(0);
static CURRENT_LOBBY_IDX: AtomicUsize = AtomicUsize::new(0);
static LOG_FILE: Mutex<Option<File>> = Mutex::new(None);

fn prompt_str() -> String {
    let n = CURRENT_LOBBY_IDX.load(Ordering::Relaxed) + 1;
    format!("[Lobby {}] $> ", n)
}

fn print_log_line(s: &str) {
    let input = INPUT_LINE
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default();
    let cursor = CURSOR_POS.load(Ordering::Relaxed);
    let prompt = prompt_str();
    let mut out = std::io::stdout().lock();
    let chars_after = input.chars().count().saturating_sub(cursor);
    if chars_after > 0 {
        let _ = write!(out, "\r\x1B[K{}\r\n{}{}\x1B[{}D", s, prompt, input, chars_after);
    } else {
        let _ = write!(out, "\r\x1B[K{}\r\n{}{}", s, prompt, input);
    }
    let _ = out.flush();
}

struct ConsoleLogger {
    level: LevelFilter,
}

impl Log for ConsoleLogger {
    fn enabled(&self, meta: &Metadata) -> bool {
        meta.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        emit_line(&record.level().to_string(), record.target(), &record.args().to_string());
    }

    fn flush(&self) {}
}

/// Shared by ConsoleLogger::log and print_always: formats a line the same way
/// regardless of whether it went through the `log` crate's level filter, then
/// writes it to the console and (if open) the log file.
fn emit_line(level: &str, target: &str, msg: &str) {
    let now = chrono::Local::now();
    let off = now.offset().local_minus_utc() / 3600;
    let tz = if off >= 0 { format!("UTC+{}", off) } else { format!("UTC{}", off) };
    let ts = format!(
        "{}Y{:02}M{:02}D {:02}:{:02}:{:02} ({})",
        now.year(), now.month(), now.day(),
        now.hour(), now.minute(), now.second(),
        tz,
    );
    let line = format!("[{} | {} | {}] {}", level, ts, target, msg);
    print_log_line(&line);

    if let Ok(mut guard) = LOG_FILE.lock() {
        if let Some(file) = guard.as_mut() {
            let file_ts = now.format("%m/%d/%Y %H:%M:%S");
            let _ = writeln!(file, "[{} {} {}] {}", file_ts, level, target, msg);
            let _ = file.flush();
        }
    }
}

/// Prints unconditionally, ignoring the configured log_level -- for the
/// startup banner and basic init report, which must be visible even at
/// log_level 0 (see the level table on server_config.logging.log_level).
pub fn print_always(target: &str, msg: &str) {
    emit_line("ALWAYS", target, msg);
}

pub fn init_logger() {
    let level = std::env::var("RUST_LOG")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(LevelFilter::Info);
    log::set_boxed_logger(Box::new(ConsoleLogger { level })).ok();
    log::set_max_level(level);
}

/// Maps the config's 0-4 log_level to a LevelFilter (see the doc comment on
/// Logging::log_level in config.rs for what each level includes).
pub fn level_filter_for(log_level: u8) -> LevelFilter {
    match log_level {
        0 => LevelFilter::Off,
        1 => LevelFilter::Error,
        2 => LevelFilter::Warn,
        3 => LevelFilter::Info,
        _ => LevelFilter::Debug,
    }
}

/// Opens the log file if `server_config.logging.log_to_file` is enabled.
/// Must run after the config is loaded, so it's a separate call from
/// init_logger(), which runs before config load so early startup messages
/// aren't lost.
pub fn init_file_logging() {
    if !crate::config::cfg().server_config.logging.log_to_file {
        return;
    }
    let _ = std::fs::create_dir_all("logs");
    let filename = chrono::Local::now().format("logs/%m%d%Y %H%M%S.log").to_string();
    match File::create(&filename) {
        Ok(file) => {
            if let Ok(mut guard) = LOG_FILE.lock() {
                *guard = Some(file);
            }
            log::info!("Logging to file: {}", filename);
        }
        Err(e) => {
            log::error!("failed to open log file {}: {}", filename, e);
        }
    }
}

pub struct Console {
    senders: Vec<SyncSender<ConsoleCmd>>,
    shared: Vec<Arc<ServerShared>>,
    current_lobby: usize,
    input: String,
    cursor: usize,
    should_exit: bool,
}

impl Console {
    pub fn new(senders: Vec<SyncSender<ConsoleCmd>>, shared: Vec<Arc<ServerShared>>) -> Self {
        Self {
            senders,
            shared,
            current_lobby: 0,
            input: String::new(),
            cursor: 0,
            should_exit: false,
        }
    }

    pub fn run(&mut self, running: Arc<AtomicBool>) {
        terminal::enable_raw_mode().ok();

        loop {
            if self.should_exit || !running.load(Ordering::Relaxed) {
                break;
            }
            if !event::poll(std::time::Duration::from_millis(50)).unwrap_or(false) {
                continue;
            }
            match event::read() {
                Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        break;
                    }
                    KeyCode::Enter => {
                        let line = self.input.trim().to_string();
                        self.input.clear();
                        self.cursor = 0;
                        self.sync_input_global();
                        self.redraw_input();
                        if !line.is_empty() {
                            self.process(&line);
                        }
                    }
                    KeyCode::Backspace => {
                        if self.cursor > 0 {
                            self.cursor -= 1;
                            let byte_idx = char_to_byte(&self.input, self.cursor);
                            self.input.remove(byte_idx);
                            self.sync_input_global();
                            self.redraw_input();
                        }
                    }
                    KeyCode::Delete => {
                        let char_count = self.input.chars().count();
                        if self.cursor < char_count {
                            let byte_idx = char_to_byte(&self.input, self.cursor);
                            self.input.remove(byte_idx);
                            self.sync_input_global();
                            self.redraw_input();
                        }
                    }
                    KeyCode::Left => {
                        if self.cursor > 0 {
                            self.cursor -= 1;
                            self.sync_input_global();
                            self.redraw_input();
                        }
                    }
                    KeyCode::Right => {
                        if self.cursor < self.input.chars().count() {
                            self.cursor += 1;
                            self.sync_input_global();
                            self.redraw_input();
                        }
                    }
                    KeyCode::Home => {
                        self.cursor = 0;
                        self.sync_input_global();
                        self.redraw_input();
                    }
                    KeyCode::End => {
                        self.cursor = self.input.chars().count();
                        self.sync_input_global();
                        self.redraw_input();
                    }
                    KeyCode::Char(c) => {
                        let byte_idx = char_to_byte(&self.input, self.cursor);
                        self.input.insert(byte_idx, c);
                        self.cursor += 1;
                        self.sync_input_global();
                        self.redraw_input();
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        terminal::disable_raw_mode().ok();
    }

    fn sync_input_global(&self) {
        if let Ok(mut guard) = INPUT_LINE.lock() {
            *guard = self.input.clone();
        }
        CURSOR_POS.store(self.cursor, Ordering::Relaxed);
    }

    fn redraw_input(&self) {
        let prompt = prompt_str();
        let mut out = std::io::stdout().lock();
        let chars_after = self.input.chars().count().saturating_sub(self.cursor);
        if chars_after > 0 {
            let _ = write!(out, "\r\x1B[K{}{}\x1B[{}D", prompt, self.input, chars_after);
        } else {
            let _ = write!(out, "\r\x1B[K{}{}", prompt, self.input);
        }
        let _ = out.flush();
    }

    fn terminal_print(&self, s: &str) {
        let prompt = prompt_str();
        let mut out = std::io::stdout().lock();
        let chars_after = self.input.chars().count().saturating_sub(self.cursor);
        if chars_after > 0 {
            let _ = write!(out, "\r\x1B[K{}\r\n{}{}\x1B[{}D", s, prompt, self.input, chars_after);
        } else {
            let _ = write!(out, "\r\x1B[K{}\r\n{}{}", s, prompt, self.input);
        }
        let _ = out.flush();
    }

    fn process(&mut self, line: &str) {

        if let Some(cmd) = parse_terminal_cmd(line) {
            let senders = &self.senders;
            let shared = &self.shared;
            let current_lobby = &mut self.current_lobby;
            let should_exit = &mut self.should_exit;

            let input_snap = self.input.clone();
            let mut print_fn = |s: &str| {
                let prompt = prompt_str();
                let mut out = std::io::stdout().lock();
                let _ = write!(out, "\r\x1B[K{}\r\n{}{}", s, prompt, input_snap);
                let _ = out.flush();
            };
            let mut ctx = TerminalCtx {
                senders,
                shared,
                current_lobby,
                should_exit,
                print: &mut print_fn,
            };
            exec_terminal_cmd(&cmd, &mut ctx);
            CURRENT_LOBBY_IDX.store(self.current_lobby, Ordering::Relaxed);
            self.redraw_input();
            return;
        }


        if let Some(tx) = self.senders.get(self.current_lobby) {
            let _ = tx.send(ConsoleCmd::ExecAsServer(line.to_string()));
        }
    }
}

fn char_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}
