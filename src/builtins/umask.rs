use crate::executor::ExecutionResult;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

pub fn builtin_umask(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    let _ = runtime; // Runtime not needed for umask — it operates on the process

    let symbolic = args.first().map(|a| a == "-S").unwrap_or(false);
    let mask_arg = if symbolic { args.get(1) } else { args.first() };

    if let Some(mask_str) = mask_arg {
        // Set the umask
        let new_mask = parse_mask(mask_str)?;
        unsafe { libc::umask(new_mask) };
        Ok(ExecutionResult::success(String::new()))
    } else {
        // Get and print the current umask
        let current = unsafe {
            let m = libc::umask(0o022);
            libc::umask(m); // restore
            m
        };

        let output = if symbolic {
            format_symbolic(current)
        } else {
            format!("{:04o}\n", current)
        };

        Ok(ExecutionResult::success(output))
    }
}

/// Parse an octal or symbolic mask string into a libc::mode_t value.
fn parse_mask(s: &str) -> Result<libc::mode_t> {
    // Octal: all digits
    if s.chars().all(|c| c.is_ascii_digit()) {
        let n = libc::mode_t::from_str_radix(s, 8)
            .map_err(|_| anyhow!("umask: invalid octal mask: {}", s))?;
        return Ok(n);
    }

    // Symbolic: apply to an all-permissions mask (like bash does)
    // Start from current mask and apply symbolic changes.
    // Format: [ugoa]*[+-=][rwxXst]* comma-separated
    let current = unsafe {
        let m = libc::umask(0o022);
        libc::umask(m);
        m
    };

    // For umask, symbolic form means what permissions to ALLOW (not block).
    // bash treats `umask -S u=rwx,g=rx,o=rx` as setting mask to 0022.
    // The symbolic sets ALLOWED bits; umask blocks the complement.
    parse_symbolic(s, current)
}

fn parse_symbolic(s: &str, current_mask: libc::mode_t) -> Result<libc::mode_t> {
    // Start from the complement of the current mask (what's currently allowed)
    let mut allowed: libc::mode_t = !current_mask & 0o777;

    for clause in s.split(',') {
        if clause.is_empty() {
            continue;
        }

        // Parse who: u, g, o, a (or empty = a)
        let mut chars = clause.chars().peekable();
        let mut who: libc::mode_t = 0;
        loop {
            match chars.peek() {
                Some('u') => { who |= 0o700; chars.next(); }
                Some('g') => { who |= 0o070; chars.next(); }
                Some('o') => { who |= 0o007; chars.next(); }
                Some('a') => { who |= 0o777; chars.next(); }
                _ => break,
            }
        }
        if who == 0 {
            who = 0o777; // default to all
        }

        // Parse operator: +, -, =
        let op = chars.next().ok_or_else(|| anyhow!("umask: invalid symbolic mask: {}", s))?;
        if op != '+' && op != '-' && op != '=' {
            return Err(anyhow!("umask: invalid operator '{}' in mask: {}", op, s));
        }

        // Parse permissions: r, w, x
        let mut perms: libc::mode_t = 0;
        for ch in chars {
            match ch {
                'r' => perms |= 0o444,
                'w' => perms |= 0o222,
                'x' => perms |= 0o111,
                _ => return Err(anyhow!("umask: invalid permission '{}' in mask: {}", ch, s)),
            }
        }
        let masked_perms = perms & who;

        match op {
            '+' => allowed |= masked_perms,
            '-' => allowed &= !masked_perms,
            '=' => {
                allowed = (allowed & !who) | masked_perms;
            }
            _ => unreachable!(),
        }
    }

    Ok((!allowed) & 0o777)
}

/// Format a mask in symbolic form: u=rwx,g=rx,o=rx
fn format_symbolic(mask: libc::mode_t) -> String {
    // allowed = complement of mask
    let allowed = !mask & 0o777;
    let format_class = |shift: u32| -> String {
        let bits = (allowed >> shift) & 0o7;
        let mut s = String::new();
        if bits & 0o4 != 0 { s.push('r'); }
        if bits & 0o2 != 0 { s.push('w'); }
        if bits & 0o1 != 0 { s.push('x'); }
        s
    };
    format!("u={},g={},o={}\n", format_class(6), format_class(3), format_class(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;
    use std::sync::Mutex;

    // umask is process-global state — serialize tests to prevent interference.
    static UMASK_LOCK: Mutex<()> = Mutex::new(());

    fn run(args: &[&str]) -> ExecutionResult {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let mut runtime = Runtime::new();
        builtin_umask(&args, &mut runtime).expect("umask failed")
    }

    #[test]
    fn test_builtin_umask_get_octal() {
        let _lock = UMASK_LOCK.lock().unwrap();
        unsafe { libc::umask(0o022) };
        let result = run(&[]);
        assert_eq!(result.stdout(), "0022\n");
    }

    #[test]
    fn test_builtin_umask_set_octal() {
        let _lock = UMASK_LOCK.lock().unwrap();
        let result = run(&["077"]);
        assert_eq!(result.exit_code, 0);
        let actual = unsafe { libc::umask(0o022) }; // read back and restore
        assert_eq!(actual, 0o077);
    }

    #[test]
    fn test_builtin_umask_get_symbolic() {
        let _lock = UMASK_LOCK.lock().unwrap();
        unsafe { libc::umask(0o022) };
        let result = run(&["-S"]);
        assert_eq!(result.stdout(), "u=rwx,g=rx,o=rx\n");
    }

    #[test]
    fn test_builtin_umask_set_symbolic() {
        let _lock = UMASK_LOCK.lock().unwrap();
        unsafe { libc::umask(0o000) }; // start from all-permitted
        let result = run(&["-S", "u=rwx,g=rx,o=rx"]);
        assert_eq!(result.exit_code, 0);
        let actual = unsafe { libc::umask(0o022) }; // read back and restore
        assert_eq!(actual, 0o022);
    }

    #[test]
    fn test_builtin_umask_set_octal_077_symbolic_output() {
        let _lock = UMASK_LOCK.lock().unwrap();
        unsafe { libc::umask(0o077) };
        let result = run(&["-S"]);
        assert_eq!(result.stdout(), "u=rwx,g=,o=\n");
    }
}
