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

    // Banner always prints, regardless of log_level (see Logging::log_level).
    let banner_target = module_path!();
    terminal::print_always(banner_target, "---------AdvancedServer---------");
    terminal::print_always(banner_target, "Created by: The Arctic Fox");
    terminal::print_always(banner_target, "Contains code referenced from: BetterServer (MIT)");
    terminal::print_always(banner_target, &format!("Version: {}", SERVER_VERSION));
    terminal::print_always(banner_target, &format!(
        "Built on {} at {} for {} via {}",
        env!("BUILD_DATE"),
        env!("BUILD_TIME"),
        env!("BUILD_TARGET_OS"),
        env!("BUILD_RUSTC_VERSION")
    ));
    terminal::print_always(banner_target, "Licensed under GNU AGPL 3.0. Source code available at: https://thearcticfox25.github.io/AdvancedServer/");
    terminal::print_always(banner_target, "--------------------------------");
    terminal::print_always(banner_target, "type ?help for terminal commands");

    let cfg = match load_config() {
        Ok(c) => c,
        Err(e) => {
            log::error!("failed to load config: {}", e);
            std::process::exit(1);
        }
    };

    if !cfg.verify() {
        if !cfg.miscellaneous.other.ignore_inadequate_configuration {
            log::error!("Ильич, ты чо, долбаēб что ли?");
            log::error!("configuration verification failed. fix the config or set ignore_inadequate_configuration = true.");
            std::process::exit(1);
        } else {
            log::warn!("configuration verification failed but ignore_inadequate_configuration is true. continuing.");
        }
    }

    CONFIG.set(cfg).expect("config already set");
    terminal::init_file_logging();

    if std::env::var("RUST_LOG").is_err() {
        let level = terminal::level_filter_for(config::cfg().server_config.logging.log_level);
        log::set_max_level(level);
    }

    anticheat::physics::load("MapPhysics.json");

    init_moderation();
    init_status();

    let cfg = config::cfg();
    let base_port = cfg.server_config.networking.port;
    let server_count = cfg.server_config.networking.server_count;

    terminal::print_always(module_path!(), &format!("starting {} server(s) on base port {}", server_count, base_port));

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

    // signal_hook::flag::register can only set a flag to `true` on signal,
    // never to `false` -- so shutdown signals set a dedicated flag here, and
    // a watcher thread translates that into `running = false`, which every
    // loop below already polls for.
    let term_requested = Arc::new(AtomicBool::new(false));
    for &sig in shutdown_signals {
        if let Err(e) = signal_hook::flag::register(sig, term_requested.clone()) {
            log::warn!("failed to register signal {} handler: {}", sig, e);
        }
    }
    {
        let running = running.clone();
        let term_requested = term_requested.clone();
        std::thread::spawn(move || {
            while !term_requested.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            running.store(false, Ordering::Relaxed);
        });
    }

    #[cfg(not(windows))]
    if let Err(e) = signal_hook::flag::register(
        signal_hook::consts::SIGPIPE,
        Arc::new(AtomicBool::new(false)),
    ) {
        log::warn!("failed to ignore sigpipe: {}", e);
    }

    // Best-effort crash handling: on SIGABRT, try to flush round stats to
    // Status.json before the process dies. Uses try_lock (see
    // save_status_best_effort) rather than the normal save_status, since
    // abort() on this same thread could have been triggered while it
    // already held that lock. Not available on Windows.
    //
    // SIGSEGV is deliberately NOT handled here: signal-hook-registry refuses
    // to register it at all (it's in its hard-coded FORBIDDEN list, alongside
    // SIGILL/SIGFPE) because a segfault leaves the process in a state where
    // running arbitrary handler code is unsound, and Rust's std already
    // installs its own SIGSEGV handler to detect stack overflow specifically.
    // Overriding that via a lower-level unchecked API would fight the
    // runtime's own safety net for a case (memory corruption via unsafe/FFI,
    // e.g. rusqlite's C internals) where nothing can be safely saved anyway.
    #[cfg(not(windows))]
    if let Err(e) = unsafe {
        signal_hook::low_level::register(signal_hook::consts::SIGABRT, || {
            status::save_status_best_effort();
            signal_hook::low_level::exit(1);
        })
    } {
        log::warn!("failed to register SIGABRT crash handler: {}", e);
    }

    let mut senders: Vec<mpsc::SyncSender<ConsoleCmd>> = Vec::new();
    // Built up-front so every worker thread can see every sibling instance's
    // live peer count, needed to redirect a player to a free lobby instead
    // of rejecting them when its own lobby is full (see
    // connection::handle_identity).
    let shared_list: Vec<Arc<ServerShared>> = (0..server_count).map(|_| Arc::new(ServerShared::new())).collect();
    let mut handles = Vec::new();

    for i in 0..server_count {
        let port = base_port + i;
        let server_id = i;
        let run = running.clone();

        let (tx, rx) = mpsc::sync_channel::<ConsoleCmd>(64);
        let shared = shared_list[i as usize].clone();
        let all_shared = shared_list.clone();

        senders.push(tx);

        let handle = std::thread::spawn(move || {
            // Supervise the instance. A panic inside the worker unwinds only
            // this thread, but without a restart the instance is silently dead for the
            // life of the process. Catch the panic, log it, and re-spawn the instance.
            // A normal return (clean shutdown or fatal bind error) ends the loop.
            while run.load(Ordering::Relaxed) {
                let run_inner = run.clone();
                let shared_inner = shared.clone();
                let all_shared_inner = all_shared.clone();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    server_worker(server_id, port, run_inner, &rx, shared_inner, all_shared_inner);
                }));
                match result {
                    Ok(()) => break,
                    Err(_) => {
                        log::error!(
                            "[Server {}] worker panicked; restarting instance in 1s",
                            server_id
                        );
                        std::thread::sleep(std::time::Duration::from_secs(1));
                    }
                }
            }
        });
        handles.push(handle);
    }

    std::thread::sleep(std::time::Duration::from_millis(150));

    Console::new(senders, shared_list).run(running.clone());

    running.store(false, Ordering::Relaxed);

    for handle in handles {
        let _ = handle.join();
    }

    status::save_status();
    log::info!("server stopped.");
}
