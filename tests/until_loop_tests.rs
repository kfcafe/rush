use rush::executor::Executor;
use rush::lexer::Lexer;
use rush::parser::Parser;

#[test]
fn test_basic_until_loop() {
    let mut executor = Executor::new();
    executor
        .runtime_mut()
        .set_variable("i".to_string(), "0".to_string());

    let input = r#"
        i=0
        until [ $i -ge 5 ]; do
            echo $i
            i=$((i+1))
        done
    "#;

    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout().trim(), "0\n1\n2\n3\n4");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_until_with_break() {
    let mut executor = Executor::new();

    let input = r#"
        i=0
        until false; do
            echo $i
            i=$((i+1))
            if [ $i -eq 3 ]; then
                break
            fi
        done
    "#;

    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout().trim(), "0\n1\n2");
}

#[test]
fn test_until_with_continue() {
    let mut executor = Executor::new();

    let input = r#"
        i=0
        until [ $i -ge 5 ]; do
            i=$((i+1))
            if [ $i -eq 3 ]; then
                continue
            fi
            echo $i
        done
    "#;

    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    // Should skip printing 3
    assert_eq!(result.stdout().trim(), "1\n2\n4\n5");
}

#[test]
fn test_nested_until_loops() {
    let mut executor = Executor::new();

    let input = r#"
        i=0
        until [ $i -ge 2 ]; do
            j=0
            until [ $j -ge 2 ]; do
                echo "$i,$j"
                j=$((j+1))
            done
            i=$((i+1))
        done
    "#;

    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout().trim(), "0,0\n0,1\n1,0\n1,1");
}

#[test]
fn test_until_mixed_with_while() {
    let mut executor = Executor::new();

    let input = r#"
        i=0
        until [ $i -ge 2 ]; do
            j=0
            while [ $j -lt 2 ]; do
                echo "$i,$j"
                j=$((j+1))
            done
            i=$((i+1))
        done
    "#;

    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout().trim(), "0,0\n0,1\n1,0\n1,1");
}

#[test]
fn test_until_exit_code_propagation() {
    let mut executor = Executor::new();

    let input = r#"
        i=0
        until [ $i -ge 3 ]; do
            echo $i
            i=$((i+1))
        done
    "#;

    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    // Exit code should be 0 (from the last executed command in the last iteration)
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_until_never_executes() {
    let mut executor = Executor::new();

    let input = r#"
        until true; do
            echo "should not print"
        done
        echo "after loop"
    "#;

    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    // Body should never execute since condition is true immediately
    assert_eq!(result.stdout().trim(), "after loop");
}

#[test]
fn test_until_complex_condition() {
    let mut executor = Executor::new();

    let input = r#"
        i=0
        until [ $i -eq 3 ] && [ $i -gt 2 ]; do
            echo $i
            i=$((i+1))
        done
    "#;

    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    // Loop should execute until i equals 3 AND i is greater than 2 (both true when i=3)
    assert_eq!(result.stdout().trim(), "0\n1\n2");
}

#[test]
fn test_basic_while_loop() {
    let mut executor = Executor::new();

    let input = r#"
        i=0
        while [ $i -lt 3 ]; do
            echo $i
            i=$((i+1))
        done
    "#;

    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout().trim(), "0\n1\n2");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_while_with_break() {
    let mut executor = Executor::new();

    let input = r#"
        i=0
        while true; do
            echo $i
            i=$((i+1))
            if [ $i -eq 3 ]; then
                break
            fi
        done
    "#;

    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout().trim(), "0\n1\n2");
}

#[test]
fn test_while_never_executes() {
    let mut executor = Executor::new();

    let input = r#"
        while false; do
            echo "should not print"
        done
        echo "after loop"
    "#;

    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout().trim(), "after loop");
}

#[test]
fn test_nested_while_loops() {
    let mut executor = Executor::new();

    let input = r#"
        i=0
        while [ $i -lt 2 ]; do
            j=0
            while [ $j -lt 2 ]; do
                echo "$i,$j"
                j=$((j+1))
            done
            i=$((i+1))
        done
    "#;

    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    assert_eq!(result.stdout().trim(), "0,0\n0,1\n1,0\n1,1");
}

#[test]
fn test_until_loop_exit_code_when_never_executed() {
    let mut executor = Executor::new();

    let input = r#"
        until true; do
            false
        done
    "#;

    let tokens = Lexer::tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().unwrap();
    let result = executor.execute(statements).unwrap();

    // Exit code should be 0 since body never executed
    assert_eq!(result.exit_code, 0);
}
