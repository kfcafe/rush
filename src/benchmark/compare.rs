use crate::executor::Executor;
use crate::lexer::Lexer;
use crate::parser::Parser;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellTimings {
    pub shell: String,
    pub duration_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonResult {
    pub command: String,
    pub rush_time: f64,
    pub bash_time: Option<f64>,
    pub zsh_time: Option<f64>,
    pub rush_vs_bash_ratio: Option<f64>,
    pub rush_vs_zsh_ratio: Option<f64>,
    pub min_time: f64,
    pub max_time: f64,
    pub std_dev: f64,
}

pub struct ComparisonRunner {
    warmup_runs: usize,
    test_runs: usize,
}

impl ComparisonRunner {
    pub fn new(warmup_runs: usize, test_runs: usize) -> Self {
        Self {
            warmup_runs,
            test_runs,
        }
    }

    /// Run comparison benchmarks for a set of test commands
    pub fn run_comparison(&self, test_commands: Vec<&str>) -> Result<Vec<ComparisonResult>> {
        let mut results = Vec::new();

        for cmd in test_commands {
            let result = self.compare_command(cmd)?;
            results.push(result);
        }

        Ok(results)
    }

    /// Compare a single command across shells
    fn compare_command(&self, cmd: &str) -> Result<ComparisonResult> {
        // Warmup runs to eliminate cold-start bias
        for _ in 0..self.warmup_runs {
            let _ = self.time_rush(cmd);
            let _ = self.time_bash(cmd);
            let _ = self.time_zsh(cmd);
        }

        // Actual test runs
        let mut rush_times = Vec::new();
        let mut bash_times = Vec::new();
        let mut zsh_times = Vec::new();

        for _ in 0..self.test_runs {
            if let Ok(t) = self.time_rush(cmd) {
                rush_times.push(t);
            }
            if let Ok(t) = self.time_bash(cmd) {
                bash_times.push(t);
            }
            if let Ok(t) = self.time_zsh(cmd) {
                zsh_times.push(t);
            }
        }

        // Calculate statistics
        let rush_time = Self::mean(&rush_times);
        let bash_time = if !bash_times.is_empty() {
            Some(Self::mean(&bash_times))
        } else {
            None
        };
        let zsh_time = if !zsh_times.is_empty() {
            Some(Self::mean(&zsh_times))
        } else {
            None
        };

        // Calculate speedup ratios
        let rush_vs_bash_ratio = bash_time.map(|b| b / rush_time);
        let rush_vs_zsh_ratio = zsh_time.map(|z| z / rush_time);

        // Collect all times for min/max/stddev
        let all_times = [&rush_times[..], &bash_times[..], &zsh_times[..]].concat();

        let min_time = all_times.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_time = all_times.iter().cloned().fold(0.0, f64::max);
        let std_dev = Self::std_dev(&all_times);

        Ok(ComparisonResult {
            command: cmd.to_string(),
            rush_time,
            bash_time,
            zsh_time,
            rush_vs_bash_ratio,
            rush_vs_zsh_ratio,
            min_time,
            max_time,
            std_dev,
        })
    }

    /// Time command execution in Rush
    fn time_rush(&self, cmd: &str) -> Result<f64> {
        let start = Instant::now();

        let mut executor = Executor::new_embedded();
        let tokens = Lexer::tokenize(cmd)?;
        let mut parser = Parser::new(tokens);
        let statements = parser.parse()?;
        let _ = executor.execute(statements)?;

        Ok(start.elapsed().as_secs_f64() * 1000.0)
    }

    /// Time command execution in bash
    fn time_bash(&self, cmd: &str) -> Result<f64> {
        let start = Instant::now();

        let output = Command::new("bash")
            .arg("-c")
            .arg(cmd)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("bash command failed"));
        }

        Ok(start.elapsed().as_secs_f64() * 1000.0)
    }

    /// Time command execution in zsh
    fn time_zsh(&self, cmd: &str) -> Result<f64> {
        // Check if zsh is available
        match Command::new("zsh").arg("-c").arg("echo test").output() {
            Ok(_) => {
                let start = Instant::now();

                let output = Command::new("zsh")
                    .arg("-c")
                    .arg(cmd)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .output()?;

                if !output.status.success() {
                    return Err(anyhow::anyhow!("zsh command failed"));
                }

                Ok(start.elapsed().as_secs_f64() * 1000.0)
            }
            Err(_) => {
                // zsh not available, skip
                Err(anyhow::anyhow!("zsh not available"))
            }
        }
    }

    /// Calculate mean of values
    fn mean(values: &[f64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        values.iter().sum::<f64>() / values.len() as f64
    }

    /// Calculate standard deviation
    fn std_dev(values: &[f64]) -> f64 {
        if values.len() < 2 {
            return 0.0;
        }

        let mean = Self::mean(values);
        let variance =
            values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (values.len() as f64 - 1.0);

        variance.sqrt()
    }
}

impl Default for ComparisonRunner {
    fn default() -> Self {
        Self::new(3, 5) // 3 warmup runs, 5 test runs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comparison_runner_creation() {
        let runner = ComparisonRunner::new(2, 3);
        assert_eq!(runner.warmup_runs, 2);
        assert_eq!(runner.test_runs, 3);
    }

    #[test]
    fn test_comparison_runner_default() {
        let runner = ComparisonRunner::default();
        assert_eq!(runner.warmup_runs, 3);
        assert_eq!(runner.test_runs, 5);
    }

    #[test]
    fn test_mean_calculation() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(ComparisonRunner::mean(&values), 3.0);
    }

    #[test]
    fn test_mean_empty() {
        let values: Vec<f64> = vec![];
        assert_eq!(ComparisonRunner::mean(&values), 0.0);
    }

    #[test]
    fn test_std_dev_calculation() {
        let values = vec![1.0, 2.0, 3.0];
        let std = ComparisonRunner::std_dev(&values);
        assert!(std > 0.0);
        assert!((std - 1.0).abs() < 0.001); // Should be approximately 1.0
    }

    #[test]
    fn test_time_rush_simple() {
        let runner = ComparisonRunner::default();
        let result = runner.time_rush("echo hello");
        assert!(result.is_ok());
        assert!(result.unwrap() > 0.0);
    }

    #[test]
    fn test_time_bash_simple() {
        let runner = ComparisonRunner::default();
        let result = runner.time_bash("echo hello");
        assert!(result.is_ok());
        assert!(result.unwrap() > 0.0);
    }

    #[test]
    fn test_compare_single_command() {
        let runner = ComparisonRunner::new(1, 1);
        let result = runner.compare_command("echo test");
        assert!(result.is_ok());

        let comparison = result.unwrap();
        assert_eq!(comparison.command, "echo test");
        assert!(comparison.rush_time > 0.0);
        assert!(comparison.bash_time.is_some());
        assert!(comparison.bash_time.unwrap() > 0.0);
    }

    #[test]
    fn test_comparison_ratios() {
        let runner = ComparisonRunner::new(1, 1);
        let result = runner.compare_command("true");
        assert!(result.is_ok());

        let comparison = result.unwrap();
        if let Some(ratio) = comparison.rush_vs_bash_ratio {
            assert!(ratio > 0.0);
        }
    }
}
