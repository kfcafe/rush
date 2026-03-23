use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

struct TrOptions {
    delete: bool,
    squeeze: bool,
    complement: bool,
    set1: Vec<char>,
    set2: Vec<char>,
}

/// Expand a tr SET string into a list of characters.
/// Handles: ranges (a-z), character classes ([:lower:] etc.), escape sequences.
fn expand_set(s: &str) -> Result<Vec<char>> {
    let mut chars = Vec::new();
    let bytes: Vec<char> = s.chars().collect();
    let mut i = 0;

    while i < bytes.len() {
        // Check for character class [:xxx:]
        if i + 3 < bytes.len() && bytes[i] == '[' && bytes[i + 1] == ':' {
            if let Some(end) = find_class_end(&bytes, i + 2) {
                let class_name: String = bytes[i + 2..end].iter().collect();
                expand_class(&class_name, &mut chars)?;
                i = end + 2; // skip ':]'
                continue;
            }
        }

        // Backslash escape
        if bytes[i] == '\\' && i + 1 < bytes.len() {
            let ch = match bytes[i + 1] {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                '\\' => '\\',
                'a' => '\x07',
                'b' => '\x08',
                'f' => '\x0C',
                'v' => '\x0B',
                c => c,
            };
            chars.push(ch);
            i += 2;
            continue;
        }

        // Range: a-z (look ahead)
        if i + 2 < bytes.len() && bytes[i + 1] == '-' {
            let from = bytes[i];
            let to = bytes[i + 2];
            if to >= from {
                let from_u32 = from as u32;
                let to_u32 = to as u32;
                for cp in from_u32..=to_u32 {
                    if let Some(c) = char::from_u32(cp) {
                        chars.push(c);
                    }
                }
                i += 3;
                continue;
            }
        }

        chars.push(bytes[i]);
        i += 1;
    }

    Ok(chars)
}

fn find_class_end(bytes: &[char], start: usize) -> Option<usize> {
    // Looking for ':]' starting at `start`
    let mut i = start;
    while i + 1 < bytes.len() {
        if bytes[i] == ':' && bytes[i + 1] == ']' {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn expand_class(name: &str, chars: &mut Vec<char>) -> Result<()> {
    match name {
        "lower" => chars.extend('a'..='z'),
        "upper" => chars.extend('A'..='Z'),
        "digit" => chars.extend('0'..='9'),
        "space" => chars.extend([' ', '\t', '\n', '\r', '\x0C', '\x0B']),
        "alpha" => {
            chars.extend('a'..='z');
            chars.extend('A'..='Z');
        }
        "alnum" => {
            chars.extend('a'..='z');
            chars.extend('A'..='Z');
            chars.extend('0'..='9');
        }
        "punct" => {
            for c in '!'..='/' {
                chars.push(c);
            }
            for c in ':'..='@' {
                chars.push(c);
            }
            for c in '['..='`' {
                chars.push(c);
            }
            for c in '{'..='~' {
                chars.push(c);
            }
        }
        "blank" => chars.extend([' ', '\t']),
        "print" => {
            for c in ' '..='~' {
                chars.push(c);
            }
        }
        "graph" => {
            for c in '!'..='~' {
                chars.push(c);
            }
        }
        "cntrl" => {
            for i in 0u32..=31 {
                if let Some(c) = char::from_u32(i) {
                    chars.push(c);
                }
            }
            chars.push('\x7F');
        }
        other => return Err(anyhow!("tr: invalid character class '{}'", other)),
    }
    Ok(())
}

impl TrOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut delete = false;
        let mut squeeze = false;
        let mut complement = false;
        let mut sets: Vec<String> = Vec::new();

        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            if arg == "--" {
                sets.extend(args[i + 1..].iter().cloned());
                break;
            }
            if arg.starts_with('-') && arg.len() > 1 {
                for ch in arg[1..].chars() {
                    match ch {
                        'd' => delete = true,
                        's' => squeeze = true,
                        'c' | 'C' => complement = true,
                        _ => return Err(anyhow!("tr: invalid option -- '{}'", ch)),
                    }
                }
            } else {
                sets.push(arg.clone());
            }
            i += 1;
        }

        let set1 = sets
            .first()
            .map(|s| expand_set(s))
            .transpose()?
            .unwrap_or_default();
        let set2 = sets
            .get(1)
            .map(|s| expand_set(s))
            .transpose()?
            .unwrap_or_default();

        // Validate
        if delete && !squeeze && set2.is_empty() {
            // OK: just delete set1
        } else if !delete && !squeeze {
            if set2.is_empty() {
                return Err(anyhow!("tr: missing operand after set1"));
            }
        }

        Ok(TrOptions {
            delete,
            squeeze,
            complement,
            set1,
            set2,
        })
    }
}

fn process_tr(input: &str, opts: &TrOptions) -> String {
    // Build the effective set1 (apply complement if needed)
    let effective_set1: Vec<char> = if opts.complement {
        // All chars NOT in set1
        let in_set1: std::collections::HashSet<char> = opts.set1.iter().copied().collect();
        // Use printable ASCII + common chars as the universe for complement
        (0u32..=127)
            .filter_map(char::from_u32)
            .filter(|c| !in_set1.contains(c))
            .collect()
    } else {
        opts.set1.clone()
    };

    // Build translation table: for each char in set1, map to corresponding set2 char
    // If set2 is shorter, the last char of set2 is used for remaining set1 chars.
    let translate_char = |c: char| -> Option<char> {
        if let Some(pos) = effective_set1.iter().position(|&s| s == c) {
            if opts.delete {
                None // delete this char
            } else if !opts.set2.is_empty() {
                // translate
                let mapped_idx = pos.min(opts.set2.len() - 1);
                Some(opts.set2[mapped_idx])
            } else {
                Some(c)
            }
        } else {
            Some(c) // not in set1, pass through
        }
    };

    if opts.squeeze {
        // After translation, squeeze repeated chars in squeeze-set
        // Squeeze set: if translating, squeeze set2; if only squeezing, squeeze set1
        let squeeze_set: std::collections::HashSet<char> = if !opts.set2.is_empty() {
            opts.set2.iter().copied().collect()
        } else {
            effective_set1.iter().copied().collect()
        };

        let mut result = String::with_capacity(input.len());
        let mut last_squeezed: Option<char> = None;

        for c in input.chars() {
            match translate_char(c) {
                None => {
                    last_squeezed = None;
                }
                Some(translated) => {
                    if squeeze_set.contains(&translated) {
                        if last_squeezed == Some(translated) {
                            // Squeeze: skip duplicate
                            continue;
                        }
                        last_squeezed = Some(translated);
                    } else {
                        last_squeezed = None;
                    }
                    result.push(translated);
                }
            }
        }
        result
    } else {
        input.chars().filter_map(translate_char).collect()
    }
}

fn run_tr(args: &[String], input: &[u8]) -> Result<ExecutionResult> {
    let opts = match TrOptions::parse(args) {
        Ok(o) => o,
        Err(e) => {
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: e.to_string(),
                exit_code: 1,
                error: None,
            });
        }
    };

    let text = String::from_utf8_lossy(input);
    let result = process_tr(&text, &opts);

    Ok(ExecutionResult {
        output: Output::Text(result),
        stderr: String::new(),
        exit_code: 0,
        error: None,
    })
}

pub fn builtin_tr(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    use std::io::Read;
    let mut data = Vec::new();
    std::io::stdin().read_to_end(&mut data).unwrap_or(0);
    run_tr(args, &data)
}

pub fn builtin_tr_with_stdin(
    args: &[String],
    _runtime: &mut Runtime,
    stdin_data: &[u8],
) -> Result<ExecutionResult> {
    run_tr(args, stdin_data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime() -> Runtime {
        Runtime::new()
    }

    fn tr_stdin(args: &[&str], input: &[u8]) -> ExecutionResult {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        builtin_tr_with_stdin(&args, &mut runtime(), input).unwrap()
    }

    #[test]
    fn test_tr_lowercase_to_uppercase() {
        let result = tr_stdin(&["a-z", "A-Z"], b"hello world\n");
        assert_eq!(result.stdout(), "HELLO WORLD\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_tr_uppercase_to_lowercase() {
        let result = tr_stdin(&["A-Z", "a-z"], b"HELLO\n");
        assert_eq!(result.stdout(), "hello\n");
    }

    #[test]
    fn test_tr_delete() {
        let result = tr_stdin(&["-d", "aeiou"], b"hello world\n");
        assert_eq!(result.stdout(), "hll wrld\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_tr_squeeze() {
        let result = tr_stdin(&["-s", " "], b"hello   world\n");
        assert_eq!(result.stdout(), "hello world\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_tr_class_lower_to_upper() {
        let result = tr_stdin(&["[:lower:]", "[:upper:]"], b"hello\n");
        assert_eq!(result.stdout(), "HELLO\n");
    }

    #[test]
    fn test_tr_delete_digits() {
        let result = tr_stdin(&["-d", "[:digit:]"], b"abc123def456\n");
        assert_eq!(result.stdout(), "abcdef\n");
    }

    #[test]
    fn test_tr_translate_single_char() {
        let result = tr_stdin(&[":", "\n"], b"a:b:c\n");
        assert_eq!(result.stdout(), "a\nb\nc\n");
    }

    #[test]
    fn test_tr_squeeze_and_translate() {
        // tr -s '[:space:]' ' ' squeezes all whitespace to single space
        let result = tr_stdin(&["-s", "[:space:]", " "], b"foo  \t  bar\n");
        assert_eq!(result.stdout(), "foo bar ");
    }

    #[test]
    fn test_tr_invalid_option() {
        let result = tr_stdin(&["-z", "a", "b"], b"data");
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("invalid option"));
    }
}
