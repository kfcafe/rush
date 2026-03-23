use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rush::builtins::Builtins;
use rush::runtime::Runtime;
use std::process::Command;
use std::time::Duration;

/// Benchmark builtin commands vs their GNU equivalents
/// This helps ensure Rush builtins are competitive with system commands

fn bench_echo_builtin(c: &mut Criterion) {
    let mut group = c.benchmark_group("echo");
    group.measurement_time(Duration::from_secs(5));

    let builtins = Builtins::new();
    let mut runtime = Runtime::new();

    // Benchmark Rush builtin echo
    group.bench_function("rush_builtin", |b| {
        b.iter(|| {
            let args = vec!["hello".to_string(), "world".to_string()];
            let result = builtins.execute("echo", black_box(args), &mut runtime);
            black_box(result);
        });
    });

    // Benchmark system echo via Rush
    group.bench_function("rush_system", |b| {
        b.iter(|| {
            let output = Command::new("target/release/rush")
                .arg("-c")
                .arg("echo hello world")
                .output()
                .expect("Failed to execute");
            black_box(output);
        });
    });

    // Benchmark GNU echo directly (baseline)
    group.bench_function("gnu_baseline", |b| {
        b.iter(|| {
            let output = Command::new("echo")
                .arg("hello")
                .arg("world")
                .output()
                .expect("Failed to execute");
            black_box(output);
        });
    });

    group.finish();
}

fn bench_pwd_builtin(c: &mut Criterion) {
    let mut group = c.benchmark_group("pwd");
    group.measurement_time(Duration::from_secs(5));

    let builtins = Builtins::new();
    let mut runtime = Runtime::new();

    // Benchmark Rush builtin pwd
    group.bench_function("rush_builtin", |b| {
        b.iter(|| {
            let result = builtins.execute("pwd", vec![], &mut runtime);
            black_box(result);
        });
    });

    // Benchmark system pwd via Rush
    group.bench_function("rush_system", |b| {
        b.iter(|| {
            let output = Command::new("target/release/rush")
                .arg("-c")
                .arg("pwd")
                .output()
                .expect("Failed to execute");
            black_box(output);
        });
    });

    // Benchmark GNU pwd directly (baseline)
    group.bench_function("gnu_baseline", |b| {
        b.iter(|| {
            let output = Command::new("pwd").output().expect("Failed to execute");
            black_box(output);
        });
    });

    group.finish();
}

fn bench_cd_builtin(c: &mut Criterion) {
    let mut group = c.benchmark_group("cd");
    group.measurement_time(Duration::from_secs(5));

    let builtins = Builtins::new();
    let mut runtime = Runtime::new();

    // Benchmark Rush builtin cd
    group.bench_function("rush_builtin_home", |b| {
        b.iter(|| {
            let args = vec![];
            let result = builtins.execute("cd", black_box(args), &mut runtime);
            black_box(result);
        });
    });

    group.bench_function("rush_builtin_tmp", |b| {
        b.iter(|| {
            let args = vec!["/tmp".to_string()];
            let result = builtins.execute("cd", black_box(args), &mut runtime);
            black_box(result);
        });
    });

    group.bench_function("rush_builtin_relative", |b| {
        b.iter(|| {
            let args = vec!["..".to_string()];
            let result = builtins.execute("cd", black_box(args), &mut runtime);
            black_box(result);
        });
    });

    group.finish();
}

fn bench_export_builtin(c: &mut Criterion) {
    let mut group = c.benchmark_group("export");
    group.measurement_time(Duration::from_secs(5));

    let builtins = Builtins::new();
    let mut runtime = Runtime::new();

    // Benchmark Rush builtin export
    group.bench_function("rush_builtin_single", |b| {
        b.iter(|| {
            let args = vec!["TEST=value".to_string()];
            let result = builtins.execute("export", black_box(args), &mut runtime);
            black_box(result);
        });
    });

    group.bench_function("rush_builtin_multiple", |b| {
        b.iter(|| {
            let args = vec![
                "TEST1=value1".to_string(),
                "TEST2=value2".to_string(),
                "TEST3=value3".to_string(),
            ];
            let result = builtins.execute("export", black_box(args), &mut runtime);
            black_box(result);
        });
    });

    group.finish();
}

/// Benchmark builtin lookup performance
fn bench_builtin_dispatch(c: &mut Criterion) {
    let mut group = c.benchmark_group("dispatch");

    let builtins = Builtins::new();

    // Benchmark builtin existence check
    group.bench_function("is_builtin_hit", |b| {
        b.iter(|| {
            let result = builtins.is_builtin(black_box("echo"));
            black_box(result);
        });
    });

    group.bench_function("is_builtin_miss", |b| {
        b.iter(|| {
            let result = builtins.is_builtin(black_box("nonexistent"));
            black_box(result);
        });
    });

    group.finish();
}

/// Benchmark various argument counts
fn bench_builtin_arg_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("arg_scaling");
    group.measurement_time(Duration::from_secs(5));

    let builtins = Builtins::new();
    let mut runtime = Runtime::new();

    // Test echo with different argument counts
    for arg_count in [1, 5, 10, 25, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("echo_args", arg_count),
            arg_count,
            |b, &count| {
                let args: Vec<String> = (0..count).map(|i| format!("arg{}", i)).collect();
                b.iter(|| {
                    let result = builtins.execute("echo", black_box(args.clone()), &mut runtime);
                    black_box(result);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark builtin initialization
fn bench_builtins_init(c: &mut Criterion) {
    c.bench_function("builtins_new", |b| {
        b.iter(|| {
            let builtins = Builtins::new();
            black_box(builtins);
        });
    });
}

/// Benchmark find builtin vs GNU find
fn bench_find_builtin(c: &mut Criterion) {
    use std::fs;
    use tempfile::TempDir;

    let mut group = c.benchmark_group("find");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(20);

    // Create a test directory with many files
    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path();

    // Create a realistic directory structure (1000+ files)
    for i in 0..50 {
        let dir = base_path.join(format!("dir{}", i));
        fs::create_dir_all(&dir).unwrap();
        for j in 0..20 {
            fs::write(dir.join(format!("file{}.txt", j)), "content").unwrap();
            fs::write(dir.join(format!("test{}.rs", j)), "content").unwrap();
        }
    }

    let builtins = Builtins::new();
    let mut runtime = Runtime::new();
    runtime.set_cwd(base_path.to_path_buf());

    // Benchmark Rush builtin find (all files)
    group.bench_function("rush_builtin_all", |b| {
        b.iter(|| {
            let result = builtins.execute("find", vec![], &mut runtime);
            black_box(result);
        });
    });

    // Benchmark Rush builtin find with pattern
    group.bench_function("rush_builtin_pattern", |b| {
        b.iter(|| {
            let args = vec!["-name".to_string(), "*.rs".to_string()];
            let result = builtins.execute("find", black_box(args), &mut runtime);
            black_box(result);
        });
    });

    // Benchmark Rush builtin find with type filter
    group.bench_function("rush_builtin_type", |b| {
        b.iter(|| {
            let args = vec!["-type".to_string(), "f".to_string()];
            let result = builtins.execute("find", black_box(args), &mut runtime);
            black_box(result);
        });
    });

    // Benchmark GNU find (all files)
    group.bench_function("gnu_find_all", |b| {
        b.iter(|| {
            let output = Command::new("find")
                .arg(base_path)
                .output()
                .expect("Failed to execute");
            black_box(output);
        });
    });

    // Benchmark GNU find with pattern
    group.bench_function("gnu_find_pattern", |b| {
        b.iter(|| {
            let output = Command::new("find")
                .arg(base_path)
                .arg("-name")
                .arg("*.rs")
                .output()
                .expect("Failed to execute");
            black_box(output);
        });
    });

    // Benchmark GNU find with type filter
    group.bench_function("gnu_find_type", |b| {
        b.iter(|| {
            let output = Command::new("find")
                .arg(base_path)
                .arg("-type")
                .arg("f")
                .output()
                .expect("Failed to execute");
            black_box(output);
        });
    });

    group.finish();
}

criterion_group!(
    builtin_benches,
    bench_echo_builtin,
    bench_pwd_builtin,
    bench_cd_builtin,
    bench_export_builtin,
    bench_builtin_dispatch,
    bench_builtin_arg_scaling,
    bench_builtins_init,
    bench_find_builtin
);

criterion_main!(builtin_benches);
