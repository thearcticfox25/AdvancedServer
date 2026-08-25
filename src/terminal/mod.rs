pub mod cmd;
pub mod commands;
pub mod console;

pub use console::{Console, init_logger, init_file_logging, print_always, level_filter_for};

pub enum ConsoleCmd {
    Chat(String),
    ExecAsServer(String),
}
