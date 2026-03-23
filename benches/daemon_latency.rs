use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::collections::HashMap;
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Daemon lifecycle helpers
// ---------------------------------------------------------------------------

fn socket_path() -> String {
    format!(
        "{}/.rush/daemon.sock",
        std::env::var("HOME").unwrap_or_default()
    )
}

fn is_daemon_running() -> bool {
    let path = socket_path();
    std::path::Path::new(&path).exists() && UnixStream::connect(&path).is_ok()
}

fn start_daemon() {
    let _ = Command::new("target/release/rushd")
        .arg("stop")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(Duration::from_millis(300));

    let _ = Command::new("target/release/rushd")
        .arg("start")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start rushd");

    let path = socket_path();
    for _ in 0..50 {
        if std::path::Path::new(&path).exists() {
            if UnixStream::connect(&path).is_ok() {
                return;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("Daemon failed to start within 5 seconds");
}

fn stop_daemon() {
    let _ = Command::new("target/release/rushd")
        .arg("stop")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(Duration::from_millis(300));
}

fn ensure_daemon() {
    if !is_daemon_running() {
        start_daemon();
    }
}

// ---------------------------------------------------------------------------
// Daemon protocol (bincode over unix socket)
// ---------------------------------------------------------------------------

fn execute_via_daemon(cmd: &str) -> i32 {
    use rush::daemon::protocol::{read_message, write_message, Message, SessionInit};

    let path = socket_path();
    let mut stream = UnixStream::connect(&path).expect("Failed to connect to daemon socket");

    let working_dir = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
        .to_string_lossy()
        .to_string();

    let mut env = HashMap::new();
    env.insert(
        "PATH".to_string(),
        std::env::var("PATH").unwrap_or_default(),
    );

    let init = SessionInit {
        working_dir,
        env,
        args: vec!["-c".to_string(), cmd.to_string()],
        stdin_mode: "null".to_string(),
    };

    write_message(&mut stream, &Message::SessionInit(init), 1).expect("Failed to write message");

    let (response, _) = read_message(&mut stream).expect("Failed to read response");

    match response {
        Message::ExecutionResult(result) => result.exit_code,
        _ => panic!("Unexpected response type"),
    }
}

// ===========================================================================
// 1. DAEMON EXECUTION (primary benchmark)
//    This is what matters — pre-warmed workers, bincode IPC, no process spawn.
// ===========================================================================

fn bench_daemon_execution(c: &mut Criterion) {
    ensure_daemon();

    let mut group = c.benchmark_group("daemon");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(100);

    for (name, cmd) in [
        ("true", "true"),
        ("echo_hello", "echo hello"),
        ("arithmetic", "echo $((2+3))"),
        ("pipe", "echo hello | cat"),
    ] {
        group.bench_with_input(BenchmarkId::new("exec", name), &cmd, |b, cmd| {
            b.iter(|| {
                black_box(execute_via_daemon(black_box(cmd)));
            });
        });
    }

    group.finish();
}

// ===========================================================================
// 2. DAEMON THROUGHPUT
//    Sequential burst — how many commands/sec can the daemon sustain?
// ===========================================================================

fn bench_daemon_throughput(c: &mut Criterion) {
    ensure_daemon();

    let mut group = c.benchmark_group("daemon_throughput");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(20);

    for (name, cmd, batch) in [("100x_true", "true", 100), ("100x_echo", "echo hello", 100)] {
        group.bench_function(name, |b| {
            b.iter(|| {
                for _ in 0..batch {
                    black_box(execute_via_daemon(black_box(cmd)));
                }
            });
        });
    }

    group.finish();
}

// ===========================================================================
// 3. DAEMON COLD START
//    Stop daemon, restart, time the first command. Measures fork+init cost.
// ===========================================================================

fn bench_daemon_cold_start(c: &mut Criterion) {
    let mut group = c.benchmark_group("daemon_cold_start");
    group.measurement_time(Duration::from_secs(30));
    group.sample_size(10);

    group.bench_function("first_command", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                stop_daemon();
                start_daemon();

                let start = Instant::now();
                black_box(execute_via_daemon("true"));
                total += start.elapsed();
            }
            total
        });
    });

    group.finish();
}

// ===========================================================================
// 4. COLD STARTUP — rush -c (single reference point)
//    Process spawn → rush binary load → lex/parse/exec → exit.
//    This is the baseline that the daemon amortizes away.
// ===========================================================================

fn bench_cold_startup(c: &mut Criterion) {
    let mut group = c.benchmark_group("cold_startup");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(50);

    for (name, cmd) in [("true", "true"), ("echo_hello", "echo hello")] {
        group.bench_with_input(BenchmarkId::new("rush_c", name), &cmd, |b, cmd| {
            b.iter(|| {
                black_box(
                    Command::new("target/release/rush")
                        .arg("-c")
                        .arg(cmd)
                        .output()
                        .expect("Failed to execute rush"),
                );
            });
        });
    }

    group.finish();
}

// ===========================================================================
// 5. SHELL COMPARISON — rush -c vs bash -c vs zsh -c
//    Context only — measures process spawn overhead across shells.
// ===========================================================================

fn bench_shell_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("shell_comparison");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(50);

    let mut shells: Vec<(&str, String)> = vec![
        ("rush", "target/release/rush".to_string()),
        ("bash", "/bin/bash".to_string()),
    ];

    if let Ok(output) = Command::new("which").arg("zsh").output() {
        if let Ok(path) = String::from_utf8(output.stdout) {
            let path = path.trim().to_string();
            if !path.is_empty() {
                shells.push(("zsh", path));
            }
        }
    }

    for (name, path) in &shells {
        group.bench_with_input(
            BenchmarkId::new("echo_hello", name),
            path.as_str(),
            |b, path| {
                b.iter(|| {
                    black_box(
                        Command::new(path)
                            .arg("-c")
                            .arg("echo hello")
                            .output()
                            .expect("Failed to execute shell"),
                    );
                });
            },
        );
    }

    group.finish();
}

// ===========================================================================

criterion_group!(
    benches,
    bench_daemon_execution,  // PRIMARY: daemon warm execution
    bench_daemon_throughput, // Sustained throughput
    bench_daemon_cold_start, // Cold start overhead
    bench_cold_startup,      // rush -c reference (single variable)
    bench_shell_comparison,  // Context: rush vs bash vs zsh
);

criterion_main!(benches);
