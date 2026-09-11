mod anticheat;
mod colors;
mod config;
mod license;
mod connection;
mod terminal;
mod entities;
mod maps;
mod moderation;
mod packet;
mod player;
mod server;
mod states;
mod status;
mod vote;
mod worker;

use std::sync::mpsc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use config::{load_config, CONFIG};
use terminal::{Console, ConsoleCmd};
use moderation::init_moderation;
use server::{ServerShared, SERVER_VERSION};
use status::init_status;
use worker::server_worker;

fn main() {
    terminal::init_logger();
    print_banner();

    let cfg = load_verified_config();
    CONFIG.set(cfg).expect("config already set");
    terminal::init_file_logging();

    // RUST_LOG, when set, wins over the config file: it is the developer's
    // override and init_logger already honoured it.
    if std::env::var("RUST_LOG").is_err() {
        let level = terminal::level_filter_for(config::cfg().server_config.logging.log_level);
        log::set_max_level(level);
    }

    anticheat::physics::load("MapPhysics.json");
    init_moderation();
    init_status();

    let running = install_signal_handlers();
    let (senders, instances) = spawn_instances(running.clone());

    // Let every instance bind and print its "listening" line before the console
    // prompt appears in the middle of them.
    std::thread::sleep(std::time::Duration::from_millis(150));

    Console::new(senders, instances.shared.clone()).run(running.clone());

    running.store(false, Ordering::Relaxed);
    for handle in instances.threads {
        let _ = handle.join();
    }

    status::save_status();
    log::info!("server stopped.");
}

/// Prints the startup banner. Goes through print_always rather than log::info so
/// it shows even at log_level 0 (see Logging::log_level in config.rs).
fn print_banner() {
    let target = module_path!();
    for line in [
        "---------AdvancedServer---------".to_string(),
        "Created by: The Arctic Fox".to_string(),
        "Contains code referenced from: BetterServer (MIT)".to_string(),
        format!("Version: {}", SERVER_VERSION),
        format!(
            "Built on {} at {} for {} via {}",
            env!("BUILD_DATE"),
            env!("BUILD_TIME"),
            env!("BUILD_TARGET_OS"),
            env!("BUILD_RUSTC_VERSION"),
        ),
        "Licensed under GNU AGPL 3.0. Source code available at: \
         https://thearcticfox25.github.io/AdvancedServer/".to_string(),
        "--------------------------------".to_string(),
        "type ?help for terminal commands".to_string(),
    ] {
        terminal::print_always(target, &line);
    }
}

/// Loads Config.toml and refuses to start on a configuration that cannot work,
/// unless the owner has explicitly said they know better.
fn load_verified_config() -> config::Config {
    let cfg = match load_config() {
        Ok(loaded) => loaded,
        Err(error) => {
            log::error!("failed to load config: {}", error);
            std::process::exit(1);
        }
    };

    if cfg.verify() {
        return cfg;
    }
    if cfg.miscellaneous.other.ignore_inadequate_configuration {
        log::warn!("configuration verification failed but ignore_inadequate_configuration is true. continuing.");
        return cfg;
    }

    log::error!("Ильич, ты чо, долбаēб что ли?");
    log::error!("configuration verification failed. fix the config or set ignore_inadequate_configuration = true.");
    std::process::exit(1);
}

/// Installs the signal handlers and returns the flag every loop in the process
/// polls: false means "shut down".
///
/// signal_hook::flag::register can only ever set a flag to `true`, never back to
/// `false`, so the shutdown signals set a flag of their own and a watcher thread
/// translates that into `running = false`.
fn install_signal_handlers() -> Arc<AtomicBool> {
    let running = Arc::new(AtomicBool::new(true));

    #[cfg(not(windows))]
    let shutdown_signals = &[
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGHUP,
    ];
    #[cfg(windows)]
    let shutdown_signals = &[
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGTERM,
    ];

    let term_requested = Arc::new(AtomicBool::new(false));
    for &sig in shutdown_signals {
        if let Err(error) = signal_hook::flag::register(sig, term_requested.clone()) {
            log::warn!("failed to register signal {} handler: {}", sig, error);
        }
    }

    let watcher_flag = running.clone();
    std::thread::spawn(move || {
        while !term_requested.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        watcher_flag.store(false, Ordering::Relaxed);
    });

    #[cfg(not(windows))]
    if let Err(error) = signal_hook::flag::register(
        signal_hook::consts::SIGPIPE,
        Arc::new(AtomicBool::new(false)),
    ) {
        log::warn!("failed to ignore sigpipe: {}", error);
    }

    install_crash_handler();
    running
}

/// Best-effort crash handling: on SIGABRT, try to flush round stats to
/// Status.json before the process dies. Uses try_lock (see
/// save_status_best_effort) rather than the normal save_status, since abort() on
/// this same thread could have been triggered while it already held that lock.
///
/// SIGSEGV is deliberately NOT handled here: signal-hook-registry refuses to
/// register it at all (it is in its hard-coded FORBIDDEN list, alongside
/// SIGILL/SIGFPE) because a segfault leaves the process in a state where running
/// arbitrary handler code is unsound, and Rust's std already installs its own
/// SIGSEGV handler to detect stack overflow specifically. Overriding that via a
/// lower-level unchecked API would fight the runtime's own safety net for a case
/// (memory corruption via unsafe/FFI, e.g. rusqlite's C internals) where nothing
/// could be safely saved anyway.
#[cfg(not(windows))]
fn install_crash_handler() {
    let registered = unsafe {
        signal_hook::low_level::register(signal_hook::consts::SIGABRT, || {
            status::save_status_best_effort();
            signal_hook::low_level::exit(1);
        })
    };
    if let Err(error) = registered {
        log::warn!("failed to register SIGABRT crash handler: {}", error);
    }
}

#[cfg(windows)]
fn install_crash_handler() {}

/// The threads running the server instances, plus the live peer counts the
/// console and the lobby-redirect logic read out of them.
struct Instances {
    threads: Vec<std::thread::JoinHandle<()>>,
    shared: Vec<Arc<ServerShared>>,
}

/// Starts one worker thread per configured lobby.
fn spawn_instances(running: Arc<AtomicBool>) -> (Vec<mpsc::SyncSender<ConsoleCmd>>, Instances) {
    let cfg = config::cfg();
    let base_port    = cfg.server_config.networking.port;
    let server_count = cfg.server_config.networking.server_count;

    terminal::print_always(
        module_path!(),
        &format!("starting {} server(s) on base port {}", server_count, base_port),
    );

    // Built up front so every worker can see every sibling instance's live peer
    // count, which is what lets a full lobby redirect a player to a free one
    // instead of rejecting them (see connection::handle_identity).
    let shared: Vec<Arc<ServerShared>> =
        (0..server_count).map(|_| Arc::new(ServerShared::new())).collect();

    let mut senders = Vec::new();
    let mut threads = Vec::new();

    for id in 0..server_count {
        let (tx, rx) = mpsc::sync_channel::<ConsoleCmd>(64);
        senders.push(tx);

        let running    = running.clone();
        let own_shared = shared[id as usize].clone();
        let all_shared = shared.clone();
        let port       = base_port + id;

        threads.push(std::thread::spawn(move || {
            supervise_instance(id, port, running, rx, own_shared, all_shared);
        }));
    }

    (senders, Instances { threads, shared })
}

/// Runs one server instance, restarting it if it panics.
///
/// A panic inside the worker unwinds only its own thread, but without a restart
/// the instance would be silently dead for the life of the process. A normal
/// return (clean shutdown, or a fatal bind error) ends the loop instead.
fn supervise_instance(
    server_id: u16,
    port: u16,
    running: Arc<AtomicBool>,
    cmd_rx: mpsc::Receiver<ConsoleCmd>,
    shared: Arc<ServerShared>,
    all_shared: Vec<Arc<ServerShared>>,
) {
    while running.load(Ordering::Relaxed) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            server_worker(
                server_id,
                port,
                running.clone(),
                &cmd_rx,
                shared.clone(),
                all_shared.clone(),
            );
        }));
        if result.is_ok() {
            break;
        }
        log::error!("[Server {}] worker panicked; restarting instance in 1s", server_id);
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
