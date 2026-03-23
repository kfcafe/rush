use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use std::env;
use std::path::PathBuf;

/// Change the runtime's current directory (shared logic for pushd/popd).
fn change_dir(path: PathBuf, runtime: &mut Runtime) -> Result<()> {
    let absolute = if path.is_absolute() {
        path
    } else {
        runtime.get_cwd().join(path)
    };

    if !absolute.exists() {
        return Err(anyhow!("pushd: no such file or directory: {:?}", absolute));
    }
    if !absolute.is_dir() {
        return Err(anyhow!("pushd: not a directory: {:?}", absolute));
    }

    let current_pwd = runtime.get_cwd().to_string_lossy().to_string();
    runtime.set_variable("OLDPWD".to_string(), current_pwd);
    runtime.set_cwd(absolute.clone());
    env::set_current_dir(&absolute)?;
    let new_pwd = absolute.to_string_lossy().to_string();
    runtime.set_variable("PWD".to_string(), new_pwd);
    Ok(())
}

/// Format the directory stack as a space-separated line, with current dir first.
fn format_stack(runtime: &Runtime) -> String {
    let cwd = runtime.get_cwd().to_string_lossy().to_string();
    let stack = runtime.get_dir_stack();
    // Stack is stored oldest-first; we want newest-first (top of stack) then cwd last.
    // Convention: dirs prints current-dir then stack entries newest-first.
    let mut parts = vec![cwd];
    for dir in stack.iter().rev() {
        parts.push(dir.to_string_lossy().to_string());
    }
    parts.join(" ")
}

/// `pushd [DIR]`
///
/// With DIR: push current directory onto the stack and cd to DIR.
/// Without DIR: swap the top two entries (current dir and top-of-stack).
pub fn builtin_pushd(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.is_empty() {
        // No arguments — swap current dir with top of stack
        let top = runtime
            .pop_dir()
            .ok_or_else(|| anyhow!("pushd: directory stack empty"))?;
        let current = runtime.get_cwd().to_owned();
        runtime.push_dir(current.clone());
        change_dir(top, runtime)?;
        let output = format_stack(runtime) + "\n";
        return Ok(ExecutionResult::success(output));
    }

    // Push current dir, then cd to requested dir
    let current = runtime.get_cwd().to_owned();
    let target = resolve_path(&args[0]);
    change_dir(target, runtime)?;
    runtime.push_dir(current);

    let output = format_stack(runtime) + "\n";
    Ok(ExecutionResult::success(output))
}

/// `popd`
///
/// Pop the top directory from the stack and cd to it.
pub fn builtin_popd(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    let _ = args; // popd ignores arguments (simplified)
    let top = runtime
        .pop_dir()
        .ok_or_else(|| anyhow!("popd: directory stack empty"))?;
    change_dir(top, runtime)?;
    let output = format_stack(runtime) + "\n";
    Ok(ExecutionResult::success(output))
}

/// `dirs [-c] [-v]`
///
/// Print the directory stack.
/// `-c` — clear the stack.
/// `-v` — print with index numbers.
pub fn builtin_dirs(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    let mut clear = false;
    let mut verbose = false;

    for arg in args {
        match arg.as_str() {
            "-c" => clear = true,
            "-v" => verbose = true,
            _ => return Err(anyhow!("dirs: unknown option: {}", arg)),
        }
    }

    if clear {
        runtime.clear_dir_stack();
        return Ok(ExecutionResult::success(String::new()));
    }

    let cwd = runtime.get_cwd().to_string_lossy().to_string();
    let stack = runtime.get_dir_stack();
    // Build list: index 0 = current dir, 1..N = stack newest-first
    let mut entries: Vec<String> = vec![cwd];
    for dir in stack.iter().rev() {
        entries.push(dir.to_string_lossy().to_string());
    }

    let output = if verbose {
        entries
            .iter()
            .enumerate()
            .map(|(i, d)| format!("{}\t{}", i, d))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    } else {
        entries.join(" ") + "\n"
    };

    Ok(ExecutionResult::success(output))
}

fn resolve_path(s: &str) -> PathBuf {
    if s.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            return home.join(s.trim_start_matches("~/"));
        }
    }
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;
    use std::env;

    fn make_runtime() -> Runtime {
        Runtime::new()
    }

    #[test]
    fn builtin_pushd_pushes_and_changes_dir() {
        let mut rt = make_runtime();
        let original = rt.get_cwd().to_owned();
        let tmp = env::temp_dir();

        let args = vec![tmp.to_string_lossy().to_string()];
        let result = builtin_pushd(&args, &mut rt).expect("pushd should succeed");
        assert_eq!(result.exit_code, 0);

        // Current dir is now tmp
        assert_eq!(rt.get_cwd(), tmp.as_path());
        // Stack has the original dir
        assert_eq!(rt.get_dir_stack(), &[original]);
    }

    #[test]
    fn builtin_pushd_no_args_swaps() {
        let mut rt = make_runtime();
        let original = rt.get_cwd().to_owned();
        let tmp = env::temp_dir();

        // First push tmp
        let args = vec![tmp.to_string_lossy().to_string()];
        builtin_pushd(&args, &mut rt).expect("first pushd");

        // Now pushd with no args should swap back
        let result = builtin_pushd(&[], &mut rt).expect("swap pushd");
        assert!(result.exit_code == 0);
        assert_eq!(rt.get_cwd(), original.as_path());
    }

    #[test]
    fn builtin_popd_restores_dir() {
        let mut rt = make_runtime();
        let original = rt.get_cwd().to_owned();
        let tmp = env::temp_dir();

        // Push to tmp
        let args = vec![tmp.to_string_lossy().to_string()];
        builtin_pushd(&args, &mut rt).expect("pushd");

        // Pop should restore original
        let result = builtin_popd(&[], &mut rt).expect("popd");
        assert!(result.exit_code == 0);
        assert_eq!(rt.get_cwd(), original.as_path());
        assert!(rt.get_dir_stack().is_empty());
    }

    #[test]
    fn builtin_popd_empty_stack_errors() {
        let mut rt = make_runtime();
        let result = builtin_popd(&[], &mut rt);
        assert!(result.is_err());
    }

    #[test]
    fn builtin_dirs_prints_stack() {
        let mut rt = make_runtime();
        let tmp = env::temp_dir();

        let args = vec![tmp.to_string_lossy().to_string()];
        builtin_pushd(&args, &mut rt).expect("pushd");

        let result = builtin_dirs(&[], &mut rt).expect("dirs");
        assert!(result.exit_code == 0);
        let out = result.stdout();
        assert!(out.contains(&tmp.to_string_lossy().to_string()));
    }

    #[test]
    fn builtin_dirs_clear() {
        let mut rt = make_runtime();
        let tmp = env::temp_dir();

        builtin_pushd(&[tmp.to_string_lossy().to_string()], &mut rt).expect("pushd");
        let result = builtin_dirs(&["-c".to_string()], &mut rt).expect("dirs -c");
        assert!(result.exit_code == 0);
        assert!(rt.get_dir_stack().is_empty());
    }

    #[test]
    fn builtin_dirs_verbose() {
        let mut rt = make_runtime();
        let tmp = env::temp_dir();

        builtin_pushd(&[tmp.to_string_lossy().to_string()], &mut rt).expect("pushd");
        let result = builtin_dirs(&["-v".to_string()], &mut rt).expect("dirs -v");
        assert!(result.exit_code == 0);
        let out = result.stdout();
        assert!(out.contains("0\t"));
        assert!(out.contains("1\t"));
    }
}
