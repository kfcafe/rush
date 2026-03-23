use crate::executor::{ExecutionResult, Output, ProfileData, ProfileFormatter};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::time::Instant;

/// The profile builtin command
///
/// Profiles command execution and reports timing statistics.
/// Supports both human-readable and JSON output formats.
///
/// Usage:
///   profile [-json] { command | pipeline }
///   profile [-json] command args...
///
/// Examples:
///   profile { ls | grep foo }
///   profile -json echo hello
///   profile -json { find . -type f | wc -l }
///
/// The -json flag outputs machine-readable timing data.
pub fn builtin_profile(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.is_empty() {
        return Err(anyhow!("profile: usage: profile [-json] command [args...]"));
    }

    // Check for -json flag
    let (json_output, command_args) = if args[0] == "-json" {
        if args.len() < 2 {
            return Err(anyhow!("profile: -json requires a command"));
        }
        (true, &args[1..])
    } else {
        (false, args)
    };

    // Reconstruct the command string from all remaining arguments
    let command_string = command_args.join(" ");

    // Create profiling data
    let mut profile_data = ProfileData::new();
    profile_data.start_total();

    // Parse and execute the command string
    use crate::executor::Executor;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    // Tokenize the command string
    let tokens =
        Lexer::tokenize(&command_string).map_err(|e| anyhow!("profile: tokenize error: {}", e))?;

    // Parse the tokens into statements
    let mut parser = Parser::new(tokens);
    let statements = parser
        .parse()
        .map_err(|e| anyhow!("profile: parse error: {}", e))?;

    // Create a new executor with profiling enabled
    let mut executor = Executor::new_embedded().with_profiling(true);

    // Copy the current runtime state
    *executor.runtime_mut() = runtime.clone();

    // Record the execution time
    let _execution_start = Instant::now();

    // Execute the parsed statements
    let result = executor
        .execute(statements)
        .map_err(|e| anyhow!("profile: execution error: {}", e))?;

    // Get profile data from executor if available
    let executor_profile = executor
        .profile_data
        .as_ref()
        .cloned()
        .unwrap_or_else(ProfileData::new);

    // Copy back the runtime state to preserve variable changes
    *runtime = executor.runtime_mut().clone();

    // Format and output profiling results
    let profile_output = if json_output {
        // Return JSON output with command result
        let json_profile = ProfileFormatter::format_json(&executor_profile);
        let json_output = serde_json::json!({
            "profile": json_profile,
            "command": command_string,
            "exit_code": result.exit_code,
        });
        json_output.to_string()
    } else {
        // Return human-readable format with command output
        let mut output = String::new();

        // Print the command being profiled
        output.push('\n');
        output.push_str("Profiling: ");
        output.push_str(&command_string);
        output.push('\n');

        // Add profiling output
        output.push_str(&ProfileFormatter::format(&executor_profile));

        output
    };

    // Combine the original command output with profiling report
    let final_output = if json_output {
        // For JSON output, we'll put the profile info as the primary output
        format!("{}\n{}", result.stdout(), profile_output)
    } else {
        // For human-readable, append profiling info to command output
        format!("{}{}", result.stdout(), profile_output)
    };

    Ok(ExecutionResult {
        output: Output::Text(final_output),
        stderr: result.stderr,
        exit_code: result.exit_code,
        error: result.error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_simple_command() {
        let mut runtime = Runtime::new();
        let args = vec!["echo".to_string(), "hello".to_string()];
        let result = builtin_profile(&args, &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(result.stdout().contains("hello"));
        assert!(result.stdout().contains("Profiling"));
    }

    #[test]
    fn test_profile_json_output() {
        let mut runtime = Runtime::new();
        let args = vec!["-json".to_string(), "echo".to_string(), "test".to_string()];
        let result = builtin_profile(&args, &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(result.stdout().contains("test"));
        assert!(result.stdout().contains("profile"));
        assert!(result.stdout().contains("total_ms"));
    }

    #[test]
    fn test_profile_with_pipe() {
        let mut runtime = Runtime::new();
        let args = vec![
            "echo".to_string(),
            "hello".to_string(),
            "|".to_string(),
            "cat".to_string(),
        ];
        let result = builtin_profile(&args, &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(result.stdout().contains("hello"));
    }

    #[test]
    fn test_profile_no_args() {
        let mut runtime = Runtime::new();
        let result = builtin_profile(&[], &mut runtime);
        assert!(result.is_err());
    }

    #[test]
    fn test_profile_json_no_command() {
        let mut runtime = Runtime::new();
        let args = vec!["-json".to_string()];
        let result = builtin_profile(&args, &mut runtime);
        assert!(result.is_err());
    }

    #[test]
    fn test_profile_preserves_variables() {
        let mut runtime = Runtime::new();
        runtime.set_variable("TEST_VAR".to_string(), "before".to_string());

        let args = vec!["echo".to_string(), "test".to_string()];
        let _ = builtin_profile(&args, &mut runtime);

        // Verify the variable is still available
        assert!(runtime.get_variable("TEST_VAR").is_some());
    }

    #[test]
    fn test_profile_exit_code_preserved() {
        let mut runtime = Runtime::new();
        let args = vec!["false".to_string()];
        let result = builtin_profile(&args, &mut runtime).unwrap();

        assert_eq!(result.exit_code, 1);
    }
}
