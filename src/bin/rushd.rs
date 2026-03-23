//! Rush daemon server binary
//!
//! Provides commands to start, stop, and manage the Rush daemon.

use anyhow::{anyhow, Result};
use nix::libc;
use rush::daemon::server::DaemonServer;
use rush::daemon::worker_pool::PoolConfig;
use std::env;
use std::fs;
use std::process;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    let command = &args[1];

    match command.as_str() {
        "start" => start_daemon(),
        "stop" => stop_daemon(),
        "status" => check_status(),
        "restart" => restart_daemon(),
        "reload" => reload_config(),
        "-h" | "--help" => {
            print_usage();
            Ok(())
        }
        _ => {
            eprintln!("Error: Unknown command '{}'", command);
            print_usage();
            process::exit(1);
        }
    }
}

fn start_daemon() -> Result<()> {
    let socket_path = DaemonServer::default_socket_path()?;

    // Check if daemon is already running
    if socket_path.exists() {
        // Try to connect to verify it's actually running
        if let Ok(_stream) = std::os::unix::net::UnixStream::connect(&socket_path) {
            eprintln!(
                "Error: Daemon is already running at {}",
                socket_path.display()
            );
            eprintln!("Use 'rushd stop' to stop it first, or 'rushd restart' to restart.");
            process::exit(1);
        } else {
            // Stale socket file, remove it
            fs::remove_file(&socket_path)?;
        }
    }

    // Create the daemon
    let mut daemon = DaemonServer::new(socket_path.clone())?;

    // Enable worker pool unless disabled via environment variable
    // RUSH_DISABLE_POOL=1 will use fork-per-request mode
    let use_pool = env::var("RUSH_DISABLE_POOL")
        .map(|v| v != "1")
        .unwrap_or(true); // Default: use pool

    if use_pool {
        // Get pool size from environment or use default
        let pool_size = env::var("RUSH_POOL_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4); // Default: 4 workers

        let config = PoolConfig {
            pool_size,
            max_queue_size: 100,
        };

        daemon = daemon.with_worker_pool(config)?;
        eprintln!("Worker pool mode enabled ({} workers)", pool_size);
    } else {
        eprintln!("Fork-per-request mode enabled (legacy)");
    }

    println!("Starting Rush daemon at {}", socket_path.display());
    println!("Use 'rush -c <command>' to execute commands via the daemon.");
    println!("Press Ctrl-C to stop the daemon.");

    daemon.start()?;

    Ok(())
}

fn stop_daemon() -> Result<()> {
    let socket_path = DaemonServer::default_socket_path()?;

    if !socket_path.exists() {
        println!("Daemon is not running (socket not found).");
        return Ok(());
    }

    // Try to connect and send shutdown signal
    match std::os::unix::net::UnixStream::connect(&socket_path) {
        Ok(_stream) => {
            // For now, we'll use the socket file existence as a proxy
            // In a full implementation, we'd send a Shutdown message

            // Read PID from a potential PID file
            let pid_path = socket_path
                .parent()
                .ok_or_else(|| anyhow!("Invalid socket path"))?
                .join("daemon.pid");

            if pid_path.exists() {
                let pid_str = fs::read_to_string(&pid_path)?;
                let pid: i32 = pid_str
                    .trim()
                    .parse()
                    .map_err(|_| anyhow!("Invalid PID in daemon.pid"))?;

                // Send SIGTERM to the daemon
                unsafe {
                    libc::kill(pid, libc::SIGTERM);
                }

                println!("Sent shutdown signal to daemon (PID {}).", pid);

                // Wait for socket to be removed (up to 5 seconds)
                for _ in 0..50 {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    if !socket_path.exists() {
                        println!("Daemon stopped.");
                        fs::remove_file(&pid_path).ok();
                        return Ok(());
                    }
                }

                eprintln!("Warning: Daemon may not have stopped cleanly.");
                fs::remove_file(&pid_path).ok();
            } else {
                eprintln!("Warning: PID file not found. Cannot send signal to daemon.");
                eprintln!("You may need to manually kill the daemon process.");
            }
        }
        Err(_) => {
            // Socket exists but can't connect - likely stale
            println!("Removing stale socket file.");
            fs::remove_file(&socket_path)?;
        }
    }

    Ok(())
}

fn check_status() -> Result<()> {
    let socket_path = DaemonServer::default_socket_path()?;

    if !socket_path.exists() {
        println!("Daemon is not running (socket not found).");
        return Ok(());
    }

    // Try to connect to the socket
    match std::os::unix::net::UnixStream::connect(&socket_path) {
        Ok(_stream) => {
            println!("Daemon is running at {}", socket_path.display());

            // Try to read PID
            let pid_path = socket_path
                .parent()
                .ok_or_else(|| anyhow!("Invalid socket path"))?
                .join("daemon.pid");

            if pid_path.exists() {
                if let Ok(pid_str) = fs::read_to_string(&pid_path) {
                    println!("PID: {}", pid_str.trim());
                }
            }
        }
        Err(_) => {
            println!("Socket file exists but daemon is not responding.");
            println!("This may be a stale socket. Try 'rushd start' to restart.");
        }
    }

    Ok(())
}

fn restart_daemon() -> Result<()> {
    println!("Stopping daemon...");
    stop_daemon()?;

    // Brief pause to ensure cleanup
    std::thread::sleep(std::time::Duration::from_millis(500));

    println!("Starting daemon...");
    start_daemon()?;

    Ok(())
}

/// Reload daemon configuration by sending SIGHUP
///
/// This is equivalent to `kill -HUP $(cat ~/.rush/daemon.pid)` but more convenient.
/// The daemon will re-parse .rushrc and update custom stat definitions without restart.
fn reload_config() -> Result<()> {
    let socket_path = DaemonServer::default_socket_path()?;

    if !socket_path.exists() {
        eprintln!("Error: Daemon is not running (socket not found).");
        process::exit(1);
    }

    // Verify daemon is actually running
    if std::os::unix::net::UnixStream::connect(&socket_path).is_err() {
        eprintln!("Error: Socket exists but daemon is not responding.");
        eprintln!("Try 'rushd restart' to restart the daemon.");
        process::exit(1);
    }

    // Read PID from pid file
    let pid_path = socket_path
        .parent()
        .ok_or_else(|| anyhow!("Invalid socket path"))?
        .join("daemon.pid");

    if !pid_path.exists() {
        eprintln!("Error: PID file not found at {}", pid_path.display());
        eprintln!("Cannot send reload signal. Try 'rushd restart' instead.");
        process::exit(1);
    }

    let pid_str = fs::read_to_string(&pid_path)?;
    let pid: i32 = pid_str
        .trim()
        .parse()
        .map_err(|_| anyhow!("Invalid PID in daemon.pid"))?;

    // Send SIGHUP to the daemon
    let result = unsafe { libc::kill(pid, libc::SIGHUP) };

    if result == 0 {
        println!("Sent reload signal (SIGHUP) to daemon (PID {}).", pid);
        println!("Configuration will be reloaded from ~/.rushrc");
    } else {
        let err = std::io::Error::last_os_error();
        eprintln!("Error: Failed to send signal: {}", err);
        process::exit(1);
    }

    Ok(())
}

fn print_usage() {
    println!("Rush Daemon Server v0.1.0");
    println!();
    println!("Usage: rushd <command>");
    println!();
    println!("Commands:");
    println!("  start      Start the Rush daemon");
    println!("  stop       Stop the Rush daemon");
    println!("  status     Check daemon status");
    println!("  restart    Restart the daemon");
    println!("  reload     Reload configuration from ~/.rushrc (via SIGHUP)");
    println!("  -h, --help Show this help message");
    println!();
    println!("Examples:");
    println!("  rushd start    # Start the daemon");
    println!("  rushd status   # Check if daemon is running");
    println!("  rushd stop     # Stop the daemon");
    println!("  rushd reload   # Reload config without restart");
    println!();
    println!("Configuration:");
    println!("  The daemon reads configuration from ~/.rushrc on startup.");
    println!("  Use 'rushd reload' or 'kill -HUP <pid>' to reload config.");
    println!("  See docs/design/banner-stats.md for configuration options.");
}
