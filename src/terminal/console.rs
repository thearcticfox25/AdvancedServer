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
    let lobby_number = CURRENT_LOBBY_IDX.load(Ordering::Relaxed) + 1;
    format!("[Lobby {}] $> ", lobby_number)
}

/// Repaints the prompt line, optionally printing `above` on its own line first.
///
/// The prompt is a single line that log output keeps having to scroll past, so
/// every write goes through here: clear the line, print what has to be printed,
/// draw the prompt with whatever the admin has typed so far, and leave the
/// terminal cursor where they left it.
fn redraw(above: Option<&str>, input: &str, cursor: usize) {
    let prompt = prompt_str();
    let chars_after = input.chars().count().saturating_sub(cursor);
    let mut out = std::io::stdout().lock();

    match above {
        Some(line) => { let _ = write!(out, "\r\x1B[K{}\r\n{}{}", line, prompt, input); }
        None       => { let _ = write!(out, "\r\x1B[K{}{}", prompt, input); }
    }
    if chars_after > 0 {
        let _ = write!(out, "\x1B[{}D", chars_after);
    }
    let _ = out.flush();
}

/// Prints one log line above the prompt. Called from whichever thread logged it,
/// so the current input is read back from the shared copy rather than passed in.
fn print_log_line(line: &str) {
    let input = INPUT_LINE.lock().map(|guard| guard.clone()).unwrap_or_default();
    let cursor = CURSOR_POS.load(Ordering::Relaxed);
    redraw(Some(line), &input, cursor);
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
    let utc_offset_hours = now.offset().local_minus_utc() / 3600;
    let timezone = if utc_offset_hours >= 0 { format!("UTC+{}", utc_offset_hours) } else { format!("UTC{}", utc_offset_hours) };
    let timestamp = format!(
        "{}Y{:02}M{:02}D {:02}:{:02}:{:02} ({})",
        now.year(), now.month(), now.day(),
        now.hour(), now.minute(), now.second(),
        timezone,
    );
    let line = format!("[{} | {} | {}] {}", level, timestamp, target, msg);
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
        .and_then(|text| text.parse().ok())
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
        Err(error) => {
            log::error!("failed to open log file {}: {}", filename, error);
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

        while !self.should_exit && running.load(Ordering::Relaxed) {
            if !event::poll(std::time::Duration::from_millis(50)).unwrap_or(false) {
                continue;
            }
            let key = match event::read() {
                Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => key,
                _ => continue,
            };
            if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                break;
            }

            let submitted = self.edit(key.code);

            // Every key ends the same way: publish the new input line for the
            // logger thread, repaint the prompt, and only then run whatever the
            // admin just pressed Enter on -- so its output lands below the prompt.
            self.sync_input_global();
            self.redraw_input();
            if let Some(line) = submitted {
                self.process(&line);
            }
        }

        terminal::disable_raw_mode().ok();
    }

    /// Applies one keypress to the input line. Returns the finished line when the
    /// key was Enter and there is something to run.
    fn edit(&mut self, code: KeyCode) -> Option<String> {
        let char_count = self.input.chars().count();
        match code {
            KeyCode::Enter => {
                let line = self.input.trim().to_string();
                self.input.clear();
                self.cursor = 0;
                return if line.is_empty() { None } else { Some(line) };
            }
            KeyCode::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                let byte_idx = char_to_byte(&self.input, self.cursor);
                self.input.remove(byte_idx);
            }
            KeyCode::Delete if self.cursor < char_count => {
                let byte_idx = char_to_byte(&self.input, self.cursor);
                self.input.remove(byte_idx);
            }
            KeyCode::Left if self.cursor > 0            => self.cursor -= 1,
            KeyCode::Right if self.cursor < char_count  => self.cursor += 1,
            KeyCode::Home                               => self.cursor = 0,
            KeyCode::End                                => self.cursor = char_count,
            KeyCode::Char(typed) => {
                let byte_idx = char_to_byte(&self.input, self.cursor);
                self.input.insert(byte_idx, typed);
                self.cursor += 1;
            }
            _ => {}
        }
        None
    }

    fn sync_input_global(&self) {
        if let Ok(mut guard) = INPUT_LINE.lock() {
            *guard = self.input.clone();
        }
        CURSOR_POS.store(self.cursor, Ordering::Relaxed);
    }

    fn redraw_input(&self) {
        redraw(None, &self.input, self.cursor);
    }

    fn process(&mut self, line: &str) {
        if let Some(cmd) = parse_terminal_cmd(line) {
            let senders = &self.senders;
            let shared = &self.shared;
            let current_lobby = &mut self.current_lobby;
            let should_exit = &mut self.should_exit;

            let input_snap = self.input.clone();
            let cursor_snap = self.cursor;
            let mut print_fn = |line: &str| redraw(Some(line), &input_snap, cursor_snap);
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

fn char_to_byte(text: &str, char_idx: usize) -> usize {
    text.char_indices()
        .nth(char_idx)
        .map(|(byte_idx, _)| byte_idx)
        .unwrap_or(text.len())
}
