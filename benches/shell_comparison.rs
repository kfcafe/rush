use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rush::executor::Executor;
use rush::lexer::Lexer;
use rush::parser::Parser;
use std::fs;
use std::path::PathBuf;

fn setup_benchmark_data() {
    let bench_dir = PathBuf::from("benchmarks/benchmark-data");

    if !bench_dir.exists() {
        fs::create_dir_all(&bench_dir).unwrap();
    }

    // Create large file
    let large_file = bench_dir.join("large-file.txt");
    if !large_file.exists() {
        let mut content = String::new();
        for i in 1..=10000 {
            content.push_str(&format!(
                "Line {}: Lorem ipsum dolor sit amet, consectetur adipiscing elit.\n",
                i
            ));
        }
        fs::write(&large_file, content).unwrap();
    }

    // Create deep tree
    let deep_tree = bench_dir.join("deep-tree");
    if !deep_tree.exists() {
        fs::create_dir_all(&deep_tree).unwrap();
        for i in 1..=20 {
            let dir = deep_tree.join(format!("dir{}", i));
            fs::create_dir_all(&dir).unwrap();
            for j in 1..=50 {
                let file = dir.join(format!("file{}.txt", j));
                fs::write(&file, format!("File content {}-{}", i, j)).unwrap();
            }
        }
    }

    // Create grep test file
    let grep_file = bench_dir.join("grep-test.txt");
    if !grep_file.exists() {
        let mut content = String::new();
        for i in 1..=5000 {
            if i % 10 == 0 {
                content.push_str(&format!("FOUND: This is a matching line {}\n", i));
            } else {
                content.push_str(&format!("Regular line {} with some content\n", i));
            }
        }
        fs::write(&grep_file, content).unwrap();
    }
}

fn execute_command(cmd: &str) {
    let mut executor = Executor::new();
    let tokens = Lexer::tokenize(cmd).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let _ = executor.execute(statements);
}

fn bench_command_overhead(c: &mut Criterion) {
    c.bench_function("command_overhead_echo", |b| {
        b.iter(|| {
            execute_command(black_box("echo test"));
        });
    });
}

fn bench_cat_large_file(c: &mut Criterion) {
    setup_benchmark_data();

    c.bench_function("cat_large_file", |b| {
        b.iter(|| {
            execute_command(black_box("cat benchmarks/benchmark-data/large-file.txt"));
        });
    });
}

fn bench_ls_operations(c: &mut Criterion) {
    setup_benchmark_data();

    let mut group = c.benchmark_group("ls_operations");

    group.bench_function("ls_many_files", |b| {
        b.iter(|| {
            execute_command(black_box("ls benchmarks/benchmark-data/deep-tree/dir1"));
        });
    });

    group.bench_function("ls_long_format", |b| {
        b.iter(|| {
            execute_command(black_box("ls -la benchmarks/benchmark-data/deep-tree/dir1"));
        });
    });

    group.finish();
}

fn bench_find_operations(c: &mut Criterion) {
    setup_benchmark_data();

    c.bench_function("find_deep_tree", |b| {
        b.iter(|| {
            execute_command(black_box(
                "find benchmarks/benchmark-data/deep-tree -name \"*.txt\"",
            ));
        });
    });
}

fn bench_grep_operations(c: &mut Criterion) {
    setup_benchmark_data();

    c.bench_function("grep_large_file", |b| {
        b.iter(|| {
            execute_command(black_box(
                "grep \"FOUND\" benchmarks/benchmark-data/grep-test.txt",
            ));
        });
    });
}

fn bench_builtin_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("builtins");

    group.bench_function("pwd", |b| {
        b.iter(|| {
            execute_command(black_box("pwd"));
        });
    });

    group.bench_function("echo", |b| {
        b.iter(|| {
            execute_command(black_box("echo hello world"));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_command_overhead,
    bench_cat_large_file,
    bench_ls_operations,
    bench_find_operations,
    bench_grep_operations,
    bench_builtin_operations
);

criterion_main!(benches);
