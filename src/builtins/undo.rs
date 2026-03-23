use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::Result;

pub fn builtin_undo(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.is_empty() {
        // Undo last operation
        match runtime.undo_manager_mut().undo() {
            Ok(msg) => Ok(ExecutionResult::success(format!("{}\n", msg))),
            Err(e) => Ok(ExecutionResult::error(format!("undo: {}\n", e))),
        }
    } else {
        match args[0].as_str() {
            "list" => {
                let limit = if args.len() > 1 {
                    args[1].parse().unwrap_or(10)
                } else {
                    10
                };

                let ops = runtime.undo_manager_mut().list_operations(limit);
                let mut output = String::new();

                if ops.is_empty() {
                    output.push_str("No operations to undo\n");
                } else {
                    output.push_str("Recent operations (most recent first):\n");
                    for (i, op) in ops.iter().enumerate() {
                        output.push_str(&format!("  {}: {}\n", i + 1, op.description));
                    }
                }

                Ok(ExecutionResult::success(output))
            }
            "enable" => {
                runtime.undo_manager_mut().enable();
                Ok(ExecutionResult::success(
                    "Undo tracking enabled\n".to_string(),
                ))
            }
            "disable" => {
                runtime.undo_manager_mut().disable();
                Ok(ExecutionResult::success(
                    "Undo tracking disabled\n".to_string(),
                ))
            }
            "clear" => {
                runtime.undo_manager_mut().clear()?;
                Ok(ExecutionResult::success(
                    "Undo history cleared\n".to_string(),
                ))
            }
            "--help" => Ok(ExecutionResult::success(
                "Usage: undo [COMMAND]\n\
                     \n\
                     Undo file operations\n\
                     \n\
                     COMMANDS:\n\
                     (none)        Undo the last operation\n\
                     list [N]      List recent operations (default: 10)\n\
                     enable        Enable undo tracking\n\
                     disable       Disable undo tracking\n\
                     clear         Clear undo history\n\
                     --help        Show this help message\n"
                    .to_string(),
            )),
            _ => Ok(ExecutionResult::error(format!(
                "undo: unknown command: {}\n",
                args[0]
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_undo_help() {
        let mut runtime = Runtime::new();
        let result = builtin_undo(&["--help".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout().contains("Usage: undo"));
    }

    #[test]
    fn test_undo_list_empty() {
        let mut runtime = Runtime::new();
        let result = builtin_undo(&["list".to_string()], &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout().contains("No operations to undo"));
    }
}
