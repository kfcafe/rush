use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

/// Implement the `alias` builtin command
///
/// Usage:
/// - `alias` - List all aliases
/// - `alias name=value` - Create an alias
pub fn builtin_alias(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    // No args: list all aliases
    if args.is_empty() {
        return list_aliases(runtime);
    }

    // Process each argument as an alias definition
    for arg in args {
        if let Some((name, value)) = arg.split_once('=') {
            // Validate alias name (no spaces, valid identifier)
            if name.is_empty() || name.contains(char::is_whitespace) {
                return Err(anyhow!("alias: invalid alias name: '{}'", name));
            }

            // Set the alias
            runtime.set_alias(name.to_string(), value.to_string());
        } else {
            // If no '=' is present, show the specific alias
            if let Some(value) = runtime.get_alias(arg) {
                return Ok(ExecutionResult::success(format!(
                    "alias {}='{}'\n",
                    arg, value
                )));
            } else {
                return Err(anyhow!("alias: {}: not found", arg));
            }
        }
    }

    Ok(ExecutionResult::success(String::new()))
}

/// Implement the `unalias` builtin command
///
/// Usage:
/// - `unalias name` - Remove an alias
/// - `unalias -a` - Remove all aliases
pub fn builtin_unalias(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.is_empty() {
        return Err(anyhow!("unalias: usage: unalias [-a] name [name ...]"));
    }

    // Check for -a flag (remove all aliases)
    if args.len() == 1 && args[0] == "-a" {
        // Clear all aliases
        let count = runtime.get_all_aliases().len();
        runtime
            .get_all_aliases()
            .keys()
            .cloned()
            .collect::<Vec<_>>()
            .iter()
            .for_each(|name| {
                runtime.remove_alias(name);
            });
        return Ok(ExecutionResult::success(format!(
            "Removed {} alias(es)\n",
            count
        )));
    }

    // Remove specified aliases
    for name in args {
        if name.starts_with('-') && name != "-a" {
            return Err(anyhow!("unalias: {}: invalid option", name));
        }

        if !runtime.remove_alias(name) {
            return Err(anyhow!("unalias: {}: not found", name));
        }
    }

    Ok(ExecutionResult::success(String::new()))
}

/// List all aliases in alphabetical order
fn list_aliases(runtime: &Runtime) -> Result<ExecutionResult> {
    let aliases = runtime.get_all_aliases();

    if aliases.is_empty() {
        return Ok(ExecutionResult::success(String::new()));
    }

    let mut output = String::new();
    let mut sorted_aliases: Vec<_> = aliases.iter().collect();
    sorted_aliases.sort_by_key(|(name, _)| *name);

    for (name, value) in sorted_aliases {
        output.push_str(&format!("alias {}='{}'\n", name, value));
    }

    Ok(ExecutionResult::success(output))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;

    #[test]
    fn test_alias_create() {
        let mut runtime = Runtime::new();

        // Create an alias
        let result = builtin_alias(&["ll=ls -la".to_string()], &mut runtime);
        assert!(result.is_ok());
        assert_eq!(runtime.get_alias("ll"), Some(&"ls -la".to_string()));
    }

    #[test]
    fn test_alias_list_empty() {
        let mut runtime = Runtime::new();

        let result = builtin_alias(&[], &mut runtime).unwrap();
        assert_eq!(result.stdout(), "");
    }

    #[test]
    fn test_alias_list_all() {
        let mut runtime = Runtime::new();
        runtime.set_alias("ll".to_string(), "ls -la".to_string());
        runtime.set_alias("la".to_string(), "ls -a".to_string());

        let result = builtin_alias(&[], &mut runtime).unwrap();
        assert!(result.stdout().contains("alias la='ls -a'"));
        assert!(result.stdout().contains("alias ll='ls -la'"));
    }

    #[test]
    fn test_alias_show_specific() {
        let mut runtime = Runtime::new();
        runtime.set_alias("ll".to_string(), "ls -la".to_string());

        let result = builtin_alias(&["ll".to_string()], &mut runtime).unwrap();
        assert_eq!(result.stdout(), "alias ll='ls -la'\n");
    }

    #[test]
    fn test_alias_not_found() {
        let mut runtime = Runtime::new();

        let result = builtin_alias(&["nonexistent".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_alias_invalid_name() {
        let mut runtime = Runtime::new();

        let result = builtin_alias(&["invalid name=value".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("invalid alias name"));
    }

    #[test]
    fn test_unalias_remove() {
        let mut runtime = Runtime::new();
        runtime.set_alias("ll".to_string(), "ls -la".to_string());

        let result = builtin_unalias(&["ll".to_string()], &mut runtime);
        assert!(result.is_ok());
        assert_eq!(runtime.get_alias("ll"), None);
    }

    #[test]
    fn test_unalias_not_found() {
        let mut runtime = Runtime::new();

        let result = builtin_unalias(&["nonexistent".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_unalias_all() {
        let mut runtime = Runtime::new();
        runtime.set_alias("ll".to_string(), "ls -la".to_string());
        runtime.set_alias("la".to_string(), "ls -a".to_string());

        let result = builtin_unalias(&["-a".to_string()], &mut runtime);
        assert!(result.is_ok());
        assert_eq!(runtime.get_all_aliases().len(), 0);
    }

    #[test]
    fn test_unalias_no_args() {
        let mut runtime = Runtime::new();

        let result = builtin_unalias(&[], &mut runtime);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("usage"));
    }

    #[test]
    fn test_unalias_invalid_option() {
        let mut runtime = Runtime::new();

        let result = builtin_unalias(&["-x".to_string()], &mut runtime);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid option"));
    }

    #[test]
    fn test_multiple_aliases() {
        let mut runtime = Runtime::new();

        // Create multiple aliases at once
        let result = builtin_alias(
            &[
                "ll=ls -la".to_string(),
                "la=ls -a".to_string(),
                "l=ls -l".to_string(),
            ],
            &mut runtime,
        );
        assert!(result.is_ok());
        assert_eq!(runtime.get_alias("ll"), Some(&"ls -la".to_string()));
        assert_eq!(runtime.get_alias("la"), Some(&"ls -a".to_string()));
        assert_eq!(runtime.get_alias("l"), Some(&"ls -l".to_string()));
    }
}
