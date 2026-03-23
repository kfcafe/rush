//! `math` builtin — floating-point arithmetic with math functions.
//!
//! Usage:  math "expr"
//!
//! Supports:
//!   Operators:  + - * / % ^ (power)
//!   Comparisons: < > <= >= == !=
//!   Grouping:   ( )
//!   Functions:  sin cos tan asin acos atan ceil floor round abs sqrt log ln
//!   Constants:  pi e

use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

pub fn builtin_math(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.is_empty() {
        return Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: "math: usage: math \"expression\"\n".to_string(),
            exit_code: 1,
            error: None,
        });
    }

    // Join all arguments with a space — allows `math 1 + 2` as well as `math "1 + 2"`
    let expr = args.join(" ");

    match eval(&expr) {
        Ok(value) => {
            // Format: if the result is a whole number, print without decimal point
            let formatted = format_value(value);
            Ok(ExecutionResult::success(formatted + "\n"))
        }
        Err(e) => Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: format!("math: {}\n", e),
            exit_code: 1,
            error: None,
        }),
    }
}

/// Format a float result: integers print without a decimal, others use enough precision.
fn format_value(v: f64) -> String {
    if v.is_nan() {
        return "nan".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".to_string() } else { "-inf".to_string() };
    }
    // If it's an exact integer (within rounding), print without decimal
    if v.fract() == 0.0 && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    // Otherwise strip trailing zeros but keep enough precision
    let s = format!("{:.6}", v);
    let s = s.trim_end_matches('0');
    let s = s.trim_end_matches('.');
    s.to_string()
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Number(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    LParen,
    RParen,
    Lt,
    Gt,
    Le,
    Ge,
    EqEq,
    Ne,
}

fn tokenize(input: &str) -> Result<Vec<Tok>> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        if ch.is_ascii_whitespace() {
            i += 1;
            continue;
        }

        // Numbers (including decimals and leading dot like .5)
        if ch.is_ascii_digit() || (ch == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit()) {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            let v: f64 = s.parse().map_err(|_| anyhow!("invalid number '{}'", s))?;
            tokens.push(Tok::Number(v));
            continue;
        }

        // Identifiers and keywords (functions / constants)
        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let name: String = chars[start..i].iter().collect();
            tokens.push(Tok::Ident(name));
            continue;
        }

        // Two-character operators
        if i + 1 < chars.len() {
            let two: String = chars[i..i + 2].iter().collect();
            let tok = match two.as_str() {
                "<=" => Some(Tok::Le),
                ">=" => Some(Tok::Ge),
                "==" => Some(Tok::EqEq),
                "!=" => Some(Tok::Ne),
                _ => None,
            };
            if let Some(t) = tok {
                tokens.push(t);
                i += 2;
                continue;
            }
        }

        // Single-character operators
        let tok = match ch {
            '+' => Tok::Plus,
            '-' => Tok::Minus,
            '*' => Tok::Star,
            '/' => Tok::Slash,
            '%' => Tok::Percent,
            '^' => Tok::Caret,
            '(' => Tok::LParen,
            ')' => Tok::RParen,
            '<' => Tok::Lt,
            '>' => Tok::Gt,
            _ => return Err(anyhow!("unexpected character '{}'", ch)),
        };
        tokens.push(tok);
        i += 1;
    }

    Ok(tokens)
}

// ---------------------------------------------------------------------------
// Recursive-descent parser / evaluator
// ---------------------------------------------------------------------------

struct Parser {
    tokens: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Tok>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Tok> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn eat(&mut self, expected: &Tok) -> bool {
        if self.peek() == Some(expected) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    // Precedence (low → high):
    //   comparison: < > <= >= == !=
    //   additive:   + -
    //   multiplicative: * / %
    //   power: ^ (right-associative)
    //   unary: - (prefix)
    //   primary: number | constant | function call | ( expr )

    fn parse_expr(&mut self) -> Result<f64> {
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<f64> {
        let mut lhs = self.parse_additive()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Lt) => '<',
                Some(Tok::Gt) => '>',
                Some(Tok::Le) => 'l',
                Some(Tok::Ge) => 'g',
                Some(Tok::EqEq) => '=',
                Some(Tok::Ne) => '!',
                _ => break,
            };
            self.advance();
            let rhs = self.parse_additive()?;
            lhs = match op {
                '<' => if lhs < rhs { 1.0 } else { 0.0 },
                '>' => if lhs > rhs { 1.0 } else { 0.0 },
                'l' => if lhs <= rhs { 1.0 } else { 0.0 },
                'g' => if lhs >= rhs { 1.0 } else { 0.0 },
                '=' => if (lhs - rhs).abs() < f64::EPSILON { 1.0 } else { 0.0 },
                '!' => if (lhs - rhs).abs() >= f64::EPSILON { 1.0 } else { 0.0 },
                _ => unreachable!(),
            };
        }
        Ok(lhs)
    }

    fn parse_additive(&mut self) -> Result<f64> {
        let mut lhs = self.parse_multiplicative()?;
        loop {
            match self.peek() {
                Some(Tok::Plus) => {
                    self.advance();
                    lhs += self.parse_multiplicative()?;
                }
                Some(Tok::Minus) => {
                    self.advance();
                    lhs -= self.parse_multiplicative()?;
                }
                _ => break,
            }
        }
        Ok(lhs)
    }

    fn parse_multiplicative(&mut self) -> Result<f64> {
        let mut lhs = self.parse_power()?;
        loop {
            match self.peek() {
                Some(Tok::Star) => {
                    self.advance();
                    lhs *= self.parse_power()?;
                }
                Some(Tok::Slash) => {
                    self.advance();
                    let rhs = self.parse_power()?;
                    if rhs == 0.0 {
                        return Err(anyhow!("division by zero"));
                    }
                    lhs /= rhs;
                }
                Some(Tok::Percent) => {
                    self.advance();
                    let rhs = self.parse_power()?;
                    if rhs == 0.0 {
                        return Err(anyhow!("division by zero"));
                    }
                    lhs %= rhs;
                }
                _ => break,
            }
        }
        Ok(lhs)
    }

    fn parse_power(&mut self) -> Result<f64> {
        let base = self.parse_unary()?;
        if self.eat(&Tok::Caret) {
            // Right-associative
            let exp = self.parse_power()?;
            Ok(base.powf(exp))
        } else {
            Ok(base)
        }
    }

    fn parse_unary(&mut self) -> Result<f64> {
        if self.eat(&Tok::Minus) {
            Ok(-self.parse_primary()?)
        } else {
            self.eat(&Tok::Plus); // optional unary +
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> Result<f64> {
        match self.peek().cloned() {
            Some(Tok::Number(v)) => {
                self.advance();
                Ok(v)
            }
            Some(Tok::Ident(name)) => {
                self.advance();
                // Check if it's a function call (followed by '(')
                if self.eat(&Tok::LParen) {
                    let arg = self.parse_expr()?;
                    if !self.eat(&Tok::RParen) {
                        return Err(anyhow!("expected ')' after function argument"));
                    }
                    self.apply_function(&name, arg)
                } else {
                    // Must be a constant
                    self.resolve_constant(&name)
                }
            }
            Some(Tok::LParen) => {
                self.advance();
                let v = self.parse_expr()?;
                if !self.eat(&Tok::RParen) {
                    return Err(anyhow!("expected ')'"));
                }
                Ok(v)
            }
            other => Err(anyhow!("unexpected token: {:?}", other)),
        }
    }

    fn apply_function(&self, name: &str, arg: f64) -> Result<f64> {
        match name {
            "sin" => Ok(arg.sin()),
            "cos" => Ok(arg.cos()),
            "tan" => Ok(arg.tan()),
            "asin" => Ok(arg.asin()),
            "acos" => Ok(arg.acos()),
            "atan" => Ok(arg.atan()),
            "ceil" => Ok(arg.ceil()),
            "floor" => Ok(arg.floor()),
            "round" => Ok(arg.round()),
            "abs" => Ok(arg.abs()),
            "sqrt" => {
                if arg < 0.0 {
                    return Err(anyhow!("sqrt of negative number"));
                }
                Ok(arg.sqrt())
            }
            "log" | "log10" => {
                if arg <= 0.0 {
                    return Err(anyhow!("log of non-positive number"));
                }
                Ok(arg.log10())
            }
            "ln" | "log2" => {
                if arg <= 0.0 {
                    return Err(anyhow!("ln of non-positive number"));
                }
                if name == "log2" { Ok(arg.log2()) } else { Ok(arg.ln()) }
            }
            "exp" => Ok(arg.exp()),
            _ => Err(anyhow!("unknown function '{}'", name)),
        }
    }

    fn resolve_constant(&self, name: &str) -> Result<f64> {
        match name {
            "pi" | "PI" => Ok(std::f64::consts::PI),
            "e" | "E" => Ok(std::f64::consts::E),
            "tau" => Ok(std::f64::consts::TAU),
            "inf" => Ok(f64::INFINITY),
            "nan" => Ok(f64::NAN),
            _ => Err(anyhow!("unknown constant or variable '{}'", name)),
        }
    }
}

/// Evaluate a math expression string, returning an f64.
pub fn eval(expr: &str) -> Result<f64> {
    let tokens = tokenize(expr)?;
    if tokens.is_empty() {
        return Err(anyhow!("empty expression"));
    }
    let mut parser = Parser::new(tokens);
    let result = parser.parse_expr()?;
    if parser.pos < parser.tokens.len() {
        return Err(anyhow!(
            "unexpected token at position {}",
            parser.pos
        ));
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;

    fn math(expr: &str) -> String {
        let mut rt = Runtime::new();
        let args = vec![expr.to_string()];
        let result = builtin_math(&args, &mut rt).unwrap();
        result.stdout().trim().to_string()
    }

    fn math_err(expr: &str) -> bool {
        let mut rt = Runtime::new();
        let args = vec![expr.to_string()];
        let result = builtin_math(&args, &mut rt).unwrap();
        result.exit_code != 0
    }

    #[test]
    fn builtin_math_addition() {
        assert_eq!(math("1 + 2"), "3");
    }

    #[test]
    fn builtin_math_float() {
        assert_eq!(math("3.14 * 2"), "6.28");
    }

    #[test]
    fn builtin_math_sin_pi() {
        // sin(pi) ≈ 0 (within float precision)
        let mut rt = Runtime::new();
        let args = vec!["sin(pi)".to_string()];
        let result = builtin_math(&args, &mut rt).unwrap();
        let v: f64 = result.stdout().trim().parse().unwrap();
        assert!(v.abs() < 1e-10, "sin(pi) should be ~0, got {}", v);
    }

    #[test]
    fn builtin_math_ceil() {
        assert_eq!(math("ceil(3.2)"), "4");
    }

    #[test]
    fn builtin_math_floor() {
        assert_eq!(math("floor(3.8)"), "3");
    }

    #[test]
    fn builtin_math_round() {
        assert_eq!(math("round(3.5)"), "4");
    }

    #[test]
    fn builtin_math_abs() {
        assert_eq!(math("abs(-5)"), "5");
    }

    #[test]
    fn builtin_math_sqrt() {
        assert_eq!(math("sqrt(16)"), "4");
    }

    #[test]
    fn builtin_math_power() {
        assert_eq!(math("2 ^ 10"), "1024");
    }

    #[test]
    fn builtin_math_subtraction() {
        assert_eq!(math("10 - 3"), "7");
    }

    #[test]
    fn builtin_math_multiplication() {
        assert_eq!(math("6 * 7"), "42");
    }

    #[test]
    fn builtin_math_division() {
        assert_eq!(math("10 / 4"), "2.5");
    }

    #[test]
    fn builtin_math_modulo() {
        assert_eq!(math("10 % 3"), "1");
    }

    #[test]
    fn builtin_math_parens() {
        assert_eq!(math("(1 + 2) * 3"), "9");
    }

    #[test]
    fn builtin_math_unary_minus() {
        assert_eq!(math("-5 + 10"), "5");
    }

    #[test]
    fn builtin_math_pi_constant() {
        let mut rt = Runtime::new();
        let args = vec!["pi".to_string()];
        let result = builtin_math(&args, &mut rt).unwrap();
        let v: f64 = result.stdout().trim().parse().unwrap();
        assert!((v - std::f64::consts::PI).abs() < 1e-6);
    }

    #[test]
    fn builtin_math_comparison_true() {
        assert_eq!(math("3 > 2"), "1");
    }

    #[test]
    fn builtin_math_comparison_false() {
        assert_eq!(math("1 > 2"), "0");
    }

    #[test]
    fn builtin_math_no_args_returns_error() {
        let mut rt = Runtime::new();
        let result = builtin_math(&[], &mut rt).unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn builtin_math_division_by_zero_is_error() {
        assert!(math_err("1 / 0"));
    }

    #[test]
    fn builtin_math_random_in_range() {
        let mut rt = Runtime::new();
        // $RANDOM is a runtime variable — just test that math itself works without it
        // The RANDOM var test is in runtime
        let val = rt.get_variable("RANDOM");
        assert!(val.is_some());
        let n: u32 = val.unwrap().parse().unwrap();
        assert!(n <= 32767);
    }
}
