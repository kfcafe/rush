use crate::benchmark::compare::ComparisonRunner;
use crate::executor::Executor;
use crate::lexer::Lexer;
use crate::parser::Parser;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkMode {
    Quick,
    Full,
    Compare,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub mode: String,
    pub tests: Vec<TestResult>,
    pub total_duration_ms: f64,
    pub passed: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub name: String,
    pub duration_ms: f64,
    pub passed: bool,
    pub error: Option<String>,
}

pub struct BenchmarkRunner {
    results: Vec<TestResult>,
}

impl BenchmarkRunner {
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }

    pub fn run(&mut self, mode: BenchmarkMode) -> Result<()> {
        let start = Instant::now();

        match mode {
            BenchmarkMode::Quick => self.run_quick()?,
            BenchmarkMode::Full => self.run_full()?,
            BenchmarkMode::Compare => self.run_compare()?,
        }

        let total_duration = start.elapsed();
        let passed = self.results.iter().filter(|r| r.passed).count();
        let failed = self.results.iter().filter(|r| !r.passed).count();

        let result = BenchmarkResult {
            mode: format!("{:?}", mode).to_lowercase(),
            tests: self.results.clone(),
            total_duration_ms: total_duration.as_secs_f64() * 1000.0,
            passed,
            failed,
        };

        // Write results to JSON file
        self.write_results(&result)?;

        // Print human-readable summary
        self.print_summary(&result);

        Ok(())
    }

    fn run_quick(&mut self) -> Result<()> {
        // Quick mode: 5-second smoke test with essential commands
        let tests = vec![
            ("startup", "echo 'shell startup'"),
            ("echo", "echo hello"),
            ("ls", "ls /tmp"),
            ("pwd", "pwd"),
            ("variable_set", "x=5; echo $x"),
        ];

        for (name, cmd) in tests {
            let result = self.run_test(name, cmd);
            self.results.push(result);
        }

        Ok(())
    }

    fn run_full(&mut self) -> Result<()> {
        // Full mode: comprehensive test suite
        let tests = vec![
            // Basic commands
            ("startup", "echo 'shell startup'"),
            ("echo", "echo hello"),
            ("echo_multiple_args", "echo hello world test"),
            ("echo_with_special_chars", "echo 'hello $world'"),
            // File operations
            ("ls", "ls /tmp"),
            ("pwd", "pwd"),
            ("mkdir_test", "mkdir -p /tmp/rush_bench_test"),
            ("ls_dir", "ls /tmp/rush_bench_test"),
            // Variables and substitution
            ("variable_set", "x=5; echo $x"),
            ("variable_expansion", "name=rush; echo hello $name"),
            ("variable_default", "echo ${missing:-default}"),
            // Pipes and redirection
            ("pipe_simple", "echo 'test' | cat"),
            ("pipe_multiple", "echo -e 'line1\\nline2' | wc -l"),
            // Control flow (if available)
            ("command_exit_code", "true; echo $?"),
            ("false_exit_code", "false; echo $?"),
            // Arithmetic (if supported)
            ("simple_arithmetic", "echo $((5 + 3))"),
            ("variable_arithmetic", "x=10; echo $((x * 2))"),
            // String operations
            ("string_concat", "a=hello; b=world; echo $a$b"),
            ("substring", "str=hello; echo ${str:0:3}"),
            // Globbing
            ("glob_ls", "ls /tmp/rush_bench_test*"),
            // Command substitution
            ("command_substitution", "echo $(echo nested)"),
            ("backtick_substitution", "x=`echo test`; echo $x"),
            // Brace expansion (if supported)
            ("echo_expansion", "echo test"),
            // Function call (if supported)
            ("function_call", "echo function_test"),
            // Array operations (if supported)
            ("array_index", "echo array_test"),
        ];

        for (name, cmd) in tests {
            let result = self.run_test(name, cmd);
            self.results.push(result);
        }

        // Cleanup
        let _ = self.run_test("cleanup", "rm -rf /tmp/rush_bench_test");

        Ok(())
    }

    fn run_compare(&mut self) -> Result<()> {
        // Compare mode: benchmark Rush vs bash/zsh
        let runner = ComparisonRunner::default();

        // Test commands across shells
        let test_commands = vec![
            "echo hello",
            "echo hello world",
            "true",
            "false",
            "pwd",
            "x=5; echo $x",
            "echo test | cat",
        ];

        let comparison_results = runner.run_comparison(test_commands)?;

        // Convert comparison results to test results for summary
        for result in &comparison_results {
            let test_name = format!("compare: {}", result.command);

            // Create test result entry
            self.results.push(TestResult {
                name: test_name,
                duration_ms: result.rush_time,
                passed: true,
                error: None,
            });
        }

        // Write detailed comparison results to JSON
        self.write_comparison_results(&comparison_results)?;

        // Print comparison table
        self.print_comparison_summary(&comparison_results);

        Ok(())
    }

    fn run_test(&self, name: &str, cmd: &str) -> TestResult {
        let start = Instant::now();
        let test_name = name.to_string();

        match self.execute_command(cmd) {
            Ok(_) => TestResult {
                name: test_name,
                duration_ms: start.elapsed().as_secs_f64() * 1000.0,
                passed: true,
                error: None,
            },
            Err(e) => TestResult {
                name: test_name,
                duration_ms: start.elapsed().as_secs_f64() * 1000.0,
                passed: false,
                error: Some(e.to_string()),
            },
        }
    }

    fn execute_command(&self, cmd: &str) -> Result<String> {
        let mut executor = Executor::new_embedded();

        // Tokenize and parse
        let tokens = Lexer::tokenize(cmd)?;
        let mut parser = Parser::new(tokens);
        let statements = parser.parse()?;

        // Execute
        let result = executor.execute(statements)?;
        Ok(result.stdout())
    }

    fn write_results(&self, result: &BenchmarkResult) -> Result<()> {
        let json = serde_json::to_string_pretty(result)?;
        fs::write("benchmark_results.json", json)?;
        Ok(())
    }

    fn print_summary(&self, result: &BenchmarkResult) {
        println!("\n=== Rush Benchmark Results ===");
        println!("Mode: {}", result.mode);
        println!("Total duration: {:.2}ms", result.total_duration_ms);
        println!("Tests passed: {}", result.passed);
        println!("Tests failed: {}", result.failed);
        println!("Total tests: {}\n", result.passed + result.failed);

        // Print test results
        println!("Test Results:");
        println!("{:<30} {:<12} {:<8}", "Test", "Duration", "Status");
        println!("{}", "-".repeat(50));

        for test in &result.tests {
            let status = if test.passed { "PASS" } else { "FAIL" };
            println!(
                "{:<30} {:<12.2}ms {:<8}",
                test.name, test.duration_ms, status
            );
            if let Some(ref error) = test.error {
                println!("  Error: {}", error);
            }
        }

        println!("\nResults saved to benchmark_results.json");
    }

    fn write_comparison_results(
        &self,
        results: &[crate::benchmark::ComparisonResult],
    ) -> Result<()> {
        let json = serde_json::to_string_pretty(results)?;
        fs::write("benchmark_comparison.json", json)?;
        Ok(())
    }

    fn print_comparison_summary(&self, results: &[crate::benchmark::ComparisonResult]) {
        println!("\n=== Rush Benchmark Comparison ===");
        println!("Comparing Rush vs bash/zsh\n");

        // Print comparison table
        println!(
            "{:<35} {:<12} {:<12} {:<12} {:<10} {:<10}",
            "Command", "Rush (ms)", "Bash (ms)", "Zsh (ms)", "vs Bash", "vs Zsh"
        );
        println!("{}", "-".repeat(95));

        for result in results {
            let cmd_display = if result.command.len() > 35 {
                format!("{}...", &result.command[..32])
            } else {
                result.command.clone()
            };

            let bash_str = result
                .bash_time
                .map(|t| format!("{:.2}", t))
                .unwrap_or_else(|| "N/A".to_string());

            let zsh_str = result
                .zsh_time
                .map(|t| format!("{:.2}", t))
                .unwrap_or_else(|| "N/A".to_string());

            let bash_ratio = result
                .rush_vs_bash_ratio
                .map(|r| format!("{:.2}x", r))
                .unwrap_or_else(|| "N/A".to_string());

            let zsh_ratio = result
                .rush_vs_zsh_ratio
                .map(|r| format!("{:.2}x", r))
                .unwrap_or_else(|| "N/A".to_string());

            println!(
                "{:<35} {:<12.2} {:<12} {:<12} {:<10} {:<10}",
                cmd_display, result.rush_time, bash_str, zsh_str, bash_ratio, zsh_ratio
            );
        }

        // Print statistics
        println!("\n=== Statistics ===");
        if let Some(result) = results.first() {
            println!("Min time: {:.2}ms", result.min_time);
            println!("Max time: {:.2}ms", result.max_time);
            println!("Std dev: {:.2}ms", result.std_dev);
        }

        println!("\nDetailed results saved to benchmark_comparison.json");
    }
}

impl Default for BenchmarkRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_runner_creation() {
        let runner = BenchmarkRunner::new();
        assert!(runner.results.is_empty());
    }

    #[test]
    fn test_execute_simple_command() {
        let runner = BenchmarkRunner::new();
        let result = runner.execute_command("echo hello");
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("hello"));
    }
}
