use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

/// Represents a format specifier in a printf format string
#[derive(Debug, PartialEq)]
enum FormatSpec {
    /// String format (%s)
    String {
        width: Option<i32>,
        left_align: bool,
    },
    /// Decimal integer format (%d)
    Decimal {
        width: Option<i32>,
        left_align: bool,
    },
    /// Floating point format (%f)
    Float {
        width: Option<i32>,
        precision: Option<usize>,
        left_align: bool,
    },
    /// Hexadecimal format (%x)
    Hex {
        width: Option<i32>,
        left_align: bool,
    },
    /// Octal format (%o)
    Octal {
        width: Option<i32>,
        left_align: bool,
    },
    /// Literal text
    Literal(String),
}

/// Parse a printf format string into a sequence of format specifiers
fn parse_format_string(format: &str) -> Result<Vec<FormatSpec>> {
    let mut specs = Vec::new();
    let mut chars = format.chars().peekable();
    let mut current_literal = String::new();

    while let Some(ch) = chars.next() {
        if ch == '%' {
            // Check for %%
            if chars.peek() == Some(&'%') {
                chars.next();
                current_literal.push('%');
                continue;
            }

            // Save any accumulated literal text
            if !current_literal.is_empty() {
                specs.push(FormatSpec::Literal(current_literal.clone()));
                current_literal.clear();
            }

            // Parse format specifier
            let mut left_align = false;
            let mut width: Option<i32> = None;
            let mut precision: Option<usize> = None;

            // Check for left alignment flag
            if chars.peek() == Some(&'-') {
                left_align = true;
                chars.next();
            }

            // Parse width
            let mut width_str = String::new();
            while let Some(&ch) = chars.peek() {
                if ch.is_ascii_digit() {
                    width_str.push(ch);
                    chars.next();
                } else {
                    break;
                }
            }
            if !width_str.is_empty() {
                width = Some(width_str.parse::<i32>().unwrap_or(0));
            }

            // Parse precision
            if chars.peek() == Some(&'.') {
                chars.next();
                let mut precision_str = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch.is_ascii_digit() {
                        precision_str.push(ch);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if !precision_str.is_empty() {
                    precision = Some(precision_str.parse::<usize>().unwrap_or(0));
                }
            }

            // Parse format type
            if let Some(type_ch) = chars.next() {
                let spec = match type_ch {
                    's' => FormatSpec::String { width, left_align },
                    'd' | 'i' => FormatSpec::Decimal { width, left_align },
                    'f' => FormatSpec::Float {
                        width,
                        precision,
                        left_align,
                    },
                    'x' => FormatSpec::Hex { width, left_align },
                    'o' => FormatSpec::Octal { width, left_align },
                    _ => return Err(anyhow!("printf: invalid format specifier: %{}", type_ch)),
                };
                specs.push(spec);
            } else {
                return Err(anyhow!("printf: incomplete format specifier"));
            }
        } else if ch == '\\' {
            // Handle escape sequences
            if let Some(escape_ch) = chars.next() {
                match escape_ch {
                    'n' => current_literal.push('\n'),
                    't' => current_literal.push('\t'),
                    'r' => current_literal.push('\r'),
                    '\\' => current_literal.push('\\'),
                    '\'' => current_literal.push('\''),
                    '"' => current_literal.push('"'),
                    _ => {
                        // Unknown escape, keep as-is
                        current_literal.push('\\');
                        current_literal.push(escape_ch);
                    }
                }
            }
        } else {
            current_literal.push(ch);
        }
    }

    // Save any remaining literal text
    if !current_literal.is_empty() {
        specs.push(FormatSpec::Literal(current_literal));
    }

    Ok(specs)
}

/// Apply a format specifier to an argument
fn apply_format(spec: &FormatSpec, arg: Option<&str>) -> Result<String> {
    match spec {
        FormatSpec::Literal(text) => Ok(text.clone()),
        FormatSpec::String { width, left_align } => {
            let value = arg.unwrap_or("");
            Ok(format_with_width(value, *width, *left_align))
        }
        FormatSpec::Decimal { width, left_align } => {
            let value = arg.unwrap_or("0");
            let num = value.parse::<i64>().unwrap_or(0);
            let formatted = num.to_string();
            Ok(format_with_width(&formatted, *width, *left_align))
        }
        FormatSpec::Float {
            width,
            precision,
            left_align,
        } => {
            let value = arg.unwrap_or("0");
            let num = value.parse::<f64>().unwrap_or(0.0);
            let precision = precision.unwrap_or(6);
            let formatted = format!("{:.prec$}", num, prec = precision);
            Ok(format_with_width(&formatted, *width, *left_align))
        }
        FormatSpec::Hex { width, left_align } => {
            let value = arg.unwrap_or("0");
            let num = value.parse::<i64>().unwrap_or(0);
            let formatted = format!("{:x}", num);
            Ok(format_with_width(&formatted, *width, *left_align))
        }
        FormatSpec::Octal { width, left_align } => {
            let value = arg.unwrap_or("0");
            let num = value.parse::<i64>().unwrap_or(0);
            let formatted = format!("{:o}", num);
            Ok(format_with_width(&formatted, *width, *left_align))
        }
    }
}

/// Apply width formatting to a string
fn format_with_width(value: &str, width: Option<i32>, left_align: bool) -> String {
    match width {
        Some(w) if w > 0 => {
            let w = w as usize;
            if left_align {
                format!("{:<width$}", value, width = w)
            } else {
                format!("{:>width$}", value, width = w)
            }
        }
        _ => value.to_string(),
    }
}

pub fn builtin_printf(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.is_empty() {
        return Err(anyhow!("printf: usage: printf format [arguments]"));
    }

    let format_string = &args[0];
    let arguments = &args[1..];

    // Parse the format string
    let specs = match parse_format_string(format_string) {
        Ok(s) => s,
        Err(e) => {
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: e.to_string() + "\n",
                exit_code: 1,
                error: None,
            });
        }
    };

    let mut output = String::new();
    let mut arg_index = 0;

    // Count format specifiers (non-literals)
    let format_spec_count = specs
        .iter()
        .filter(|s| !matches!(s, FormatSpec::Literal(_)))
        .count();

    // If there are no format specifiers, just output the format string
    if format_spec_count == 0 {
        for spec in &specs {
            if let FormatSpec::Literal(text) = spec {
                output.push_str(text);
            }
        }
        return Ok(ExecutionResult::success(output));
    }

    // Process arguments, reusing format if there are more arguments than format specs
    loop {
        let mut used_arg = false;

        for spec in &specs {
            match spec {
                FormatSpec::Literal(_) => {
                    output.push_str(&apply_format(spec, None)?);
                }
                _ => {
                    let arg = arguments.get(arg_index);
                    output.push_str(&apply_format(spec, arg.map(|s| s.as_str()))?);
                    arg_index += 1;
                    used_arg = true;
                }
            }
        }

        // If we've processed all arguments, break
        if arg_index >= arguments.len() {
            break;
        }

        // If we didn't use any argument in this iteration (all literals), break to avoid infinite loop
        if !used_arg {
            break;
        }
    }

    Ok(ExecutionResult::success(output))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_format_string_literal() {
        let specs = parse_format_string("Hello World").unwrap();
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0], FormatSpec::Literal("Hello World".to_string()));
    }

    #[test]
    fn test_parse_format_string_simple() {
        let specs = parse_format_string("Hello %s").unwrap();
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0], FormatSpec::Literal("Hello ".to_string()));
        assert_eq!(
            specs[1],
            FormatSpec::String {
                width: None,
                left_align: false
            }
        );
    }

    #[test]
    fn test_parse_format_string_multiple() {
        let specs = parse_format_string("%s %d %f").unwrap();
        assert_eq!(specs.len(), 5);
        assert_eq!(
            specs[0],
            FormatSpec::String {
                width: None,
                left_align: false
            }
        );
        assert_eq!(specs[1], FormatSpec::Literal(" ".to_string()));
        assert_eq!(
            specs[2],
            FormatSpec::Decimal {
                width: None,
                left_align: false
            }
        );
        assert_eq!(specs[3], FormatSpec::Literal(" ".to_string()));
        assert_eq!(
            specs[4],
            FormatSpec::Float {
                width: None,
                precision: None,
                left_align: false
            }
        );
    }

    #[test]
    fn test_parse_format_string_width() {
        let specs = parse_format_string("%10s").unwrap();
        assert_eq!(specs.len(), 1);
        assert_eq!(
            specs[0],
            FormatSpec::String {
                width: Some(10),
                left_align: false
            }
        );
    }

    #[test]
    fn test_parse_format_string_left_align() {
        let specs = parse_format_string("%-10s").unwrap();
        assert_eq!(specs.len(), 1);
        assert_eq!(
            specs[0],
            FormatSpec::String {
                width: Some(10),
                left_align: true
            }
        );
    }

    #[test]
    fn test_parse_format_string_precision() {
        let specs = parse_format_string("%.2f").unwrap();
        assert_eq!(specs.len(), 1);
        assert_eq!(
            specs[0],
            FormatSpec::Float {
                width: None,
                precision: Some(2),
                left_align: false
            }
        );
    }

    #[test]
    fn test_parse_format_string_width_and_precision() {
        let specs = parse_format_string("%10.2f").unwrap();
        assert_eq!(specs.len(), 1);
        assert_eq!(
            specs[0],
            FormatSpec::Float {
                width: Some(10),
                precision: Some(2),
                left_align: false
            }
        );
    }

    #[test]
    fn test_parse_format_string_escape_sequences() {
        let specs = parse_format_string("Line 1\\nLine 2\\tTabbed").unwrap();
        assert_eq!(specs.len(), 1);
        assert_eq!(
            specs[0],
            FormatSpec::Literal("Line 1\nLine 2\tTabbed".to_string())
        );
    }

    #[test]
    fn test_parse_format_string_percent_escape() {
        let specs = parse_format_string("100%% complete").unwrap();
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0], FormatSpec::Literal("100% complete".to_string()));
    }

    #[test]
    fn test_printf_simple_string() {
        let mut runtime = Runtime::new();
        let result =
            builtin_printf(&["Hello %s".to_string(), "World".to_string()], &mut runtime).unwrap();
        assert_eq!(result.stdout(), "Hello World");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_printf_decimal() {
        let mut runtime = Runtime::new();
        let result =
            builtin_printf(&["Count: %d".to_string(), "42".to_string()], &mut runtime).unwrap();
        assert_eq!(result.stdout(), "Count: 42");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_printf_float() {
        let mut runtime = Runtime::new();
        let result = builtin_printf(
            &["Price: %.2f".to_string(), "3.14159".to_string()],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.stdout(), "Price: 3.14");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_printf_hex() {
        let mut runtime = Runtime::new();
        let result =
            builtin_printf(&["Hex: %x".to_string(), "255".to_string()], &mut runtime).unwrap();
        assert_eq!(result.stdout(), "Hex: ff");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_printf_octal() {
        let mut runtime = Runtime::new();
        let result =
            builtin_printf(&["Octal: %o".to_string(), "255".to_string()], &mut runtime).unwrap();
        assert_eq!(result.stdout(), "Octal: 377");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_printf_width() {
        let mut runtime = Runtime::new();
        let result =
            builtin_printf(&["%10s".to_string(), "test".to_string()], &mut runtime).unwrap();
        assert_eq!(result.stdout(), "      test");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_printf_left_align() {
        let mut runtime = Runtime::new();
        let result =
            builtin_printf(&["%-10s".to_string(), "test".to_string()], &mut runtime).unwrap();
        assert_eq!(result.stdout(), "test      ");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_printf_escape_sequences() {
        let mut runtime = Runtime::new();
        let result = builtin_printf(&["Line 1\\nLine 2".to_string()], &mut runtime).unwrap();
        assert_eq!(result.stdout(), "Line 1\nLine 2");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_printf_no_newline() {
        let mut runtime = Runtime::new();
        let result = builtin_printf(&["Hello".to_string()], &mut runtime).unwrap();
        assert_eq!(result.stdout(), "Hello");
        assert!(!result.stdout().ends_with('\n'));
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_printf_multiple_args() {
        let mut runtime = Runtime::new();
        let result = builtin_printf(
            &[
                "Name: %s, Age: %d".to_string(),
                "Alice".to_string(),
                "30".to_string(),
            ],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.stdout(), "Name: Alice, Age: 30");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_printf_reuse_format() {
        let mut runtime = Runtime::new();
        let result = builtin_printf(
            &[
                "%s\\n".to_string(),
                "one".to_string(),
                "two".to_string(),
                "three".to_string(),
            ],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.stdout(), "one\ntwo\nthree\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_printf_aligned_columns() {
        let mut runtime = Runtime::new();
        let result = builtin_printf(
            &[
                "%-20s %10.2f\\n".to_string(),
                "Apple".to_string(),
                "1.99".to_string(),
            ],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.stdout(), "Apple                      1.99\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_printf_no_args() {
        let mut runtime = Runtime::new();
        let result = builtin_printf(&[], &mut runtime);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("usage"));
    }

    #[test]
    fn test_printf_missing_args() {
        let mut runtime = Runtime::new();
        let result =
            builtin_printf(&["%s %d".to_string(), "test".to_string()], &mut runtime).unwrap();
        // Missing argument should be treated as "0" for numbers
        assert_eq!(result.stdout(), "test 0");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_printf_invalid_number() {
        let mut runtime = Runtime::new();
        let result = builtin_printf(&["%d".to_string(), "abc".to_string()], &mut runtime).unwrap();
        // Invalid number should be treated as 0
        assert_eq!(result.stdout(), "0");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_printf_mixed_formats() {
        let mut runtime = Runtime::new();
        let result = builtin_printf(
            &[
                "Hex: %x, Octal: %o, Decimal: %d\\n".to_string(),
                "255".to_string(),
                "255".to_string(),
                "255".to_string(),
            ],
            &mut runtime,
        )
        .unwrap();
        assert_eq!(result.stdout(), "Hex: ff, Octal: 377, Decimal: 255\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_printf_literal_only() {
        let mut runtime = Runtime::new();
        let result = builtin_printf(&["Just text\\n".to_string()], &mut runtime).unwrap();
        assert_eq!(result.stdout(), "Just text\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_printf_percent_escape() {
        let mut runtime = Runtime::new();
        let result = builtin_printf(&["100%% complete\\n".to_string()], &mut runtime).unwrap();
        assert_eq!(result.stdout(), "100% complete\n");
        assert_eq!(result.exit_code, 0);
    }
}
