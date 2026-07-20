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

    log::info!("---------AdvancedServer---------");
    log::info!("Created by: The Arctic Fox");
    log::info!("Contains code referenced from: BetterServer (MIT)");
    log::info!("Version: {}", SERVER_VERSION);
    log::info!(
        "Built on {} at {} for {} via {}",
        env!("BUILD_DATE"),
        env!("BUILD_TIME"),
        env!("BUILD_TARGET_OS"),
        env!("BUILD_RUSTC_VERSION")
    );
    log::info!("Licensed under GNU AGPL 3.0. Source code available at: https://thearcticfox25.github.io/AdvancedServer/");
    log::info!("--------------------------------");
    log::info!("type ?help for terminal commands");

    let cfg = match load_config() {
        Ok(c) => c,
        Err(e) => {
            log::error!("failed to load config: {}", e);
            std::process::exit(1);
        }
    };

    if !cfg.verify() {
        if !cfg.miscellaneous.other.ignore_inadequate_configuration {
            log::error!("configuration verification failed. fix the config or set ignore_inadequate_configuration = true.");
            std::process::exit(1);
        } else {
            log::warn!("configuration verification failed but ignore_inadequate_configuration is true. continuing.");
        }
    }

    CONFIG.set(cfg).expect("config already set");

    if std::env::var("RUST_LOG").is_err() {
        let level = if config::cfg().server_config.logging.log_debug {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        };
        log::set_max_level(level);
    }

    anticheat::physics::load("MapPhysics.json");

    init_moderation();
    init_status();

    let cfg = config::cfg();
    let base_port = cfg.server_config.networking.port;
    let server_count = cfg.server_config.networking.server_count;

    log::info!("starting {} server(s) on base port {}", server_count, base_port);

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

    for &sig in shutdown_signals {
        if let Err(e) = signal_hook::flag::register(sig, running.clone()) {
            log::warn!("failed to register signal {} handler: {}", sig, e);
        }
    }

    #[cfg(not(windows))]
    if let Err(e) = signal_hook::flag::register(
        signal_hook::consts::SIGPIPE,
        Arc::new(AtomicBool::new(false)),
    ) {
        log::warn!("failed to ignore sigpipe: {}", e);
    }

    let mut senders: Vec<mpsc::SyncSender<ConsoleCmd>> = Vec::new();
    let mut shared_list: Vec<Arc<ServerShared>> = Vec::new();
    let mut handles = Vec::new();

    for i in 0..server_count {
        let port = base_port + i;
        let server_id = i;
        let run = running.clone();

        let (tx, rx) = mpsc::sync_channel::<ConsoleCmd>(64);
        let shared = Arc::new(ServerShared::new());

        senders.push(tx);
        shared_list.push(shared.clone());

        let handle = std::thread::spawn(move || {
            // STAB-S1: supervise the instance. A panic inside the worker unwinds only
            // this thread, but without a restart the instance is silently dead for the
            // life of the process. Catch the panic, log it, and re-spawn the instance.
            // A normal return (clean shutdown or fatal bind error) ends the loop.
            while run.load(Ordering::Relaxed) {
                let run_inner = run.clone();
                let shared_inner = shared.clone();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    server_worker(server_id, port, run_inner, &rx, shared_inner);
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
