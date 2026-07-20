pub mod cmd;
pub mod commands;
pub mod console;

pub use console::{Console, init_logger};

pub enum ConsoleCmd {
    Chat(String),
    ExecAsServer(String),
}
