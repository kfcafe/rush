use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

/// Implement the `declare`/`typeset` builtin command
///
/// Usage:
/// - `declare VAR=value`      — declare and assign a variable
/// - `declare VAR`            — declare variable (empty if not set)
/// - `declare -i VAR`         — declare with integer attribute
/// - `declare -r VAR[=val]`   — declare as readonly
/// - `declare -x VAR[=val]`   — declare and export to environment
/// - `declare -a VAR`         — declare as indexed array (treated as string in this shell)
/// - `declare -A VAR`         — declare as associative array (treated as string in this shell)
/// - `declare -p [VAR...]`    — print variable declarations
/// - `declare`                — list all shell variables
/// - `typeset`                — alias for `declare`
pub fn builtin_declare(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    // No arguments: list all shell variables
    if args.is_empty() {
        return list_all_variables(runtime);
    }

    // Parse flags and remaining arguments
    let mut flags = Flags::default();
    let mut var_args: Vec<&str> = Vec::new();
    let mut parsing_flags = true;

    for arg in args {
        if parsing_flags && arg.starts_with('-') && arg.len() > 1 && arg != "--" {
            parse_flags(arg, &mut flags)?;
        } else if arg == "--" {
            parsing_flags = false;
        } else {
            parsing_flags = false;
            var_args.push(arg.as_str());
        }
    }

    // -p with no variable names: print all variables
    if flags.print && var_args.is_empty() {
        return list_all_variables(runtime);
    }

    // -p with variable names: print those specific variables
    if flags.print {
        return print_declarations(&var_args, runtime);
    }

    // No variable arguments with flags (e.g., `declare -x` alone): list filtered variables
    if var_args.is_empty() {
        return list_all_variables(runtime);
    }

    // Process each variable argument
    for var_arg in var_args {
        if let Some(eq_pos) = var_arg.find('=') {
            let name = &var_arg[..eq_pos];
            let value = &var_arg[eq_pos + 1..];

            validate_var_name(name)?;

            if flags.readonly && runtime.is_readonly(name) {
                return Err(anyhow!("declare: {}: readonly variable", name));
            }

            runtime.set_variable(name.to_string(), value.to_string());
            apply_flags(name, &flags, runtime);
        } else {
            let name = var_arg;
            validate_var_name(name)?;

            if flags.readonly && runtime.is_readonly(name) {
                return Err(anyhow!("declare: {}: readonly variable", name));
            }

            // Declare without assignment: initialize to empty string if not set
            if runtime.get_variable(name).is_none() {
                runtime.set_variable(name.to_string(), String::new());
            }

            apply_flags(name, &flags, runtime);
        }
    }

    Ok(ExecutionResult::success(String::new()))
}

/// Flags parsed from declare's option arguments
#[derive(Default)]
struct Flags {
    integer: bool,  // -i
    readonly: bool, // -r
    export: bool,   // -x
    array: bool,    // -a
    assoc: bool,    // -A
    print: bool,    // -p
}

fn parse_flags(arg: &str, flags: &mut Flags) -> Result<()> {
    // arg starts with '-', e.g. "-rx" or "-p"
    for ch in arg.chars().skip(1) {
        match ch {
            'i' => flags.integer = true,
            'r' => flags.readonly = true,
            'x' => flags.export = true,
            'a' => flags.array = true,
            'A' => flags.assoc = true,
            'p' => flags.print = true,
            // -g (global), -l (lowercase), -u (uppercase), -t (trace), -n (nameref):
            // silently accept common bash declare flags we don't fully implement
            'g' | 'l' | 'u' | 't' | 'n' | 'f' | 'F' => {}
            _ => return Err(anyhow!("declare: -{}: invalid option", ch)),
        }
    }
    Ok(())
}

/// Apply the parsed attribute flags to a variable that has already been set/declared
fn apply_flags(name: &str, flags: &Flags, runtime: &mut Runtime) {
    if flags.readonly {
        runtime.mark_readonly(name.to_string());
    }
    if flags.export {
        if let Some(value) = runtime.get_variable(name) {
            runtime.set_env(name, &value);
        } else {
            runtime.set_env(name, "");
        }
    }
    // -i, -a, -A: we accept the flags for compatibility but don't enforce
    // type constraints since this shell uses string storage for all variables.
    let _ = flags.integer;
    let _ = flags.array;
    let _ = flags.assoc;
}

/// List all shell variables in `declare [-flags] NAME='value'` format
fn list_all_variables(runtime: &Runtime) -> Result<ExecutionResult> {
    let all_vars = runtime.get_all_variables();
    let readonly_vars = runtime.get_readonly_vars();
    let readonly_set: std::collections::HashSet<&str> =
        readonly_vars.iter().map(|s| s.as_str()).collect();

    let mut output = String::new();
    for (name, value) in &all_vars {
        // Skip internal positional and special variables
        if is_special_var(name) {
            continue;
        }
        let flag_str = build_flag_str(name, &readonly_set, runtime);
        output.push_str(&format!(
            "declare{} {}='{}'\n",
            flag_str,
            name,
            shell_quote(value)
        ));
    }

    Ok(ExecutionResult::success(output))
}

/// Print declarations for specific named variables
fn print_declarations(var_names: &[&str], runtime: &Runtime) -> Result<ExecutionResult> {
    let readonly_vars = runtime.get_readonly_vars();
    let readonly_set: std::collections::HashSet<&str> =
        readonly_vars.iter().map(|s| s.as_str()).collect();

    let mut output = String::new();
    let mut exit_code = 0;

    for &name in var_names {
        if let Some(value) = runtime.get_variable(name) {
            let flag_str = build_flag_str(name, &readonly_set, runtime);
            output.push_str(&format!(
                "declare{} {}='{}'\n",
                flag_str,
                name,
                shell_quote(&value)
            ));
        } else {
            eprintln!("declare: {}: not found", name);
            exit_code = 1;
        }
    }

    if exit_code == 0 {
        Ok(ExecutionResult::success(output))
    } else {
        Ok(ExecutionResult {
            output: Output::Text(output),
            stderr: String::new(),
            exit_code,
            error: None,
        })
    }
}

/// Build the flag string for a variable (e.g., " -rx" for readonly+exported)
fn build_flag_str(
    name: &str,
    readonly_set: &std::collections::HashSet<&str>,
    runtime: &Runtime,
) -> String {
    let mut flags = String::new();
    if readonly_set.contains(name) {
        flags.push('r');
    }
    if runtime.is_exported(name) {
        flags.push('x');
    }
    if flags.is_empty() {
        String::new()
    } else {
        format!(" -{}", flags)
    }
}

/// Escape single quotes inside a value for shell-safe single-quoted output
fn shell_quote(value: &str) -> String {
    value.replace('\'', "'\\''")
}

/// Validate that a string is a legal shell variable name
fn validate_var_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(anyhow!("declare: invalid variable name: empty"));
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return Err(anyhow!("declare: `{}': not a valid identifier", name));
    }
    for ch in chars {
        if !ch.is_ascii_alphanumeric() && ch != '_' {
            return Err(anyhow!("declare: `{}': not a valid identifier", name));
        }
    }
    Ok(())
}

/// Returns true for positional/special variables we skip in full listings
fn is_special_var(name: &str) -> bool {
    matches!(
        name,
        "0" | "1"
            | "2"
            | "3"
            | "4"
            | "5"
            | "6"
            | "7"
            | "8"
            | "9"
            | "#"
            | "@"
            | "*"
            | "?"
            | "$"
            | "!"
            | "-"
            | "_"
            | "PIPESTATUS"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;

    fn make_args(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_builtin_declare_simple_assignment() {
        let mut runtime = Runtime::new();
        let result = builtin_declare(&make_args(&["FOO=bar"]), &mut runtime);
        assert!(result.is_ok());
        assert_eq!(runtime.get_variable("FOO"), Some("bar".to_string()));
    }

    #[test]
    fn test_builtin_declare_declaration_only() {
        let mut runtime = Runtime::new();
        let result = builtin_declare(&make_args(&["MYVAR"]), &mut runtime);
        assert!(result.is_ok());
        assert_eq!(runtime.get_variable("MYVAR"), Some(String::new()));
    }

    #[test]
    fn test_builtin_declare_readonly_flag() {
        let mut runtime = Runtime::new();
        let result = builtin_declare(&make_args(&["-r", "ROVAR=hello"]), &mut runtime);
        assert!(result.is_ok());
        assert_eq!(runtime.get_variable("ROVAR"), Some("hello".to_string()));
        assert!(runtime.is_readonly("ROVAR"));
    }

    #[test]
    fn test_builtin_declare_export_flag() {
        let mut runtime = Runtime::new();
        let result = builtin_declare(&make_args(&["-x", "MYEXPORT=world"]), &mut runtime);
        assert!(result.is_ok());
        assert_eq!(runtime.get_variable("MYEXPORT"), Some("world".to_string()));
        // The variable should now be in the process environment
        assert!(runtime.is_exported("MYEXPORT"));
    }

    #[test]
    fn test_builtin_declare_integer_flag_accepted() {
        let mut runtime = Runtime::new();
        // -i should be accepted without error (we don't enforce integer arithmetic)
        let result = builtin_declare(&make_args(&["-i", "INTVAR=42"]), &mut runtime);
        assert!(result.is_ok());
        assert_eq!(runtime.get_variable("INTVAR"), Some("42".to_string()));
    }

    #[test]
    fn test_builtin_declare_array_flag_accepted() {
        let mut runtime = Runtime::new();
        let result = builtin_declare(&make_args(&["-a", "ARRVAR"]), &mut runtime);
        assert!(result.is_ok());
    }

    #[test]
    fn test_builtin_declare_assoc_flag_accepted() {
        let mut runtime = Runtime::new();
        let result = builtin_declare(&make_args(&["-A", "HASHVAR"]), &mut runtime);
        assert!(result.is_ok());
    }

    #[test]
    fn test_builtin_declare_combined_flags() {
        let mut runtime = Runtime::new();
        // -rx: readonly + export
        let result = builtin_declare(&make_args(&["-rx", "COMBO=yes"]), &mut runtime);
        assert!(result.is_ok());
        assert!(runtime.is_readonly("COMBO"));
        assert!(runtime.is_exported("COMBO"));
    }

    #[test]
    fn test_builtin_declare_print_flag() {
        let mut runtime = Runtime::new();
        runtime.set_variable("PRINTME".to_string(), "value123".to_string());

        let result = builtin_declare(&make_args(&["-p", "PRINTME"]), &mut runtime);
        assert!(result.is_ok());
        let output = result.unwrap().stdout();
        assert!(output.contains("PRINTME"));
        assert!(output.contains("value123"));
        assert!(output.contains("declare"));
    }

    #[test]
    fn test_builtin_declare_print_no_args_lists_all() {
        let mut runtime = Runtime::new();
        runtime.set_variable("VARA".to_string(), "aaa".to_string());
        runtime.set_variable("VARB".to_string(), "bbb".to_string());

        let result = builtin_declare(&make_args(&["-p"]), &mut runtime);
        assert!(result.is_ok());
        let output = result.unwrap().stdout();
        assert!(output.contains("VARA"));
        assert!(output.contains("VARB"));
    }

    #[test]
    fn test_builtin_declare_no_args_lists_all() {
        let mut runtime = Runtime::new();
        runtime.set_variable("LISTME".to_string(), "found".to_string());

        let result = builtin_declare(&[], &mut runtime);
        assert!(result.is_ok());
        let output = result.unwrap().stdout();
        assert!(output.contains("LISTME"));
        assert!(output.contains("found"));
    }

    #[test]
    fn test_builtin_declare_invalid_name() {
        let mut runtime = Runtime::new();
        let result = builtin_declare(&make_args(&["123bad=val"]), &mut runtime);
        assert!(result.is_err());
    }

    #[test]
    fn test_builtin_declare_invalid_flag() {
        let mut runtime = Runtime::new();
        let result = builtin_declare(&make_args(&["-Z", "VAR"]), &mut runtime);
        assert!(result.is_err());
    }

    #[test]
    fn test_builtin_declare_readonly_error_on_reassign() {
        let mut runtime = Runtime::new();
        builtin_declare(&make_args(&["-r", "LOCKED=orig"]), &mut runtime).unwrap();

        // Trying to declare again as readonly with new value should error
        let result = builtin_declare(&make_args(&["-r", "LOCKED=new"]), &mut runtime);
        assert!(result.is_err());
        // Value should remain unchanged
        assert_eq!(runtime.get_variable("LOCKED"), Some("orig".to_string()));
    }

    #[test]
    fn test_builtin_declare_print_missing_var() {
        let mut runtime = Runtime::new();
        let result = builtin_declare(&make_args(&["-p", "NOSUCHVAR"]), &mut runtime);
        // Should return non-zero but not panic
        assert!(result.is_ok());
        let exec_result = result.unwrap();
        assert_ne!(exec_result.exit_code, 0);
    }

    #[test]
    fn test_builtin_declare_multiple_vars() {
        let mut runtime = Runtime::new();
        let result = builtin_declare(&make_args(&["A=1", "B=2", "C=3"]), &mut runtime);
        assert!(result.is_ok());
        assert_eq!(runtime.get_variable("A"), Some("1".to_string()));
        assert_eq!(runtime.get_variable("B"), Some("2".to_string()));
        assert_eq!(runtime.get_variable("C"), Some("3".to_string()));
    }

    #[test]
    fn test_builtin_declare_flag_and_assignment() {
        let mut runtime = Runtime::new();
        let result = builtin_declare(&make_args(&["-i", "NUM=5"]), &mut runtime);
        assert!(result.is_ok());
        assert_eq!(runtime.get_variable("NUM"), Some("5".to_string()));
    }
}
