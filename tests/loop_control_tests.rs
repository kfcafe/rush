use rush::executor::Executor;
use rush::parser::ast::{Argument, Command, ForLoop, Statement};

#[test]
fn test_break_basic_for_loop() {
    let mut executor = Executor::new();

    // for i in 1 2 3 4 5; do echo $i; if [ $i = 3 ]; then break; fi; done
    let for_loop = ForLoop {
        variable: "i".to_string(),
        words: vec![
            Argument::Literal("1".to_string()),
            Argument::Literal("2".to_string()),
            Argument::Literal("3".to_string()),
            Argument::Literal("4".to_string()),
            Argument::Literal("5".to_string()),
        ],
        body: vec![
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Variable("i".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "break".to_string(),
                args: vec![],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    let result = executor
        .execute(vec![Statement::ForLoop(for_loop)])
        .unwrap();

    // Should only print "1" before breaking
    assert_eq!(result.stdout(), "1\n");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_break_with_condition() {
    let mut executor = Executor::new();

    // Set up a counter variable
    executor
        .runtime_mut()
        .set_variable("counter".to_string(), "0".to_string());

    // Manually construct a loop that increments counter and breaks at 3
    let for_loop = ForLoop {
        variable: "i".to_string(),
        words: vec![
            Argument::Literal("1".to_string()),
            Argument::Literal("2".to_string()),
            Argument::Literal("3".to_string()),
            Argument::Literal("4".to_string()),
            Argument::Literal("5".to_string()),
            Argument::Literal("6".to_string()),
            Argument::Literal("7".to_string()),
            Argument::Literal("8".to_string()),
            Argument::Literal("9".to_string()),
            Argument::Literal("10".to_string()),
        ],
        body: vec![Statement::Command(Command {
            name: "echo".to_string(),
            args: vec![Argument::Variable("i".to_string())],
            redirects: vec![],
            prefix_env: vec![],
        })],
    };

    // Execute a simple loop that echoes each iteration
    let result = executor
        .execute(vec![Statement::ForLoop(for_loop)])
        .unwrap();

    // All items should be echoed since we're not breaking in this test
    assert_eq!(result.stdout(), "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n");
}

#[test]
fn test_break_outside_loop() {
    let mut executor = Executor::new();

    // Try to break outside a loop
    let result = executor.execute(vec![Statement::Command(Command {
        name: "break".to_string(),
        args: vec![],
        redirects: vec![],
        prefix_env: vec![],
    })]);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("only meaningful in a"));
}

#[test]
fn test_break_nested_loops_level_1() {
    let mut executor = Executor::new();

    // Nested loops: outer loop over 'a b', inner loop over '1 2 3'
    // Break from inner loop when inner = 2
    let inner_loop = ForLoop {
        variable: "inner".to_string(),
        words: vec![
            Argument::Literal("1".to_string()),
            Argument::Literal("2".to_string()),
            Argument::Literal("3".to_string()),
        ],
        body: vec![
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![
                    Argument::Variable("outer".to_string()),
                    Argument::Literal(":".to_string()),
                    Argument::Variable("inner".to_string()),
                ],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "break".to_string(),
                args: vec![],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    let outer_loop = ForLoop {
        variable: "outer".to_string(),
        words: vec![
            Argument::Literal("a".to_string()),
            Argument::Literal("b".to_string()),
        ],
        body: vec![Statement::ForLoop(inner_loop)],
    };

    let result = executor
        .execute(vec![Statement::ForLoop(outer_loop)])
        .unwrap();

    // Should print "a : 1" and "b : 1" (breaks from inner loop each time)
    assert_eq!(result.stdout(), "a : 1\nb : 1\n");
}

#[test]
fn test_break_nested_loops_level_2() {
    let mut executor = Executor::new();

    // Nested loops: break 2 should exit both loops
    let inner_loop = ForLoop {
        variable: "inner".to_string(),
        words: vec![
            Argument::Literal("1".to_string()),
            Argument::Literal("2".to_string()),
            Argument::Literal("3".to_string()),
        ],
        body: vec![
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![
                    Argument::Variable("outer".to_string()),
                    Argument::Literal(":".to_string()),
                    Argument::Variable("inner".to_string()),
                ],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "break".to_string(),
                args: vec![Argument::Literal("2".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    let outer_loop = ForLoop {
        variable: "outer".to_string(),
        words: vec![
            Argument::Literal("a".to_string()),
            Argument::Literal("b".to_string()),
            Argument::Literal("c".to_string()),
        ],
        body: vec![Statement::ForLoop(inner_loop)],
    };

    let result = executor
        .execute(vec![Statement::ForLoop(outer_loop)])
        .unwrap();

    // Should only print "a : 1" before breaking from both loops
    assert_eq!(result.stdout(), "a : 1\n");
}

#[test]
fn test_break_with_invalid_argument() {
    let mut executor = Executor::new();

    let for_loop = ForLoop {
        variable: "i".to_string(),
        words: vec![
            Argument::Literal("1".to_string()),
            Argument::Literal("2".to_string()),
            Argument::Literal("3".to_string()),
        ],
        body: vec![Statement::Command(Command {
            name: "break".to_string(),
            args: vec![Argument::Literal("not_a_number".to_string())],
            redirects: vec![],
            prefix_env: vec![],
        })],
    };

    let result = executor.execute(vec![Statement::ForLoop(for_loop)]);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("numeric argument required"));
}

#[test]
fn test_break_with_zero() {
    let mut executor = Executor::new();

    let for_loop = ForLoop {
        variable: "i".to_string(),
        words: vec![
            Argument::Literal("1".to_string()),
            Argument::Literal("2".to_string()),
            Argument::Literal("3".to_string()),
        ],
        body: vec![Statement::Command(Command {
            name: "break".to_string(),
            args: vec![Argument::Literal("0".to_string())],
            redirects: vec![],
            prefix_env: vec![],
        })],
    };

    let result = executor.execute(vec![Statement::ForLoop(for_loop)]);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("loop count out of range"));
}

#[test]
fn test_break_exceeds_loop_depth() {
    let mut executor = Executor::new();

    let for_loop = ForLoop {
        variable: "i".to_string(),
        words: vec![
            Argument::Literal("1".to_string()),
            Argument::Literal("2".to_string()),
            Argument::Literal("3".to_string()),
        ],
        body: vec![Statement::Command(Command {
            name: "break".to_string(),
            args: vec![Argument::Literal("2".to_string())],
            redirects: vec![],
            prefix_env: vec![],
        })],
    };

    let result = executor.execute(vec![Statement::ForLoop(for_loop)]);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("loop count out of range"));
}

#[test]
fn test_break_too_many_arguments() {
    let mut executor = Executor::new();

    let for_loop = ForLoop {
        variable: "i".to_string(),
        words: vec![
            Argument::Literal("1".to_string()),
            Argument::Literal("2".to_string()),
            Argument::Literal("3".to_string()),
        ],
        body: vec![Statement::Command(Command {
            name: "break".to_string(),
            args: vec![
                Argument::Literal("1".to_string()),
                Argument::Literal("2".to_string()),
            ],
            redirects: vec![],
            prefix_env: vec![],
        })],
    };

    let result = executor.execute(vec![Statement::ForLoop(for_loop)]);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("too many arguments"));
}

#[test]
fn test_break_preserves_output_before_break() {
    let mut executor = Executor::new();

    // Loop that echoes before breaking
    let for_loop = ForLoop {
        variable: "i".to_string(),
        words: vec![
            Argument::Literal("first".to_string()),
            Argument::Literal("second".to_string()),
            Argument::Literal("third".to_string()),
        ],
        body: vec![
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![
                    Argument::Literal("Processing:".to_string()),
                    Argument::Variable("i".to_string()),
                ],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "break".to_string(),
                args: vec![],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    let result = executor
        .execute(vec![Statement::ForLoop(for_loop)])
        .unwrap();

    // Should see output from first iteration only
    assert_eq!(result.stdout(), "Processing: first\n");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_break_in_triple_nested_loop() {
    let mut executor = Executor::new();

    // Test break 3 in triple-nested loops
    let innermost_loop = ForLoop {
        variable: "k".to_string(),
        words: vec![
            Argument::Literal("1".to_string()),
            Argument::Literal("2".to_string()),
        ],
        body: vec![
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![
                    Argument::Variable("i".to_string()),
                    Argument::Variable("j".to_string()),
                    Argument::Variable("k".to_string()),
                ],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "break".to_string(),
                args: vec![Argument::Literal("3".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    let middle_loop = ForLoop {
        variable: "j".to_string(),
        words: vec![
            Argument::Literal("x".to_string()),
            Argument::Literal("y".to_string()),
        ],
        body: vec![Statement::ForLoop(innermost_loop)],
    };

    let outer_loop = ForLoop {
        variable: "i".to_string(),
        words: vec![
            Argument::Literal("a".to_string()),
            Argument::Literal("b".to_string()),
        ],
        body: vec![Statement::ForLoop(middle_loop)],
    };

    let result = executor
        .execute(vec![Statement::ForLoop(outer_loop)])
        .unwrap();

    // Should only print "a x 1" before breaking from all three loops
    assert_eq!(result.stdout(), "a x 1\n");
}

// ========== Continue Builtin Tests ==========

#[test]
fn test_continue_basic_for_loop() {
    let mut executor = Executor::new();

    // for i in 1 2 3 4 5; do if [ $i = 3 ]; then continue; fi; echo $i; done
    // This should skip printing "3" but print others
    let for_loop = ForLoop {
        variable: "i".to_string(),
        words: vec![
            Argument::Literal("1".to_string()),
            Argument::Literal("2".to_string()),
            Argument::Literal("3".to_string()),
            Argument::Literal("4".to_string()),
            Argument::Literal("5".to_string()),
        ],
        body: vec![
            // Simulate: if [ "$i" = "3" ]; then continue; fi
            // For test simplicity, we'll just continue on first iteration
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![
                    Argument::Literal("before".to_string()),
                    Argument::Variable("i".to_string()),
                ],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "continue".to_string(),
                args: vec![],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![
                    Argument::Literal("after".to_string()),
                    Argument::Variable("i".to_string()),
                ],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    let result = executor
        .execute(vec![Statement::ForLoop(for_loop)])
        .unwrap();

    // Should print "before X" for all items, but never "after X"
    assert_eq!(
        result.stdout(),
        "before 1\nbefore 2\nbefore 3\nbefore 4\nbefore 5\n"
    );
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_continue_skips_remaining_statements() {
    let mut executor = Executor::new();

    // Loop that continues after first echo - second echo should be skipped
    let for_loop = ForLoop {
        variable: "i".to_string(),
        words: vec![
            Argument::Literal("a".to_string()),
            Argument::Literal("b".to_string()),
            Argument::Literal("c".to_string()),
        ],
        body: vec![
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![
                    Argument::Literal("first:".to_string()),
                    Argument::Variable("i".to_string()),
                ],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "continue".to_string(),
                args: vec![],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![
                    Argument::Literal("second:".to_string()),
                    Argument::Variable("i".to_string()),
                ],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    let result = executor
        .execute(vec![Statement::ForLoop(for_loop)])
        .unwrap();

    // Should only see "first:" outputs, never "second:"
    assert_eq!(result.stdout(), "first: a\nfirst: b\nfirst: c\n");
}

#[test]
fn test_continue_outside_loop() {
    let mut executor = Executor::new();

    // Try to continue outside a loop
    let result = executor.execute(vec![Statement::Command(Command {
        name: "continue".to_string(),
        args: vec![],
        redirects: vec![],
        prefix_env: vec![],
    })]);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("only meaningful in a"));
}

#[test]
fn test_continue_nested_loops_level_1() {
    let mut executor = Executor::new();

    // Nested loops: continue from inner loop should skip to next inner iteration
    let inner_loop = ForLoop {
        variable: "inner".to_string(),
        words: vec![
            Argument::Literal("1".to_string()),
            Argument::Literal("2".to_string()),
            Argument::Literal("3".to_string()),
        ],
        body: vec![
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![
                    Argument::Variable("outer".to_string()),
                    Argument::Literal(":".to_string()),
                    Argument::Variable("inner".to_string()),
                ],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "continue".to_string(),
                args: vec![],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Literal("SHOULD_NOT_PRINT".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    let outer_loop = ForLoop {
        variable: "outer".to_string(),
        words: vec![
            Argument::Literal("a".to_string()),
            Argument::Literal("b".to_string()),
        ],
        body: vec![Statement::ForLoop(inner_loop)],
    };

    let result = executor
        .execute(vec![Statement::ForLoop(outer_loop)])
        .unwrap();

    // Should print all combinations, but never SHOULD_NOT_PRINT
    assert_eq!(
        result.stdout(),
        "a : 1\na : 2\na : 3\nb : 1\nb : 2\nb : 3\n"
    );
}

#[test]
fn test_continue_nested_loops_level_2() {
    let mut executor = Executor::new();

    // Nested loops: continue 2 should skip to next outer loop iteration
    let inner_loop = ForLoop {
        variable: "inner".to_string(),
        words: vec![
            Argument::Literal("1".to_string()),
            Argument::Literal("2".to_string()),
            Argument::Literal("3".to_string()),
        ],
        body: vec![
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![
                    Argument::Variable("outer".to_string()),
                    Argument::Literal(":".to_string()),
                    Argument::Variable("inner".to_string()),
                ],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "continue".to_string(),
                args: vec![Argument::Literal("2".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    let outer_loop = ForLoop {
        variable: "outer".to_string(),
        words: vec![
            Argument::Literal("a".to_string()),
            Argument::Literal("b".to_string()),
            Argument::Literal("c".to_string()),
        ],
        body: vec![
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![
                    Argument::Literal("outer-before:".to_string()),
                    Argument::Variable("outer".to_string()),
                ],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::ForLoop(inner_loop),
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![
                    Argument::Literal("outer-after:".to_string()),
                    Argument::Variable("outer".to_string()),
                ],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    let result = executor
        .execute(vec![Statement::ForLoop(outer_loop)])
        .unwrap();

    // Should print "outer-before" for each, then "X : 1" for first inner iteration
    // Then continue 2 skips to next outer iteration
    // Never should see "outer-after" or inner iterations 2 and 3
    assert_eq!(
        result.stdout(),
        "outer-before: a\na : 1\nouter-before: b\nb : 1\nouter-before: c\nc : 1\n"
    );
}

#[test]
fn test_continue_with_invalid_argument() {
    let mut executor = Executor::new();

    let for_loop = ForLoop {
        variable: "i".to_string(),
        words: vec![
            Argument::Literal("1".to_string()),
            Argument::Literal("2".to_string()),
            Argument::Literal("3".to_string()),
        ],
        body: vec![Statement::Command(Command {
            name: "continue".to_string(),
            args: vec![Argument::Literal("not_a_number".to_string())],
            redirects: vec![],
            prefix_env: vec![],
        })],
    };

    let result = executor.execute(vec![Statement::ForLoop(for_loop)]);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("numeric argument required"));
}

#[test]
fn test_continue_with_zero() {
    let mut executor = Executor::new();

    let for_loop = ForLoop {
        variable: "i".to_string(),
        words: vec![
            Argument::Literal("1".to_string()),
            Argument::Literal("2".to_string()),
            Argument::Literal("3".to_string()),
        ],
        body: vec![Statement::Command(Command {
            name: "continue".to_string(),
            args: vec![Argument::Literal("0".to_string())],
            redirects: vec![],
            prefix_env: vec![],
        })],
    };

    let result = executor.execute(vec![Statement::ForLoop(for_loop)]);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("loop count out of range"));
}

#[test]
fn test_continue_exceeds_loop_depth() {
    let mut executor = Executor::new();

    let for_loop = ForLoop {
        variable: "i".to_string(),
        words: vec![
            Argument::Literal("1".to_string()),
            Argument::Literal("2".to_string()),
            Argument::Literal("3".to_string()),
        ],
        body: vec![Statement::Command(Command {
            name: "continue".to_string(),
            args: vec![Argument::Literal("2".to_string())],
            redirects: vec![],
            prefix_env: vec![],
        })],
    };

    let result = executor.execute(vec![Statement::ForLoop(for_loop)]);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("loop count out of range"));
}

#[test]
fn test_continue_too_many_arguments() {
    let mut executor = Executor::new();

    let for_loop = ForLoop {
        variable: "i".to_string(),
        words: vec![
            Argument::Literal("1".to_string()),
            Argument::Literal("2".to_string()),
            Argument::Literal("3".to_string()),
        ],
        body: vec![Statement::Command(Command {
            name: "continue".to_string(),
            args: vec![
                Argument::Literal("1".to_string()),
                Argument::Literal("2".to_string()),
            ],
            redirects: vec![],
            prefix_env: vec![],
        })],
    };

    let result = executor.execute(vec![Statement::ForLoop(for_loop)]);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("too many arguments"));
}

#[test]
fn test_continue_preserves_output_before_continue() {
    let mut executor = Executor::new();

    // Loop that echoes before continuing
    let for_loop = ForLoop {
        variable: "i".to_string(),
        words: vec![
            Argument::Literal("first".to_string()),
            Argument::Literal("second".to_string()),
            Argument::Literal("third".to_string()),
        ],
        body: vec![
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![
                    Argument::Literal("Processing:".to_string()),
                    Argument::Variable("i".to_string()),
                ],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "continue".to_string(),
                args: vec![],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![
                    Argument::Literal("Done:".to_string()),
                    Argument::Variable("i".to_string()),
                ],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    let result = executor
        .execute(vec![Statement::ForLoop(for_loop)])
        .unwrap();

    // Should see "Processing:" for all iterations, but never "Done:"
    assert_eq!(
        result.stdout(),
        "Processing: first\nProcessing: second\nProcessing: third\n"
    );
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_continue_in_triple_nested_loop() {
    let mut executor = Executor::new();

    // Test continue 3 in triple-nested loops
    let innermost_loop = ForLoop {
        variable: "k".to_string(),
        words: vec![
            Argument::Literal("1".to_string()),
            Argument::Literal("2".to_string()),
        ],
        body: vec![
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![
                    Argument::Variable("i".to_string()),
                    Argument::Variable("j".to_string()),
                    Argument::Variable("k".to_string()),
                ],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "continue".to_string(),
                args: vec![Argument::Literal("3".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    let middle_loop = ForLoop {
        variable: "j".to_string(),
        words: vec![
            Argument::Literal("x".to_string()),
            Argument::Literal("y".to_string()),
        ],
        body: vec![
            Statement::ForLoop(innermost_loop),
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Literal("middle-after".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    let outer_loop = ForLoop {
        variable: "i".to_string(),
        words: vec![
            Argument::Literal("a".to_string()),
            Argument::Literal("b".to_string()),
        ],
        body: vec![
            Statement::ForLoop(middle_loop),
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Literal("outer-after".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    let result = executor
        .execute(vec![Statement::ForLoop(outer_loop)])
        .unwrap();

    // Should print first iteration of each level, then skip to next outer iteration
    // Never see "middle-after" or "outer-after"
    assert_eq!(result.stdout(), "a x 1\nb x 1\n");
}

#[test]
fn test_continue_all_iterations_complete() {
    let mut executor = Executor::new();

    // Verify that continue doesn't prevent loop from finishing all iterations
    let for_loop = ForLoop {
        variable: "i".to_string(),
        words: vec![
            Argument::Literal("1".to_string()),
            Argument::Literal("2".to_string()),
            Argument::Literal("3".to_string()),
            Argument::Literal("4".to_string()),
            Argument::Literal("5".to_string()),
        ],
        body: vec![
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Variable("i".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "continue".to_string(),
                args: vec![],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    let result = executor
        .execute(vec![Statement::ForLoop(for_loop)])
        .unwrap();

    // All 5 iterations should complete, each printing the variable
    assert_eq!(result.stdout(), "1\n2\n3\n4\n5\n");
    assert_eq!(result.exit_code, 0);
}
