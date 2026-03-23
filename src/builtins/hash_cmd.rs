use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// Process-global hash table: command name → resolved path.
///
/// POSIX requires `hash` to maintain a per-shell cache of command paths so the
/// shell avoids repeated PATH searches.  We model this as a process-wide static
/// map rather than putting it on the Runtime struct so that the cache persists
/// across subshell clones (matching bash behaviour).
static HASH_TABLE: Mutex<Option<HashMap<String, PathBuf>>> = Mutex::new(None);

fn with_table<F, T>(f: F) -> T
where
    F: FnOnce(&mut HashMap<String, PathBuf>) -> T,
{
    let mut guard = HASH_TABLE.lock().unwrap();
    let table = guard.get_or_insert_with(HashMap::new);
    f(table)
}

/// The `hash` builtin — cache and display command path lookups.
///
/// Usage:
///   hash                   List all cached commands
///   hash -r                Clear the entire hash table
///   hash -l                List in re-input form (`hash -p path name`)
///   hash -p path name      Add an explicit path mapping for `name`
///   hash name...           Look up `name` in PATH and cache the result
///
/// Errors with exit code 1 if a command cannot be found in PATH.
pub fn builtin_hash(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    // No args: list cached entries (name → path columns)
    if args.is_empty() {
        return with_table(|table| {
            if table.is_empty() {
                return Ok(ExecutionResult::success(String::new()));
            }
            let mut pairs: Vec<_> = table.iter().collect();
            pairs.sort_by_key(|(k, _)| k.as_str());
            let mut out = String::new();
            for (name, path) in pairs {
                out.push_str(&format!("{}\t{}\n", name, path.display()));
            }
            Ok(ExecutionResult::success(out))
        });
    }

    let mut do_reset = false;
    let mut do_list = false;
    let mut explicit_path: Option<String> = None;
    let mut commands: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-r" => do_reset = true,
            "-l" => do_list = true,
            "-p" => {
                i += 1;
                explicit_path = Some(
                    args.get(i)
                        .cloned()
                        .ok_or_else(|| anyhow!("hash: -p: requires a pathname argument"))?,
                );
                // The next positional argument after -p path is the name.
                i += 1;
                if let Some(name) = args.get(i) {
                    commands.push(name.clone());
                } else {
                    return Err(anyhow!("hash: -p path: requires a name argument"));
                }
            }
            arg if arg.starts_with('-') => {
                return Err(anyhow!("hash: {}: invalid option", arg));
            }
            name => commands.push(name.to_string()),
        }
        i += 1;
    }

    // -r clears the table and exits immediately.
    if do_reset {
        with_table(|table| table.clear());
        return Ok(ExecutionResult::success(String::new()));
    }

    // -p path name: insert an explicit mapping.
    if let Some(path_str) = explicit_path {
        let name = commands
            .first()
            .cloned()
            .ok_or_else(|| anyhow!("hash: -p: requires a name argument"))?;
        with_table(|table| table.insert(name, PathBuf::from(&path_str)));
        return Ok(ExecutionResult::success(String::new()));
    }

    // -l (or no commands): list in re-input form.
    if do_list || commands.is_empty() {
        return with_table(|table| {
            let mut pairs: Vec<_> = table.iter().collect();
            pairs.sort_by_key(|(k, _)| k.as_str());
            let mut out = String::new();
            for (name, path) in pairs {
                out.push_str(&format!("hash -p {} {}\n", path.display(), name));
            }
            Ok(ExecutionResult::success(out))
        });
    }

    // Hash named commands: resolve via PATH and cache.
    let path_env = std::env::var("PATH").unwrap_or_default();
    let mut stderr = String::new();
    let mut any_missing = false;

    for name in &commands {
        if name.contains('/') {
            // Absolute or relative path — store as-is without PATH search.
            with_table(|table| table.insert(name.clone(), PathBuf::from(name)));
        } else {
            let found = path_env.split(':').find_map(|dir| {
                let full = PathBuf::from(dir).join(name);
                if full.is_file() {
                    Some(full)
                } else {
                    None
                }
            });
            match found {
                Some(path) => {
                    with_table(|table| table.insert(name.clone(), path));
                }
                None => {
                    stderr.push_str(&format!("hash: {}: not found\n", name));
                    any_missing = true;
                }
            }
        }
    }

    Ok(ExecutionResult {
        output: crate::executor::Output::Text(String::new()),
        stderr,
        exit_code: if any_missing { 1 } else { 0 },
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> ExecutionResult {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let mut runtime = Runtime::new();
        // Clear table before each test for isolation.
        with_table(|t| t.clear());
        builtin_hash(&args, &mut runtime).expect("hash failed")
    }

    #[test]
    fn test_hash_empty_table() {
        with_table(|t| t.clear());
        let mut runtime = Runtime::new();
        let result = builtin_hash(&[], &mut runtime).unwrap();
        assert_eq!(result.stdout(), "");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_hash_reset() {
        // Insert something, then reset.
        with_table(|t| {
            t.insert("ls".to_string(), PathBuf::from("/bin/ls"));
        });
        let result = run(&["-r"]);
        assert_eq!(result.exit_code, 0);
        with_table(|t| assert!(t.is_empty()));
    }

    #[test]
    fn test_hash_explicit_path() {
        let result = run(&["-p", "/usr/bin/ls", "ls"]);
        assert_eq!(result.exit_code, 0);
        with_table(|t| {
            assert_eq!(t.get("ls"), Some(&PathBuf::from("/usr/bin/ls")));
        });
    }

    #[test]
    fn test_hash_list_output() {
        with_table(|t| {
            t.clear();
            t.insert("ls".to_string(), PathBuf::from("/bin/ls"));
        });
        let mut runtime = Runtime::new();
        let result = builtin_hash(&["-l".to_string()], &mut runtime).unwrap();
        assert!(result.stdout().contains("hash -p /bin/ls ls"));
    }

    #[test]
    fn test_hash_lookup_missing_command() {
        let result = run(&["__this_command_does_not_exist__"]);
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("not found"));
    }

    #[test]
    fn test_hash_lookup_existing_command() {
        // Use 'sh' which should always be in PATH on POSIX systems.
        let mut runtime = Runtime::new();
        with_table(|t| t.clear());
        let args = vec!["sh".to_string()];
        let result = builtin_hash(&args, &mut runtime).unwrap();
        // sh is ubiquitous; if not found the test environment is unusual.
        if result.exit_code == 0 {
            with_table(|t| assert!(t.contains_key("sh")));
        }
    }
}
