//! Arithmetic expansion evaluator for $(( expr ))
//!
//! Implements a recursive-descent parser/evaluator supporting:
//! - Integer arithmetic: + - * / %
//! - Comparisons: < > <= >= == !=
//! - Bitwise: & | ^ ~ << >>
//! - Logical: && || !
//! - Parenthesized grouping
//! - Variable references (no $ needed inside $(( )))
//! - Assignment operators: = += -= *= /=

use crate::runtime::Runtime;
use anyhow::{anyhow, Result};

/// Evaluate an arithmetic expression string in the context of a runtime.
/// Variables referenced by name (without $) resolve to their numeric value (0 if unset/non-numeric).
pub fn evaluate(expr: &str, runtime: &Runtime) -> Result<i64> {
    let tokens = tokenize(expr)?;
    let mut parser = ArithParser::new(&tokens, runtime);
    let result = parser.parse_assignment()?;
    if parser.pos < parser.tokens.len() {
        return Err(anyhow!(
            "arithmetic: unexpected token at position {}",
            parser.pos
        ));
    }
    Ok(result)
}

/// Evaluate an arithmetic expression string, returning the result and any variable
/// assignments that were made. Used by resolve_argument where we need to update the runtime.
pub fn evaluate_mut(expr: &str, runtime: &mut Runtime) -> Result<i64> {
    let tokens = tokenize(expr)?;
    let mut parser = ArithParser::new(&tokens, runtime);
    let result = parser.parse_assignment()?;
    if parser.pos < parser.tokens.len() {
        return Err(anyhow!(
            "arithmetic: unexpected token at position {}",
            parser.pos
        ));
    }
    // Apply any pending assignments
    for (name, value) in parser.pending_assignments.clone() {
        runtime.set_variable(name, value.to_string());
    }
    Ok(result)
}

#[derive(Debug, Clone, PartialEq)]
enum ArithToken {
    Number(i64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    LParen,
    RParen,
    Lt,
    Gt,
    Le,
    Ge,
    EqEq,
    Ne,
    Amp,
    Pipe,
    Caret,
    Tilde,
    Shl,
    Shr,
    AmpAmp,
    PipePipe,
    Bang,
    Eq,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
}

fn tokenize(input: &str) -> Result<Vec<ArithToken>> {
    let mut tokens = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let ch = bytes[i] as char;

        // Skip whitespace
        if ch.is_ascii_whitespace() {
            i += 1;
            continue;
        }

        // Numbers
        if ch.is_ascii_digit() {
            let start = i;
            while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                i += 1;
            }
            let num_str = &input[start..i];
            let num: i64 = num_str
                .parse()
                .map_err(|_| anyhow!("arithmetic: invalid number '{}'", num_str))?;
            tokens.push(ArithToken::Number(num));
            continue;
        }

        // Identifiers (variable names) - may start with $ or letter/_
        if ch == '$' || ch.is_ascii_alphabetic() || ch == '_' {
            if ch == '$' {
                i += 1;
            }
            let ident_start = i;
            while i < bytes.len()
                && ((bytes[i] as char).is_ascii_alphanumeric() || bytes[i] as char == '_')
            {
                i += 1;
            }
            if i == ident_start {
                return Err(anyhow!("arithmetic: expected identifier after '$'"));
            }
            tokens.push(ArithToken::Ident(input[ident_start..i].to_string()));
            continue;
        }

        // Two-character operators (check before single-char)
        if i + 1 < bytes.len() {
            let two = &input[i..i + 2];
            let tok = match two {
                "<=" => Some(ArithToken::Le),
                ">=" => Some(ArithToken::Ge),
                "==" => Some(ArithToken::EqEq),
                "!=" => Some(ArithToken::Ne),
                "<<" => Some(ArithToken::Shl),
                ">>" => Some(ArithToken::Shr),
                "&&" => Some(ArithToken::AmpAmp),
                "||" => Some(ArithToken::PipePipe),
                "+=" => Some(ArithToken::PlusEq),
                "-=" => Some(ArithToken::MinusEq),
                "*=" => Some(ArithToken::StarEq),
                "/=" => Some(ArithToken::SlashEq),
                "%=" => Some(ArithToken::PercentEq),
                _ => None,
            };
            if let Some(tok) = tok {
                tokens.push(tok);
                i += 2;
                continue;
            }
        }

        // Single-character operators
        let tok = match ch {
            '+' => ArithToken::Plus,
            '-' => ArithToken::Minus,
            '*' => ArithToken::Star,
            '/' => ArithToken::Slash,
            '%' => ArithToken::Percent,
            '(' => ArithToken::LParen,
            ')' => ArithToken::RParen,
            '<' => ArithToken::Lt,
            '>' => ArithToken::Gt,
            '&' => ArithToken::Amp,
            '|' => ArithToken::Pipe,
            '^' => ArithToken::Caret,
            '~' => ArithToken::Tilde,
            '!' => ArithToken::Bang,
            '=' => ArithToken::Eq,
            _ => {
                return Err(anyhow!("arithmetic: unexpected character '{}'", ch));
            }
        };
        tokens.push(tok);
        i += 1;
    }

    Ok(tokens)
}

struct ArithParser<'a> {
    tokens: &'a [ArithToken],
    pos: usize,
    runtime: &'a Runtime,
    pending_assignments: Vec<(String, i64)>,
}

impl<'a> ArithParser<'a> {
    fn new(tokens: &'a [ArithToken], runtime: &'a Runtime) -> Self {
        Self {
            tokens,
            pos: 0,
            runtime,
            pending_assignments: Vec::new(),
        }
    }

    fn peek(&self) -> Option<&ArithToken> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&ArithToken> {
        if self.pos < self.tokens.len() {
            let tok = &self.tokens[self.pos];
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    fn expect(&mut self, expected: &ArithToken) -> Result<()> {
        if self.peek() == Some(expected) {
            self.advance();
            Ok(())
        } else {
            Err(anyhow!(
                "arithmetic: expected {:?}, found {:?}",
                expected,
                self.peek()
            ))
        }
    }

    fn var_value(&self, name: &str) -> i64 {
        // Check pending assignments first (for chained assignments)
        for (n, v) in self.pending_assignments.iter().rev() {
            if n == name {
                return *v;
            }
        }
        self.runtime
            .get_variable(name)
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0)
    }

    // Precedence levels (lowest to highest):
    // 1. Assignment: = += -= *= /= %=
    // 2. Logical OR: ||
    // 3. Logical AND: &&
    // 4. Bitwise OR: |
    // 5. Bitwise XOR: ^
    // 6. Bitwise AND: &
    // 7. Equality: == !=
    // 8. Relational: < > <= >=
    // 9. Shift: << >>
    // 10. Additive: + -
    // 11. Multiplicative: * / %
    // 12. Unary: - + ! ~

    fn parse_assignment(&mut self) -> Result<i64> {
        // Check if this is an assignment: IDENT (= | += | -= | *= | /= | %=) expr
        if let Some(ArithToken::Ident(name)) = self.peek().cloned() {
            let saved_pos = self.pos;
            self.advance();

            if let Some(op) = self.peek().cloned() {
                match op {
                    ArithToken::Eq => {
                        self.advance();
                        let value = self.parse_assignment()?; // Right-associative
                        self.pending_assignments.push((name, value));
                        return Ok(value);
                    }
                    ArithToken::PlusEq => {
                        self.advance();
                        let rhs = self.parse_assignment()?;
                        let value = self.var_value(&name) + rhs;
                        self.pending_assignments.push((name, value));
                        return Ok(value);
                    }
                    ArithToken::MinusEq => {
                        self.advance();
                        let rhs = self.parse_assignment()?;
                        let value = self.var_value(&name) - rhs;
                        self.pending_assignments.push((name, value));
                        return Ok(value);
                    }
                    ArithToken::StarEq => {
                        self.advance();
                        let rhs = self.parse_assignment()?;
                        let value = self.var_value(&name) * rhs;
                        self.pending_assignments.push((name, value));
                        return Ok(value);
                    }
                    ArithToken::SlashEq => {
                        self.advance();
                        let rhs = self.parse_assignment()?;
                        if rhs == 0 {
                            return Err(anyhow!("arithmetic: division by zero"));
                        }
                        let value = self.var_value(&name) / rhs;
                        self.pending_assignments.push((name, value));
                        return Ok(value);
                    }
                    ArithToken::PercentEq => {
                        self.advance();
                        let rhs = self.parse_assignment()?;
                        if rhs == 0 {
                            return Err(anyhow!("arithmetic: division by zero"));
                        }
                        let value = self.var_value(&name) % rhs;
                        self.pending_assignments.push((name, value));
                        return Ok(value);
                    }
                    _ => {
                        // Not an assignment, backtrack
                        self.pos = saved_pos;
                    }
                }
            } else {
                // End of tokens after ident, backtrack to let logical_or handle it
                self.pos = saved_pos;
            }
        }

        self.parse_logical_or()
    }

    fn parse_logical_or(&mut self) -> Result<i64> {
        let mut left = self.parse_logical_and()?;
        while self.peek() == Some(&ArithToken::PipePipe) {
            self.advance();
            let right = self.parse_logical_and()?;
            left = if left != 0 || right != 0 { 1 } else { 0 };
        }
        Ok(left)
    }

    fn parse_logical_and(&mut self) -> Result<i64> {
        let mut left = self.parse_bitwise_or()?;
        while self.peek() == Some(&ArithToken::AmpAmp) {
            self.advance();
            let right = self.parse_bitwise_or()?;
            left = if left != 0 && right != 0 { 1 } else { 0 };
        }
        Ok(left)
    }

    fn parse_bitwise_or(&mut self) -> Result<i64> {
        let mut left = self.parse_bitwise_xor()?;
        while self.peek() == Some(&ArithToken::Pipe) {
            self.advance();
            let right = self.parse_bitwise_xor()?;
            left |= right;
        }
        Ok(left)
    }

    fn parse_bitwise_xor(&mut self) -> Result<i64> {
        let mut left = self.parse_bitwise_and()?;
        while self.peek() == Some(&ArithToken::Caret) {
            self.advance();
            let right = self.parse_bitwise_and()?;
            left ^= right;
        }
        Ok(left)
    }

    fn parse_bitwise_and(&mut self) -> Result<i64> {
        let mut left = self.parse_equality()?;
        while self.peek() == Some(&ArithToken::Amp) {
            self.advance();
            let right = self.parse_equality()?;
            left &= right;
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<i64> {
        let mut left = self.parse_relational()?;
        loop {
            match self.peek() {
                Some(ArithToken::EqEq) => {
                    self.advance();
                    let right = self.parse_relational()?;
                    left = if left == right { 1 } else { 0 };
                }
                Some(ArithToken::Ne) => {
                    self.advance();
                    let right = self.parse_relational()?;
                    left = if left != right { 1 } else { 0 };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_relational(&mut self) -> Result<i64> {
        let mut left = self.parse_shift()?;
        loop {
            match self.peek() {
                Some(ArithToken::Lt) => {
                    self.advance();
                    let right = self.parse_shift()?;
                    left = if left < right { 1 } else { 0 };
                }
                Some(ArithToken::Gt) => {
                    self.advance();
                    let right = self.parse_shift()?;
                    left = if left > right { 1 } else { 0 };
                }
                Some(ArithToken::Le) => {
                    self.advance();
                    let right = self.parse_shift()?;
                    left = if left <= right { 1 } else { 0 };
                }
                Some(ArithToken::Ge) => {
                    self.advance();
                    let right = self.parse_shift()?;
                    left = if left >= right { 1 } else { 0 };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_shift(&mut self) -> Result<i64> {
        let mut left = self.parse_additive()?;
        loop {
            match self.peek() {
                Some(ArithToken::Shl) => {
                    self.advance();
                    let right = self.parse_additive()?;
                    left <<= right;
                }
                Some(ArithToken::Shr) => {
                    self.advance();
                    let right = self.parse_additive()?;
                    left >>= right;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<i64> {
        let mut left = self.parse_multiplicative()?;
        loop {
            match self.peek() {
                Some(ArithToken::Plus) => {
                    self.advance();
                    let right = self.parse_multiplicative()?;
                    left += right;
                }
                Some(ArithToken::Minus) => {
                    self.advance();
                    let right = self.parse_multiplicative()?;
                    left -= right;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<i64> {
        let mut left = self.parse_unary()?;
        loop {
            match self.peek() {
                Some(ArithToken::Star) => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left *= right;
                }
                Some(ArithToken::Slash) => {
                    self.advance();
                    let right = self.parse_unary()?;
                    if right == 0 {
                        return Err(anyhow!("arithmetic: division by zero"));
                    }
                    left /= right;
                }
                Some(ArithToken::Percent) => {
                    self.advance();
                    let right = self.parse_unary()?;
                    if right == 0 {
                        return Err(anyhow!("arithmetic: division by zero"));
                    }
                    left %= right;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<i64> {
        match self.peek() {
            Some(ArithToken::Minus) => {
                self.advance();
                let val = self.parse_unary()?;
                Ok(-val)
            }
            Some(ArithToken::Plus) => {
                self.advance();
                self.parse_unary()
            }
            Some(ArithToken::Bang) => {
                self.advance();
                let val = self.parse_unary()?;
                Ok(if val == 0 { 1 } else { 0 })
            }
            Some(ArithToken::Tilde) => {
                self.advance();
                let val = self.parse_unary()?;
                Ok(!val)
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<i64> {
        match self.peek().cloned() {
            Some(ArithToken::Number(n)) => {
                self.advance();
                Ok(n)
            }
            Some(ArithToken::Ident(name)) => {
                self.advance();
                Ok(self.var_value(&name))
            }
            Some(ArithToken::LParen) => {
                self.advance();
                let val = self.parse_assignment()?;
                self.expect(&ArithToken::RParen)?;
                Ok(val)
            }
            other => Err(anyhow!(
                "arithmetic: unexpected {:?} at position {}",
                other,
                self.pos
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(expr: &str) -> i64 {
        let runtime = Runtime::new();
        evaluate(expr, &runtime).unwrap()
    }

    fn eval_with_vars(expr: &str, vars: &[(&str, &str)]) -> i64 {
        let mut runtime = Runtime::new();
        for (name, val) in vars {
            runtime.set_variable(name.to_string(), val.to_string());
        }
        evaluate(expr, &runtime).unwrap()
    }

    #[test]
    fn test_basic_addition() {
        assert_eq!(eval("1+2"), 3);
    }

    #[test]
    fn test_basic_subtraction() {
        assert_eq!(eval("10-3"), 7);
    }

    #[test]
    fn test_multiplication() {
        assert_eq!(eval("3*4"), 12);
    }

    #[test]
    fn test_division() {
        assert_eq!(eval("20/4"), 5);
    }

    #[test]
    fn test_modulo() {
        assert_eq!(eval("17%5"), 2);
    }

    #[test]
    fn test_precedence() {
        assert_eq!(eval("2+3*4"), 14);
        assert_eq!(eval("(2+3)*4"), 20);
    }

    #[test]
    fn test_parentheses() {
        assert_eq!(eval("(2+3)*4"), 20);
        assert_eq!(eval("((1+2))"), 3);
    }

    #[test]
    fn test_comparison_gt() {
        assert_eq!(eval("5>3"), 1);
        assert_eq!(eval("3>5"), 0);
    }

    #[test]
    fn test_comparison_lt() {
        assert_eq!(eval("5<3"), 0);
        assert_eq!(eval("3<5"), 1);
    }

    #[test]
    fn test_comparison_eq() {
        assert_eq!(eval("5==5"), 1);
        assert_eq!(eval("5==3"), 0);
    }

    #[test]
    fn test_comparison_ne() {
        assert_eq!(eval("5!=3"), 1);
        assert_eq!(eval("5!=5"), 0);
    }

    #[test]
    fn test_comparison_le_ge() {
        assert_eq!(eval("5>=5"), 1);
        assert_eq!(eval("5>=3"), 1);
        assert_eq!(eval("3>=5"), 0);
        assert_eq!(eval("5<=5"), 1);
        assert_eq!(eval("3<=5"), 1);
        assert_eq!(eval("5<=3"), 0);
    }

    #[test]
    fn test_logical_and() {
        assert_eq!(eval("1&&1"), 1);
        assert_eq!(eval("1&&0"), 0);
        assert_eq!(eval("0&&1"), 0);
    }

    #[test]
    fn test_logical_or() {
        assert_eq!(eval("1||0"), 1);
        assert_eq!(eval("0||1"), 1);
        assert_eq!(eval("0||0"), 0);
    }

    #[test]
    fn test_logical_not() {
        assert_eq!(eval("!0"), 1);
        assert_eq!(eval("!1"), 0);
        assert_eq!(eval("!5"), 0);
    }

    #[test]
    fn test_bitwise_and() {
        assert_eq!(eval("12&10"), 8);
    }

    #[test]
    fn test_bitwise_or() {
        assert_eq!(eval("12|10"), 14);
    }

    #[test]
    fn test_bitwise_xor() {
        assert_eq!(eval("12^10"), 6);
    }

    #[test]
    fn test_bitwise_not() {
        assert_eq!(eval("~0"), -1);
    }

    #[test]
    fn test_shift() {
        assert_eq!(eval("1<<4"), 16);
        assert_eq!(eval("16>>2"), 4);
    }

    #[test]
    fn test_unary_minus() {
        assert_eq!(eval("-5"), -5);
        assert_eq!(eval("-(3+2)"), -5);
    }

    #[test]
    fn test_unary_plus() {
        assert_eq!(eval("+5"), 5);
    }

    #[test]
    fn test_variable_reference() {
        assert_eq!(eval_with_vars("x+3", &[("x", "5")]), 8);
    }

    #[test]
    fn test_variable_with_dollar() {
        assert_eq!(eval_with_vars("$x+3", &[("x", "5")]), 8);
    }

    #[test]
    fn test_unset_variable_is_zero() {
        assert_eq!(eval("x+3"), 3);
    }

    #[test]
    fn test_non_numeric_variable_is_zero() {
        assert_eq!(eval_with_vars("x+3", &[("x", "hello")]), 3);
    }

    #[test]
    fn test_whitespace() {
        assert_eq!(eval(" 1 + 2 "), 3);
        assert_eq!(eval("  5 > 3  "), 1);
    }

    #[test]
    fn test_assignment() {
        let mut runtime = Runtime::new();
        let result = evaluate_mut("x = 5", &mut runtime).unwrap();
        assert_eq!(result, 5);
        assert_eq!(runtime.get_variable("x"), Some("5".to_string()));
    }

    #[test]
    fn test_compound_assignment() {
        let mut runtime = Runtime::new();
        runtime.set_variable("x".to_string(), "10".to_string());
        let result = evaluate_mut("x += 5", &mut runtime).unwrap();
        assert_eq!(result, 15);
        assert_eq!(runtime.get_variable("x"), Some("15".to_string()));
    }

    #[test]
    fn test_complex_expression() {
        assert_eq!(eval("(2 + 3) * 4 - 1"), 19);
        assert_eq!(eval("10 / 2 + 3 * 4"), 17);
    }

    #[test]
    fn test_division_by_zero() {
        let runtime = Runtime::new();
        assert!(evaluate("1/0", &runtime).is_err());
        assert!(evaluate("1%0", &runtime).is_err());
    }
}
