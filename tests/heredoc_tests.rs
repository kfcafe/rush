use rush::executor::Executor;
use rush::lexer::{HereDocData, Lexer, Token};
use rush::parser::ast::*;
use rush::parser::Parser;

// ──────────────────────────────────────────────
// Lexer tests
// ──────────────────────────────────────────────

#[test]
fn test_lexer_heredoc_basic() {
    let input = "cat <<EOF\nhello world\nEOF";
    let tokens = Lexer::tokenize(input).unwrap();

    // Should contain: Identifier("cat"), HereDocBody(...)
    let heredoc_body = tokens.iter().find(|t| matches!(t, Token::HereDocBody(_)));
    assert!(heredoc_body.is_some(), "Should have a HereDocBody token");

    if let Some(Token::HereDocBody(data)) = heredoc_body {
        assert_eq!(data.body, "hello world\n");
        assert!(data.expand_vars, "Unquoted delimiter should expand vars");
        assert!(!data.strip_tabs, "Basic heredoc should not strip tabs");
    }
}

#[test]
fn test_lexer_heredoc_multiline() {
    let input = "cat <<EOF\nline one\nline two\nline three\nEOF";
    let tokens = Lexer::tokenize(input).unwrap();

    let heredoc_body = tokens.iter().find(|t| matches!(t, Token::HereDocBody(_)));
    assert!(heredoc_body.is_some());

    if let Some(Token::HereDocBody(data)) = heredoc_body {
        assert_eq!(data.body, "line one\nline two\nline three\n");
    }
}

#[test]
fn test_lexer_heredoc_quoted_single() {
    let input = "cat <<'EOF'\nhello $VAR\nEOF";
    let tokens = Lexer::tokenize(input).unwrap();

    let heredoc_body = tokens.iter().find(|t| matches!(t, Token::HereDocBody(_)));
    assert!(heredoc_body.is_some());

    if let Some(Token::HereDocBody(data)) = heredoc_body {
        assert_eq!(data.body, "hello $VAR\n");
        assert!(
            !data.expand_vars,
            "Single-quoted delimiter should NOT expand vars"
        );
    }
}

#[test]
fn test_lexer_heredoc_quoted_double() {
    let input = "cat <<\"EOF\"\nhello $VAR\nEOF";
    let tokens = Lexer::tokenize(input).unwrap();

    let heredoc_body = tokens.iter().find(|t| matches!(t, Token::HereDocBody(_)));
    assert!(heredoc_body.is_some());

    if let Some(Token::HereDocBody(data)) = heredoc_body {
        assert_eq!(data.body, "hello $VAR\n");
        assert!(
            !data.expand_vars,
            "Double-quoted delimiter should NOT expand vars"
        );
    }
}

#[test]
fn test_lexer_heredoc_strip_tabs() {
    let input = "cat <<-EOF\n\thello\n\tworld\n\tEOF";
    let tokens = Lexer::tokenize(input).unwrap();

    let heredoc_body = tokens.iter().find(|t| matches!(t, Token::HereDocBody(_)));
    assert!(heredoc_body.is_some());

    if let Some(Token::HereDocBody(data)) = heredoc_body {
        assert_eq!(data.body, "hello\nworld\n");
        assert!(data.strip_tabs, "<<- should strip tabs");
    }
}

#[test]
fn test_lexer_heredoc_empty_body() {
    let input = "cat <<EOF\nEOF";
    let tokens = Lexer::tokenize(input).unwrap();

    let heredoc_body = tokens.iter().find(|t| matches!(t, Token::HereDocBody(_)));
    assert!(heredoc_body.is_some());

    if let Some(Token::HereDocBody(data)) = heredoc_body {
        assert_eq!(data.body, "");
    }
}

// ──────────────────────────────────────────────
// Parser tests
// ──────────────────────────────────────────────

#[test]
fn test_parser_heredoc_creates_redirect() {
    let input = "cat <<EOF\nhello\nEOF";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let stmts = parser.parse().unwrap();

    assert_eq!(stmts.len(), 1);
    if let Statement::Command(cmd) = &stmts[0] {
        assert_eq!(cmd.name, "cat");
        assert_eq!(cmd.redirects.len(), 1);
        assert_eq!(cmd.redirects[0].kind, RedirectKind::HereDoc);
        assert_eq!(cmd.redirects[0].target, Some("hello\n".to_string()));
    } else {
        panic!("Expected Command statement");
    }
}

#[test]
fn test_parser_heredoc_literal_creates_redirect() {
    let input = "cat <<'EOF'\nhello $VAR\nEOF";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let stmts = parser.parse().unwrap();

    assert_eq!(stmts.len(), 1);
    if let Statement::Command(cmd) = &stmts[0] {
        assert_eq!(cmd.redirects.len(), 1);
        assert_eq!(cmd.redirects[0].kind, RedirectKind::HereDocLiteral);
        assert_eq!(cmd.redirects[0].target, Some("hello $VAR\n".to_string()));
    } else {
        panic!("Expected Command statement");
    }
}

// ──────────────────────────────────────────────
// Executor tests (external commands with heredoc stdin)
// ──────────────────────────────────────────────

#[test]
fn test_executor_heredoc_basic_cat() {
    let input = "cat <<EOF\nhello world\nEOF";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let stmts = parser.parse().unwrap();

    let mut executor = Executor::new_embedded();
    let result = executor.execute(stmts).unwrap();
    assert_eq!(result.stdout().trim(), "hello world");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_executor_heredoc_multiline_cat() {
    let input = "cat <<EOF\nline one\nline two\nline three\nEOF";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let stmts = parser.parse().unwrap();

    let mut executor = Executor::new_embedded();
    let result = executor.execute(stmts).unwrap();
    assert_eq!(result.stdout(), "line one\nline two\nline three\n");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_executor_heredoc_literal_no_expansion() {
    // Quoted delimiter should NOT expand variables
    let input = "cat <<'EOF'\nhello $HOME\nEOF";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let stmts = parser.parse().unwrap();

    let mut executor = Executor::new_embedded();
    let result = executor.execute(stmts).unwrap();
    // $HOME should NOT be expanded -- literal output
    assert_eq!(result.stdout().trim(), "hello $HOME");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_executor_heredoc_strip_tabs() {
    let input = "cat <<-EOF\n\thello\n\t\tindented\n\tEOF";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let stmts = parser.parse().unwrap();

    let mut executor = Executor::new_embedded();
    let result = executor.execute(stmts).unwrap();
    // <<- strips ALL leading tabs from each body line (POSIX)
    assert_eq!(result.stdout(), "hello\nindented\n");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_executor_heredoc_empty_body() {
    let input = "cat <<EOF\nEOF";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let stmts = parser.parse().unwrap();

    let mut executor = Executor::new_embedded();
    let result = executor.execute(stmts).unwrap();
    assert_eq!(result.stdout(), "");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_executor_heredoc_with_wc() {
    // heredoc piped to wc -l should count lines
    let input = "wc -l <<EOF\nfirst\nsecond\nthird\nEOF";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let stmts = parser.parse().unwrap();

    let mut executor = Executor::new_embedded();
    let result = executor.execute(stmts).unwrap();
    // wc -l should report 3 lines
    assert!(
        result.stdout().trim().contains("3"),
        "Expected 3 lines, got: {}",
        result.stdout()
    );
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_executor_heredoc_variable_expansion() {
    // Unquoted delimiter should expand variables
    let input = "cat <<EOF\nhello $USER\nEOF";
    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let stmts = parser.parse().unwrap();

    let mut executor = Executor::new_embedded();
    // Set a variable in the runtime
    executor
        .runtime_mut()
        .set_variable("USER".to_string(), "testuser".to_string());
    let result = executor.execute(stmts).unwrap();
    assert_eq!(result.stdout().trim(), "hello testuser");
    assert_eq!(result.exit_code, 0);
}
