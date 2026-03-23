use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

/// Implement the `abbr` builtin command.
///
/// Abbreviations work like aliases but are stored persistently in
/// `~/.config/rush/abbreviations` and are expanded before execution.
///
/// Usage:
///   abbr                    — list all abbreviations
///   abbr -a NAME EXPANSION  — add an abbreviation
///   abbr -e NAME            — erase an abbreviation
pub fn builtin_abbr(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.is_empty() {
        return list_abbreviations(runtime);
    }

    match args[0].as_str() {
        "-a" | "--add" => {
            if args.len() < 3 {
                return Err(anyhow!("abbr: usage: abbr -a NAME EXPANSION"));
            }
            let name = &args[1];
            if name.is_empty() || name.contains(char::is_whitespace) {
                return Err(anyhow!("abbr: invalid abbreviation name: '{}'", name));
            }
            let expansion = args[2..].join(" ");
            runtime.add_abbreviation(name.to_string(), expansion);
            Ok(ExecutionResult::success(String::new()))
        }
        "-e" | "--erase" => {
            if args.len() < 2 {
                return Err(anyhow!("abbr: usage: abbr -e NAME"));
            }
            let name = &args[1];
            if !runtime.remove_abbreviation(name) {
                return Err(anyhow!("abbr: {}: not found", name));
            }
            Ok(ExecutionResult::success(String::new()))
        }
        flag if flag.starts_with('-') => Err(anyhow!(
            "abbr: {}: unknown option\nUsage: abbr [-a NAME EXPANSION | -e NAME]",
            flag
        )),
        _ => {
            // No flag — treat as `abbr -a NAME EXPANSION` shorthand if two args given
            if args.len() >= 2 {
                let name = &args[0];
                if name.is_empty() || name.contains(char::is_whitespace) {
                    return Err(anyhow!("abbr: invalid abbreviation name: '{}'", name));
                }
                let expansion = args[1..].join(" ");
                runtime.add_abbreviation(name.to_string(), expansion);
                Ok(ExecutionResult::success(String::new()))
            } else {
                // Single non-flag arg: show that specific abbreviation
                let name = &args[0];
                if let Some(expansion) = runtime.get_abbreviation(name) {
                    Ok(ExecutionResult::success(format!(
                        "abbr {} {}\n",
                        name, expansion
                    )))
                } else {
                    Err(anyhow!("abbr: {}: not found", name))
                }
            }
        }
    }
}

fn list_abbreviations(runtime: &Runtime) -> Result<ExecutionResult> {
    let abbrs = runtime.get_all_abbreviations();
    if abbrs.is_empty() {
        return Ok(ExecutionResult::success(String::new()));
    }

    let mut sorted: Vec<_> = abbrs.iter().collect();
    sorted.sort_by_key(|(name, _)| *name);

    let mut output = String::new();
    for (name, expansion) in sorted {
        output.push_str(&format!("abbr {} {}\n", name, expansion));
    }
    Ok(ExecutionResult::success(output))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;

    fn make_runtime() -> Runtime {
        Runtime::new()
    }

    #[test]
    fn test_builtin_abbr_add() {
        let mut rt = make_runtime();
        let result = builtin_abbr(
            &["-a".to_string(), "gs".to_string(), "git status".to_string()],
            &mut rt,
        );
        assert!(result.is_ok());
        assert_eq!(rt.get_abbreviation("gs"), Some(&"git status".to_string()));
    }

    #[test]
    fn test_builtin_abbr_add_shorthand() {
        let mut rt = make_runtime();
        let result = builtin_abbr(&["gp".to_string(), "git push".to_string()], &mut rt);
        assert!(result.is_ok());
        assert_eq!(rt.get_abbreviation("gp"), Some(&"git push".to_string()));
    }

    #[test]
    fn test_builtin_abbr_add_multiword_expansion() {
        let mut rt = make_runtime();
        let result = builtin_abbr(
            &[
                "-a".to_string(),
                "gc".to_string(),
                "git".to_string(),
                "commit".to_string(),
                "-m".to_string(),
            ],
            &mut rt,
        );
        assert!(result.is_ok());
        assert_eq!(
            rt.get_abbreviation("gc"),
            Some(&"git commit -m".to_string())
        );
    }

    #[test]
    fn test_builtin_abbr_erase() {
        let mut rt = make_runtime();
        rt.add_abbreviation("gs".to_string(), "git status".to_string());
        let result = builtin_abbr(&["-e".to_string(), "gs".to_string()], &mut rt);
        assert!(result.is_ok());
        assert_eq!(rt.get_abbreviation("gs"), None);
    }

    #[test]
    fn test_builtin_abbr_erase_not_found() {
        let mut rt = make_runtime();
        let result = builtin_abbr(&["-e".to_string(), "nonexistent".to_string()], &mut rt);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_builtin_abbr_list_empty() {
        let mut rt = make_runtime();
        // Clear any abbreviations loaded from disk during Runtime::new()
        for name in rt
            .get_all_abbreviations()
            .keys()
            .cloned()
            .collect::<Vec<_>>()
        {
            rt.remove_abbreviation(&name);
        }
        let result = builtin_abbr(&[], &mut rt).unwrap();
        assert_eq!(result.stdout(), "");
    }

    #[test]
    fn test_builtin_abbr_list() {
        let mut rt = make_runtime();
        // Clear any preloaded abbreviations
        for name in rt
            .get_all_abbreviations()
            .keys()
            .cloned()
            .collect::<Vec<_>>()
        {
            rt.remove_abbreviation(&name);
        }
        rt.add_abbreviation("gs".to_string(), "git status".to_string());
        rt.add_abbreviation("gp".to_string(), "git push".to_string());
        let result = builtin_abbr(&[], &mut rt).unwrap();
        let out = result.stdout();
        assert!(out.contains("abbr gs git status"));
        assert!(out.contains("abbr gp git push"));
    }

    #[test]
    fn test_builtin_abbr_show_specific() {
        let mut rt = make_runtime();
        rt.add_abbreviation("gs".to_string(), "git status".to_string());
        let result = builtin_abbr(&["gs".to_string()], &mut rt).unwrap();
        assert!(result.stdout().contains("abbr gs git status"));
    }

    #[test]
    fn test_builtin_abbr_missing_args_add() {
        let mut rt = make_runtime();
        let result = builtin_abbr(&["-a".to_string(), "gs".to_string()], &mut rt);
        assert!(result.is_err());
    }

    #[test]
    fn test_builtin_abbr_missing_args_erase() {
        let mut rt = make_runtime();
        let result = builtin_abbr(&["-e".to_string()], &mut rt);
        assert!(result.is_err());
    }

    #[test]
    fn test_expand_abbreviation() {
        let mut rt = make_runtime();
        rt.add_abbreviation("gs".to_string(), "git status".to_string());

        // Expands first word
        assert_eq!(rt.expand_abbreviation("gs"), Some("git status".to_string()));
        // Expands with trailing args
        assert_eq!(
            rt.expand_abbreviation("gs --short"),
            Some("git status --short".to_string())
        );
        // Does not expand when word not registered
        assert_eq!(rt.expand_abbreviation("ls -la"), None);
    }

    #[test]
    fn test_builtin_abbr_unknown_option() {
        let mut rt = make_runtime();
        let result = builtin_abbr(&["-z".to_string()], &mut rt);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown option"));
    }
}
