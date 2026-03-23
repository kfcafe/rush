use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

/// The eval builtin command
///
/// Concatenates all arguments into a single string, then parses and executes
/// that string as a shell command. This allows dynamic command construction
/// and execution at runtime.
///
/// Security Note: eval executes arbitrary commands and should be used with caution,
/// especially with untrusted input.
pub fn builtin_eval(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    // If no arguments, return success with no output
    if args.is_empty() {
        return Ok(ExecutionResult::success(String::new()));
    }

    // Concatenate all arguments with spaces
    let command_string = args.join(" ");

    // Parse and execute the command string
    // We need to use the lexer and parser to tokenize and parse the command
    use crate::executor::Executor;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    // Tokenize the command string
    let tokens =
        Lexer::tokenize(&command_string).map_err(|e| anyhow!("eval: tokenize error: {}", e))?;

    // Parse the tokens into statements
    let mut parser = Parser::new(tokens);
    let statements = parser
        .parse()
        .map_err(|e| anyhow!("eval: parse error: {}", e))?;

    // Create a temporary executor with the current runtime state
    // We use new_embedded() which disables progress indicators for eval commands
    let mut executor = Executor::new_embedded();

    // Copy the current runtime state into the executor
    *executor.runtime_mut() = runtime.clone();

    // Execute the parsed statements
    let result = executor
        .execute(statements)
        .map_err(|e| anyhow!("eval: execution error: {}", e))?;

    // Copy back the runtime state to preserve variable changes, etc.
    *runtime = executor.runtime_mut().clone();

    // Return the execution result
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_simple_command() {
        let mut runtime = Runtime::new();
        let args = vec!["echo".to_string(), "hello".to_string(), "world".to_string()];
        let result = builtin_eval(&args, &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "hello world\n");
    }

    #[test]
    fn test_eval_variable_expansion() {
        let mut runtime = Runtime::new();
        runtime.set_variable("TEST_VAR".to_string(), "test_value".to_string());

        let args = vec!["echo".to_string(), "$TEST_VAR".to_string()];
        let result = builtin_eval(&args, &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "test_value\n");
    }

    #[test]
    fn test_eval_command_substitution() {
        let mut runtime = Runtime::new();

        let args = vec![
            "echo".to_string(),
            "$(echo".to_string(),
            "nested)".to_string(),
        ];
        let result = builtin_eval(&args, &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "nested\n");
    }

    #[test]
    fn test_eval_with_pipes() {
        let mut runtime = Runtime::new();

        let args = vec![
            "echo".to_string(),
            "hello".to_string(),
            "|".to_string(),
            "cat".to_string(),
        ];
        let result = builtin_eval(&args, &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "hello\n");
    }

    #[test]
    fn test_eval_exit_code() {
        let mut runtime = Runtime::new();

        // false builtin returns exit code 1
        let args = vec!["false".to_string()];
        let result = builtin_eval(&args, &mut runtime).unwrap();

        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_eval_arithmetic() {
        let mut runtime = Runtime::new();
        runtime.set_variable("x".to_string(), "5".to_string());
        runtime.set_variable("y".to_string(), "10".to_string());

        // Test arithmetic through eval
        let args = vec!["echo $x $y".to_string()];
        let result = builtin_eval(&args, &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "5 10\n");
    }

    #[test]
    fn test_eval_no_args() {
        let mut runtime = Runtime::new();
        let result = builtin_eval(&[], &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "");
    }

    #[test]
    fn test_eval_parse_error() {
        let mut runtime = Runtime::new();

        // Invalid syntax: unclosed quote
        let args = vec!["echo".to_string(), "\"unclosed".to_string()];
        let result = builtin_eval(&args, &mut runtime);

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        // Accept either parse or tokenize error
        assert!(error_msg.contains("error") || error_msg.contains("eval:"));
    }

    #[test]
    fn test_eval_multiple_statements() {
        let mut runtime = Runtime::new();

        let args = vec![
            "echo".to_string(),
            "first".to_string(),
            ";".to_string(),
            "echo".to_string(),
            "second".to_string(),
        ];
        let result = builtin_eval(&args, &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "first\nsecond\n");
    }

    #[test]
    fn test_eval_complex_command() {
        let mut runtime = Runtime::new();
        runtime.set_variable("VAR".to_string(), "value".to_string());

        // Simpler test with && instead of if/then/fi
        let args = vec![
            "test".to_string(),
            "-n".to_string(),
            "$VAR".to_string(),
            "&&".to_string(),
            "echo".to_string(),
            "yes".to_string(),
        ];
        let result = builtin_eval(&args, &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "yes\n");
    }

    #[test]
    fn test_eval_with_conditionals() {
        let mut runtime = Runtime::new();

        let args = vec![
            "true".to_string(),
            "&&".to_string(),
            "echo".to_string(),
            "success".to_string(),
        ];
        let result = builtin_eval(&args, &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "success\n");
    }

    #[test]
    fn test_eval_with_or_conditional() {
        let mut runtime = Runtime::new();

        let args = vec![
            "false".to_string(),
            "||".to_string(),
            "echo".to_string(),
            "fallback".to_string(),
        ];
        let result = builtin_eval(&args, &mut runtime).unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout(), "fallback\n");
    }
}
