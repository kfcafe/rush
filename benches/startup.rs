use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::process::Command;
use std::time::Duration;

/// Benchmark the startup time of the Rush shell
/// Target: < 10ms from invocation to ready state
fn bench_startup_time(c: &mut Criterion) {
    let mut group = c.benchmark_group("startup");

    // Configure to run for a reasonable time
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(100);

    // Benchmark cold startup with exit
    group.bench_function("cold_start_exit", |b| {
        b.iter(|| {
            let output = Command::new("target/release/rush")
                .arg("-c")
                .arg("exit")
                .output()
                .expect("Failed to execute rush");
            black_box(output);
        });
    });

    // Benchmark startup with simple echo
    group.bench_function("start_echo_exit", |b| {
        b.iter(|| {
            let output = Command::new("target/release/rush")
                .arg("-c")
                .arg("echo test")
                .output()
                .expect("Failed to execute rush");
            black_box(output);
        });
    });

    group.finish();
}

/// Benchmark lexer initialization and tokenization
fn bench_lexer_init(c: &mut Criterion) {
    let mut group = c.benchmark_group("lexer");

    group.bench_function("tokenize_simple", |b| {
        b.iter(|| {
            let tokens = rush::lexer::Lexer::tokenize(black_box("echo hello world"));
            black_box(tokens);
        });
    });

    group.bench_function("tokenize_complex", |b| {
        b.iter(|| {
            let tokens = rush::lexer::Lexer::tokenize(black_box("ls -la | grep .rs | wc -l"));
            black_box(tokens);
        });
    });

    group.bench_function("tokenize_pipeline", |b| {
        b.iter(|| {
            let tokens =
                rush::lexer::Lexer::tokenize(black_box("cat file.txt | sort | uniq | head -n 10"));
            black_box(tokens);
        });
    });

    group.finish();
}

/// Benchmark parser initialization and parsing
fn bench_parser_init(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser");

    group.bench_function("parse_simple", |b| {
        let tokens = rush::lexer::Lexer::tokenize("echo hello").unwrap();
        b.iter(|| {
            let mut parser = rush::parser::Parser::new(black_box(tokens.clone()));
            let ast = parser.parse();
            black_box(ast);
        });
    });

    group.bench_function("parse_pipeline", |b| {
        let tokens = rush::lexer::Lexer::tokenize("ls | grep test | wc -l").unwrap();
        b.iter(|| {
            let mut parser = rush::parser::Parser::new(black_box(tokens.clone()));
            let ast = parser.parse();
            black_box(ast);
        });
    });

    group.finish();
}

/// Benchmark executor initialization
fn bench_executor_init(c: &mut Criterion) {
    c.bench_function("executor_new", |b| {
        b.iter(|| {
            let executor = rush::executor::Executor::new();
            black_box(executor);
        });
    });
}

/// Benchmark runtime initialization
fn bench_runtime_init(c: &mut Criterion) {
    c.bench_function("runtime_new", |b| {
        b.iter(|| {
            let runtime = rush::runtime::Runtime::new();
            black_box(runtime);
        });
    });
}

/// Benchmark memory footprint at various stages
fn bench_memory_footprint(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory");

    // Measure allocations during executor creation
    group.bench_function("executor_allocation", |b| {
        b.iter(|| {
            let executor = rush::executor::Executor::new();
            // Force allocation to be measured
            std::mem::size_of_val(&executor);
            black_box(executor);
        });
    });

    group.finish();
}

criterion_group!(
    startup_benches,
    bench_startup_time,
    bench_lexer_init,
    bench_parser_init,
    bench_executor_init,
    bench_runtime_init,
    bench_memory_footprint
);

criterion_main!(startup_benches);
