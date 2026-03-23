pub mod compare;
pub mod runner;

pub use compare::ComparisonResult;
pub use runner::BenchmarkMode;
pub use runner::BenchmarkRunner;

/// Run benchmark based on mode
pub fn run_benchmark(mode: BenchmarkMode) -> anyhow::Result<()> {
    let mut runner = BenchmarkRunner::new();
    runner.run(mode)
}
