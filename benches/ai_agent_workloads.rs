use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rush::executor::Executor;
use rush::lexer::Lexer;
use rush::parser::Parser;
use rush::runtime::Runtime;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Setup a benchmark git repository with realistic structure
fn setup_git_repo() -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize git repo
    Command::new("git")
        .args(&["init"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Configure git
    Command::new("git")
        .args(&["config", "user.email", "bench@rush.sh"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(&["config", "user.name", "Bench User"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Create initial structure with realistic files
    let src_dir = repo_path.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    for i in 1..=50 {
        let content = format!(
            "// File {}\npub fn function_{}() {{\n    println!(\"test {}\");\n}}\n",
            i, i, i
        );
        fs::write(src_dir.join(format!("file{}.rs", i)), content).unwrap();
    }

    // Create initial commit
    Command::new("git")
        .args(&["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(&["commit", "-m", "Initial commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Create more commits for git log tests
    for i in 2..=100 {
        fs::write(
            src_dir.join(format!("file{}.rs", i % 50 + 1)),
            format!("// Updated in commit {}\npub fn function_{}() {{}}\n", i, i),
        )
        .unwrap();
        Command::new("git")
            .args(&["add", "."])
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(&["commit", "-m", &format!("Update {}", i)])
            .current_dir(repo_path)
            .output()
            .unwrap();
    }

    // Make some files modified but not staged
    fs::write(
        src_dir.join("file1.rs"),
        "// Modified\npub fn modified() {}\n",
    )
    .unwrap();
    fs::write(repo_path.join("untracked.rs"), "// Untracked\n").unwrap();

    temp_dir
}

/// Setup test data files with JSON content
fn setup_test_files() -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().join("data");
    fs::create_dir_all(&data_dir).unwrap();

    // Create 1000 JSON files for find benchmarks
    for i in 0..1000 {
        let json_content = serde_json::json!({
            "id": i,
            "name": format!("item_{}", i),
            "value": i * 100,
            "active": i % 2 == 0,
            "tags": ["tag1", "tag2", "tag3"],
            "metadata": {
                "created": "2024-01-01",
                "updated": "2024-01-15"
            }
        });
        fs::write(
            data_dir.join(format!("file{}.json", i)),
            serde_json::to_string_pretty(&json_content).unwrap(),
        )
        .unwrap();
    }

    // Create files with TODO comments for grep
    for i in 0..50 {
        let content = format!(
            "// File {}\nfn main() {{\n    // TODO: Implement feature\n    println!(\"hello\");\n}}\n",
            i
        );
        fs::write(data_dir.join(format!("source{}.rs", i)), content).unwrap();
    }

    temp_dir
}

/// Execute a Rush command
fn execute_command(cmd: &str, cwd: &PathBuf) -> String {
    let tokens = Lexer::tokenize(cmd).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();

    let mut executor = Executor::new_embedded();
    executor.runtime_mut().set_cwd(cwd.clone());

    let result = executor.execute(statements).unwrap();

    result.stdout()
}

/// Benchmark 1: Git Status Check Loop (100x calls)
/// Target: <500ms total (<5ms per call)
/// AI agents constantly check git status while working
fn bench_git_status_loop(c: &mut Criterion) {
    let repo = setup_git_repo();
    let repo_path = repo.path().to_path_buf();

    c.bench_function("git_status_json_100x", |b| {
        b.iter(|| {
            for _ in 0..100 {
                let output = execute_command(black_box("git_status --json"), black_box(&repo_path));
                black_box(output);
            }
        });
    });

    // Single call benchmark for detailed timing
    c.bench_function("git_status_json_single", |b| {
        b.iter(|| {
            let output = execute_command(black_box("git_status --json"), black_box(&repo_path));
            black_box(output);
        });
    });
}

/// Benchmark 2: Find + Filter + JSON
/// Target: <10ms for 1000 files
/// AI agents frequently search and filter files
fn bench_find_filter_json(c: &mut Criterion) {
    let test_files = setup_test_files();
    let data_dir = test_files.path().to_path_buf();

    let mut group = c.benchmark_group("find_filter_json");

    // Find all JSON files
    group.bench_function("find_json_files", |b| {
        b.iter(|| {
            let output = execute_command(
                black_box("find --json data/ -name \"*.json\""),
                black_box(&data_dir.parent().unwrap().to_path_buf()),
            );
            black_box(output);
        });
    });

    // Find with size filter
    group.bench_function("find_json_with_size_filter", |b| {
        b.iter(|| {
            let output = execute_command(
                black_box("find --json data/ -name \"*.json\" -size +1000"),
                black_box(&data_dir.parent().unwrap().to_path_buf()),
            );
            black_box(output);
        });
    });

    // Find with mtime filter
    group.bench_function("find_json_with_mtime_filter", |b| {
        b.iter(|| {
            let output = execute_command(
                black_box("find --json data/ -name \"*.rs\" -mtime -1"),
                black_box(&data_dir.parent().unwrap().to_path_buf()),
            );
            black_box(output);
        });
    });

    group.finish();
}

/// Benchmark 3: Git Log + Analysis
/// Target: <50ms for 100 commits
/// AI agents analyze commit history for context
fn bench_git_log_analysis(c: &mut Criterion) {
    let repo = setup_git_repo();
    let repo_path = repo.path().to_path_buf();

    let mut group = c.benchmark_group("git_log_analysis");

    // Get 100 commits as JSON
    group.bench_function("git_log_100_commits", |b| {
        b.iter(|| {
            let output = execute_command(black_box("git_log --json -n 100"), black_box(&repo_path));
            black_box(output);
        });
    });

    // Get 10 commits (common case)
    group.bench_function("git_log_10_commits", |b| {
        b.iter(|| {
            let output = execute_command(black_box("git_log --json -n 10"), black_box(&repo_path));
            black_box(output);
        });
    });

    // Git log with grep filter
    group.bench_function("git_log_with_grep", |b| {
        b.iter(|| {
            let output = execute_command(
                black_box("git_log --json -n 50 --grep \"Update\""),
                black_box(&repo_path),
            );
            black_box(output);
        });
    });

    group.finish();
}

/// Benchmark 4: JSON Query Operations
/// Target: <1ms for typical queries
/// AI agents constantly parse and query JSON data
fn bench_json_operations(c: &mut Criterion) {
    let test_files = setup_test_files();
    let json_file = test_files.path().join("data/file0.json");

    let mut group = c.benchmark_group("json_operations");

    // Simple field access
    group.bench_function("json_get_simple_field", |b| {
        b.iter(|| {
            let output = execute_command(
                &format!("json_get .name {}", json_file.display()),
                black_box(&test_files.path().to_path_buf()),
            );
            black_box(output);
        });
    });

    // Nested field access
    group.bench_function("json_get_nested_field", |b| {
        b.iter(|| {
            let output = execute_command(
                &format!("json_get .metadata.created {}", json_file.display()),
                black_box(&test_files.path().to_path_buf()),
            );
            black_box(output);
        });
    });

    // Array access
    group.bench_function("json_get_array_element", |b| {
        b.iter(|| {
            let output = execute_command(
                &format!("json_get .tags.[0] {}", json_file.display()),
                black_box(&test_files.path().to_path_buf()),
            );
            black_box(output);
        });
    });

    group.finish();
}

/// Benchmark 5: Grep Operations
/// Target: <20ms for 50 files
/// AI agents search for patterns in code
fn bench_grep_operations(c: &mut Criterion) {
    let test_files = setup_test_files();
    let data_dir = test_files.path().to_path_buf();

    let mut group = c.benchmark_group("grep_operations");

    // Search for TODO in all Rust files
    group.bench_function("grep_todo_in_rust_files", |b| {
        b.iter(|| {
            let output = execute_command(
                black_box("grep --json \"TODO\" data/*.rs"),
                black_box(&data_dir.parent().unwrap().to_path_buf()),
            );
            black_box(output);
        });
    });

    // Case-insensitive search
    group.bench_function("grep_case_insensitive", |b| {
        b.iter(|| {
            let output = execute_command(
                black_box("grep --json -i \"todo\" data/*.rs"),
                black_box(&data_dir.parent().unwrap().to_path_buf()),
            );
            black_box(output);
        });
    });

    group.finish();
}

/// Benchmark 6: Complex Pipeline
/// Target: <100ms for 50 files
/// Realistic AI agent workflow combining multiple operations
fn bench_complex_pipeline(c: &mut Criterion) {
    let repo = setup_git_repo();
    let repo_path = repo.path().to_path_buf();

    c.bench_function("complex_pipeline_git_status_to_grep", |b| {
        b.iter(|| {
            // Get unstaged files
            let status_output =
                execute_command(black_box("git_status --json"), black_box(&repo_path));
            black_box(status_output);

            // In a real pipeline, we'd parse the JSON and grep each file
            // For now, just grep the modified file
            let grep_output = execute_command(
                black_box("grep --json \"Modified\" src/file1.rs"),
                black_box(&repo_path),
            );
            black_box(grep_output);
        });
    });
}

/// Benchmark 7: Parallel Operations
/// Test performance of multiple concurrent operations
fn bench_parallel_operations(c: &mut Criterion) {
    let test_files = setup_test_files();
    let data_dir = test_files.path().to_path_buf();

    c.bench_function("parallel_find_10x", |b| {
        b.iter(|| {
            // Simulate multiple find operations happening concurrently
            for i in 0..10 {
                let pattern = format!("file{}.json", i * 100);
                let output = execute_command(
                    &format!("find --json data/ -name \"{}\"", pattern),
                    black_box(&data_dir.parent().unwrap().to_path_buf()),
                );
                black_box(output);
            }
        });
    });
}

/// Benchmark 8: Builtin Command Overhead
/// Measure the overhead of Rush's builtin system
fn bench_builtin_overhead(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path().to_path_buf();

    let mut group = c.benchmark_group("builtin_overhead");

    // pwd - minimal builtin
    group.bench_function("pwd", |b| {
        b.iter(|| {
            let output = execute_command(black_box("pwd"), black_box(&test_path));
            black_box(output);
        });
    });

    // echo - string handling
    group.bench_function("echo", |b| {
        b.iter(|| {
            let output = execute_command(
                black_box("echo hello world test benchmark"),
                black_box(&test_path),
            );
            black_box(output);
        });
    });

    group.finish();
}

/// Benchmark 9: Rapid Fire Operations
/// Test performance under high-frequency calls (AI agent polling)
fn bench_rapid_fire(c: &mut Criterion) {
    let repo = setup_git_repo();
    let repo_path = repo.path().to_path_buf();

    c.bench_function("rapid_fire_git_status_1000x", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                let output = execute_command(black_box("git_status --json"), black_box(&repo_path));
                black_box(output);
            }
        });
    });
}

/// Benchmark 10: Large File Operations
/// Test performance with larger data sets
fn bench_large_file_operations(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();

    // Create a large JSON file
    let mut large_array = Vec::new();
    for i in 0..10000 {
        large_array.push(serde_json::json!({
            "id": i,
            "name": format!("item_{}", i),
            "value": i * 100,
        }));
    }
    let large_json_file = temp_dir.path().join("large.json");
    fs::write(
        &large_json_file,
        serde_json::to_string(&large_array).unwrap(),
    )
    .unwrap();

    c.bench_function("json_query_large_file", |b| {
        b.iter(|| {
            let output = execute_command(
                &format!("json_get \".[0].name\" {}", large_json_file.display()),
                black_box(&temp_dir.path().to_path_buf()),
            );
            black_box(output);
        });
    });
}

criterion_group!(
    benches,
    bench_git_status_loop,
    bench_find_filter_json,
    bench_git_log_analysis,
    bench_json_operations,
    bench_grep_operations,
    bench_complex_pipeline,
    bench_parallel_operations,
    bench_builtin_overhead,
    bench_rapid_fire,
    bench_large_file_operations,
);

criterion_main!(benches);
