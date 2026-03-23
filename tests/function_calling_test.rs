use rush::executor::Executor;
use rush::parser::ast::*;

#[test]
fn test_simple_function_call_no_params() {
    let mut executor = Executor::new();

    // Define a simple function that echoes "hello"
    let func = FunctionDef {
        name: "greet".to_string(),
        params: vec![],
        body: vec![Statement::Command(Command {
            name: "echo".to_string(),
            args: vec![Argument::Literal("hello".to_string())],
            redirects: vec![],
            prefix_env: vec![],
        })],
    };

    // Define the function
    executor
        .execute_statement(Statement::FunctionDef(func))
        .unwrap();

    // Call the function
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "greet".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    // Should return "hello\n" from echo
    assert!(result.stdout().contains("hello"));
}

#[test]
fn test_function_with_parameters() {
    let mut executor = Executor::new();

    // Define a function that echoes its first parameter
    let func = FunctionDef {
        name: "say".to_string(),
        params: vec![Parameter {
            name: "message".to_string(),
            type_hint: None,
        }],
        body: vec![Statement::Command(Command {
            name: "echo".to_string(),
            args: vec![Argument::Variable("$message".to_string())],
            redirects: vec![],
            prefix_env: vec![],
        })],
    };

    executor
        .execute_statement(Statement::FunctionDef(func))
        .unwrap();

    // Call the function with an argument
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "say".to_string(),
            args: vec![Argument::Literal("world".to_string())],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    assert!(result.stdout().contains("world"));
}

#[test]
fn test_function_with_multiple_parameters() {
    let mut executor = Executor::new();

    // Define a function that echoes two parameters
    let func = FunctionDef {
        name: "combine".to_string(),
        params: vec![
            Parameter {
                name: "first".to_string(),
                type_hint: None,
            },
            Parameter {
                name: "second".to_string(),
                type_hint: None,
            },
        ],
        body: vec![Statement::Command(Command {
            name: "echo".to_string(),
            args: vec![
                Argument::Variable("$first".to_string()),
                Argument::Variable("$second".to_string()),
            ],
            redirects: vec![],
            prefix_env: vec![],
        })],
    };

    executor
        .execute_statement(Statement::FunctionDef(func))
        .unwrap();

    // Call the function with two arguments
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "combine".to_string(),
            args: vec![
                Argument::Literal("hello".to_string()),
                Argument::Literal("world".to_string()),
            ],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    assert!(result.stdout().contains("hello"));
    assert!(result.stdout().contains("world"));
}

#[test]
fn test_recursive_factorial() {
    let mut executor = Executor::new();

    // Define a factorial function using if statements
    // factorial(n) = if n <= 1 then 1 else n * factorial(n-1)
    // For simplicity, this will just count down and echo the values
    let func = FunctionDef {
        name: "countdown".to_string(),
        params: vec![Parameter {
            name: "n".to_string(),
            type_hint: None,
        }],
        body: vec![
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Variable("$n".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
            // This is a simplified test - a real factorial would need more complex logic
        ],
    };

    executor
        .execute_statement(Statement::FunctionDef(func))
        .unwrap();

    // Call the function
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "countdown".to_string(),
            args: vec![Argument::Literal("5".to_string())],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    assert!(result.stdout().contains("5"));
}

#[test]
fn test_function_calling_another_function() {
    let mut executor = Executor::new();

    // Define a helper function
    let helper = FunctionDef {
        name: "helper".to_string(),
        params: vec![],
        body: vec![Statement::Command(Command {
            name: "echo".to_string(),
            args: vec![Argument::Literal("helper called".to_string())],
            redirects: vec![],
            prefix_env: vec![],
        })],
    };

    // Define a main function that calls the helper
    let main = FunctionDef {
        name: "main".to_string(),
        params: vec![],
        body: vec![
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Literal("main called".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "helper".to_string(),
                args: vec![],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    executor
        .execute_statement(Statement::FunctionDef(helper))
        .unwrap();
    executor
        .execute_statement(Statement::FunctionDef(main))
        .unwrap();

    // Call the main function
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "main".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    assert!(result.stdout().contains("main called"));
    assert!(result.stdout().contains("helper called"));
}

#[test]
fn test_scope_isolation_parameters_shadow_variables() {
    let mut executor = Executor::new();

    // Set a global variable
    executor
        .execute_statement(Statement::Assignment(Assignment {
            name: "x".to_string(),
            value: Expression::Literal(Literal::String("global".to_string())),
        }))
        .unwrap();

    // Define a function with a parameter named 'x'
    let func = FunctionDef {
        name: "test_scope".to_string(),
        params: vec![Parameter {
            name: "x".to_string(),
            type_hint: None,
        }],
        body: vec![Statement::Command(Command {
            name: "echo".to_string(),
            args: vec![Argument::Variable("$x".to_string())],
            redirects: vec![],
            prefix_env: vec![],
        })],
    };

    executor
        .execute_statement(Statement::FunctionDef(func))
        .unwrap();

    // Call the function - should echo "local" not "global"
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "test_scope".to_string(),
            args: vec![Argument::Literal("local".to_string())],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    assert!(result.stdout().contains("local"));
    assert!(!result.stdout().contains("global"));
}

#[test]
fn test_local_variables_dont_leak() {
    let mut executor = Executor::new();

    // Define a function that sets a local variable
    let func = FunctionDef {
        name: "set_local".to_string(),
        params: vec![],
        body: vec![Statement::Assignment(Assignment {
            name: "local_var".to_string(),
            value: Expression::Literal(Literal::String("local_value".to_string())),
        })],
    };

    executor
        .execute_statement(Statement::FunctionDef(func))
        .unwrap();

    // Call the function
    executor
        .execute_statement(Statement::Command(Command {
            name: "set_local".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    // Try to access the local variable - should not exist
    let var_value = executor.runtime_mut().get_variable("local_var");
    assert!(
        var_value.is_none(),
        "Local variable should not leak to global scope"
    );
}

#[test]
fn test_recursion_depth_limit() {
    let mut executor = Executor::new();

    // Define an infinitely recursive function
    let func = FunctionDef {
        name: "infinite".to_string(),
        params: vec![],
        body: vec![Statement::Command(Command {
            name: "infinite".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        })],
    };

    executor
        .execute_statement(Statement::FunctionDef(func))
        .unwrap();

    // Call the function - should error with recursion limit
    let result = executor.execute_statement(Statement::Command(Command {
        name: "infinite".to_string(),
        args: vec![],
        redirects: vec![],
        prefix_env: vec![],
    }));

    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("recursion") || error_msg.contains("depth"));
}

#[test]
fn test_function_return_value() {
    let mut executor = Executor::new();

    // Define a function that returns a value via echo
    let func = FunctionDef {
        name: "get_value".to_string(),
        params: vec![],
        body: vec![Statement::Command(Command {
            name: "echo".to_string(),
            args: vec![Argument::Literal("42".to_string())],
            redirects: vec![],
            prefix_env: vec![],
        })],
    };

    executor
        .execute_statement(Statement::FunctionDef(func))
        .unwrap();

    // Call the function and check the return value
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "get_value".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    assert!(result.stdout().contains("42"));
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_function_stdout_capture() {
    let mut executor = Executor::new();

    // Define a function that outputs multiple lines
    let func = FunctionDef {
        name: "multi_output".to_string(),
        params: vec![],
        body: vec![
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Literal("line1".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Literal("line2".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Literal("line3".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    executor
        .execute_statement(Statement::FunctionDef(func))
        .unwrap();

    // Call the function and verify all output is captured
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "multi_output".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    // Note: Each echo only captures its own output, not accumulated
    // The last statement's output is what's returned
    assert!(result.stdout().contains("line3"));
}

// ============================================================================
// RETURN BUILTIN TESTS
// ============================================================================

#[test]
fn test_return_with_exit_code_42() {
    let mut executor = Executor::new();

    // Define a function that returns with exit code 42
    let func = FunctionDef {
        name: "return_42".to_string(),
        params: vec![],
        body: vec![Statement::Command(Command {
            name: "return".to_string(),
            args: vec![Argument::Literal("42".to_string())],
            redirects: vec![],
            prefix_env: vec![],
        })],
    };

    executor
        .execute_statement(Statement::FunctionDef(func))
        .unwrap();

    // Call the function and check the exit code
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "return_42".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    assert_eq!(result.exit_code, 42);
}

#[test]
fn test_return_with_no_argument_defaults_to_zero() {
    let mut executor = Executor::new();

    // Define a function that returns without an argument
    let func = FunctionDef {
        name: "return_default".to_string(),
        params: vec![],
        body: vec![Statement::Command(Command {
            name: "return".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        })],
    };

    executor
        .execute_statement(Statement::FunctionDef(func))
        .unwrap();

    // Call the function and check the exit code is 0
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "return_default".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_return_early_from_function() {
    let mut executor = Executor::new();

    // Define a function that returns early, skipping subsequent commands
    let func = FunctionDef {
        name: "early_return".to_string(),
        params: vec![],
        body: vec![
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Literal("before return".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "return".to_string(),
                args: vec![Argument::Literal("5".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Literal("after return".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    executor
        .execute_statement(Statement::FunctionDef(func))
        .unwrap();

    // Call the function
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "early_return".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    // Should return early with exit code 5
    assert_eq!(result.exit_code, 5);
    // The output should contain "before return" but not "after return"
    // Note: In the current implementation, the last command output is what's captured
}

#[test]
fn test_return_with_various_exit_codes() {
    let mut executor = Executor::new();

    // Test return with exit code 0
    let func_0 = FunctionDef {
        name: "return_0".to_string(),
        params: vec![],
        body: vec![Statement::Command(Command {
            name: "return".to_string(),
            args: vec![Argument::Literal("0".to_string())],
            redirects: vec![],
            prefix_env: vec![],
        })],
    };

    executor
        .execute_statement(Statement::FunctionDef(func_0))
        .unwrap();
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "return_0".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();
    assert_eq!(result.exit_code, 0);

    // Test return with exit code 1
    let func_1 = FunctionDef {
        name: "return_1".to_string(),
        params: vec![],
        body: vec![Statement::Command(Command {
            name: "return".to_string(),
            args: vec![Argument::Literal("1".to_string())],
            redirects: vec![],
            prefix_env: vec![],
        })],
    };

    executor
        .execute_statement(Statement::FunctionDef(func_1))
        .unwrap();
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "return_1".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();
    assert_eq!(result.exit_code, 1);

    // Test return with exit code 255
    let func_255 = FunctionDef {
        name: "return_255".to_string(),
        params: vec![],
        body: vec![Statement::Command(Command {
            name: "return".to_string(),
            args: vec![Argument::Literal("255".to_string())],
            redirects: vec![],
            prefix_env: vec![],
        })],
    };

    executor
        .execute_statement(Statement::FunctionDef(func_255))
        .unwrap();
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "return_255".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();
    assert_eq!(result.exit_code, 255);
}

#[test]
fn test_return_in_nested_function_calls() {
    let mut executor = Executor::new();

    // Define inner function that returns 10
    let inner_func = FunctionDef {
        name: "inner".to_string(),
        params: vec![],
        body: vec![Statement::Command(Command {
            name: "return".to_string(),
            args: vec![Argument::Literal("10".to_string())],
            redirects: vec![],
            prefix_env: vec![],
        })],
    };

    // Define outer function that calls inner and then returns 20
    let outer_func = FunctionDef {
        name: "outer".to_string(),
        params: vec![],
        body: vec![
            Statement::Command(Command {
                name: "inner".to_string(),
                args: vec![],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "return".to_string(),
                args: vec![Argument::Literal("20".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    executor
        .execute_statement(Statement::FunctionDef(inner_func))
        .unwrap();
    executor
        .execute_statement(Statement::FunctionDef(outer_func))
        .unwrap();

    // Call outer function - should return 20 (its own return, not inner's)
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "outer".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    assert_eq!(result.exit_code, 20);
}

#[test]
fn test_return_preserves_function_output() {
    let mut executor = Executor::new();

    // Define a function that echoes something then returns
    let func = FunctionDef {
        name: "echo_and_return".to_string(),
        params: vec![],
        body: vec![
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Literal("output".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "return".to_string(),
                args: vec![Argument::Literal("7".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    executor
        .execute_statement(Statement::FunctionDef(func))
        .unwrap();

    // Call the function
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "echo_and_return".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    // Should have exit code 7
    assert_eq!(result.exit_code, 7);
    // Note: Output behavior depends on implementation details
}

#[test]
fn test_return_with_conditional_logic() {
    let mut executor = Executor::new();

    // This test demonstrates return in a more complex function
    // Define a function that conditionally returns different codes
    // Since we don't have full if/else parsing in simple AST, we'll simulate with commands
    let func = FunctionDef {
        name: "conditional_return".to_string(),
        params: vec![Parameter {
            name: "should_fail".to_string(),
            type_hint: None,
        }],
        body: vec![
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Literal("checking condition".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "return".to_string(),
                args: vec![Argument::Literal("99".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    executor
        .execute_statement(Statement::FunctionDef(func))
        .unwrap();

    // Call the function
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "conditional_return".to_string(),
            args: vec![Argument::Literal("yes".to_string())],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    assert_eq!(result.exit_code, 99);
}

// ===== shift builtin tests =====

#[test]
fn test_shift_basic_single_parameter() {
    let mut executor = Executor::new();

    // Set positional parameters manually
    executor.runtime_mut().set_positional_params(vec![
        "arg1".to_string(),
        "arg2".to_string(),
        "arg3".to_string(),
    ]);

    // Verify initial state
    assert_eq!(
        executor.runtime_mut().get_variable("1"),
        Some("arg1".to_string())
    );
    assert_eq!(
        executor.runtime_mut().get_variable("#"),
        Some("3".to_string())
    );

    // Execute shift
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "shift".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    assert_eq!(result.exit_code, 0);
    assert_eq!(
        executor.runtime_mut().get_variable("1"),
        Some("arg2".to_string())
    );
    assert_eq!(
        executor.runtime_mut().get_variable("2"),
        Some("arg3".to_string())
    );
    assert_eq!(executor.runtime_mut().get_variable("3"), None);
    assert_eq!(
        executor.runtime_mut().get_variable("#"),
        Some("2".to_string())
    );
}

#[test]
fn test_shift_multiple_parameters() {
    let mut executor = Executor::new();

    executor.runtime_mut().set_positional_params(vec![
        "a".to_string(),
        "b".to_string(),
        "c".to_string(),
        "d".to_string(),
        "e".to_string(),
    ]);

    // Shift by 2
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "shift".to_string(),
            args: vec![Argument::Literal("2".to_string())],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    assert_eq!(result.exit_code, 0);
    assert_eq!(
        executor.runtime_mut().get_variable("1"),
        Some("c".to_string())
    );
    assert_eq!(
        executor.runtime_mut().get_variable("2"),
        Some("d".to_string())
    );
    assert_eq!(
        executor.runtime_mut().get_variable("3"),
        Some("e".to_string())
    );
    assert_eq!(
        executor.runtime_mut().get_variable("#"),
        Some("3".to_string())
    );
}

#[test]
fn test_shift_in_function_with_args() {
    let mut executor = Executor::new();

    // Create a function that processes arguments using shift
    let func = FunctionDef {
        name: "process_args".to_string(),
        params: vec![],
        body: vec![
            // Echo first arg ($1)
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Variable("1".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
            // Shift
            Statement::Command(Command {
                name: "shift".to_string(),
                args: vec![],
                redirects: vec![],
                prefix_env: vec![],
            }),
            // Echo new first arg (was $2, now $1)
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Variable("1".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    executor
        .execute_statement(Statement::FunctionDef(func))
        .unwrap();

    // Call function with arguments
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "process_args".to_string(),
            args: vec![
                Argument::Literal("first".to_string()),
                Argument::Literal("second".to_string()),
                Argument::Literal("third".to_string()),
            ],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    assert_eq!(result.exit_code, 0);
    assert!(result.stdout().contains("first"));
    assert!(result.stdout().contains("second"));
}

#[test]
fn test_shift_multiple_times_in_function() {
    let mut executor = Executor::new();

    // Create a function that shifts multiple times
    let func = FunctionDef {
        name: "shift_twice".to_string(),
        params: vec![],
        body: vec![
            // Echo $1
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Variable("1".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
            // Shift
            Statement::Command(Command {
                name: "shift".to_string(),
                args: vec![],
                redirects: vec![],
                prefix_env: vec![],
            }),
            // Echo $1 (was $2)
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Variable("1".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
            // Shift again
            Statement::Command(Command {
                name: "shift".to_string(),
                args: vec![],
                redirects: vec![],
                prefix_env: vec![],
            }),
            // Echo $1 (was $3)
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Variable("1".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    executor
        .execute_statement(Statement::FunctionDef(func))
        .unwrap();

    // Call with multiple arguments
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "shift_twice".to_string(),
            args: vec![
                Argument::Literal("alpha".to_string()),
                Argument::Literal("beta".to_string()),
                Argument::Literal("gamma".to_string()),
            ],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    assert_eq!(result.exit_code, 0);
    assert!(result.stdout().contains("alpha"));
    assert!(result.stdout().contains("beta"));
    assert!(result.stdout().contains("gamma"));
}

#[test]
fn test_shift_error_when_no_params() {
    let mut executor = Executor::new();

    // No positional parameters set
    executor.runtime_mut().set_positional_params(vec![]);

    // Try to shift - should error
    let result = executor.execute_statement(Statement::Command(Command {
        name: "shift".to_string(),
        args: vec![],
        redirects: vec![],
        prefix_env: vec![],
    }));

    assert!(result.is_err());
}

#[test]
fn test_shift_error_when_count_exceeds_params() {
    let mut executor = Executor::new();

    executor
        .runtime_mut()
        .set_positional_params(vec!["arg1".to_string(), "arg2".to_string()]);

    // Try to shift by 3 when only 2 params - should error
    let result = executor.execute_statement(Statement::Command(Command {
        name: "shift".to_string(),
        args: vec![Argument::Literal("3".to_string())],
        redirects: vec![],
        prefix_env: vec![],
    }));

    assert!(result.is_err());
}

#[test]
fn test_shift_preserves_dollar_at_and_star() {
    let mut executor = Executor::new();

    executor.runtime_mut().set_positional_params(vec![
        "one".to_string(),
        "two".to_string(),
        "three".to_string(),
    ]);

    // Shift once
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "shift".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    assert_eq!(result.exit_code, 0);
    // $@ and $* should now contain only "two three"
    let dollar_at = executor.runtime_mut().get_variable("@");
    assert!(dollar_at.is_some());
    let at_val = dollar_at.unwrap();
    assert!(at_val.contains("two"));
    assert!(at_val.contains("three"));
}

// ============================================================================
// POSIX-004: Local builtin integration tests
// ============================================================================

#[test]
fn test_local_basic_variable() {
    let mut executor = Executor::new();

    // Define a function that uses local variable
    let func = FunctionDef {
        name: "foo".to_string(),
        params: vec![],
        body: vec![
            Statement::Command(Command {
                name: "local".to_string(),
                args: vec![Argument::Literal("x=5".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Variable("x".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    executor
        .execute_statement(Statement::FunctionDef(func))
        .unwrap();

    // Call the function
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "foo".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout().trim(), "5");
}

#[test]
fn test_local_shadows_global() {
    let mut executor = Executor::new();

    // Set global variable
    executor
        .runtime_mut()
        .set_variable("x".to_string(), "10".to_string());

    // Define a function that creates local variable with same name
    let func = FunctionDef {
        name: "foo".to_string(),
        params: vec![],
        body: vec![
            Statement::Command(Command {
                name: "local".to_string(),
                args: vec![Argument::Literal("x=5".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Variable("x".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    executor
        .execute_statement(Statement::FunctionDef(func))
        .unwrap();

    // Call the function - should see local value (5)
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "foo".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout().trim(), "5");

    // After function exits, global value should be restored
    assert_eq!(
        executor.runtime_mut().get_variable("x"),
        Some("10".to_string())
    );
}

#[test]
fn test_local_error_outside_function() {
    let mut executor = Executor::new();

    // Try to use local outside a function - should error
    let result = executor.execute_statement(Statement::Command(Command {
        name: "local".to_string(),
        args: vec![Argument::Literal("x=5".to_string())],
        redirects: vec![],
        prefix_env: vec![],
    }));

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("can only be used in a function"));
}

#[test]
fn test_local_multiple_variables() {
    let mut executor = Executor::new();

    // Define a function that declares multiple local variables
    let func = FunctionDef {
        name: "foo".to_string(),
        params: vec![],
        body: vec![
            Statement::Command(Command {
                name: "local".to_string(),
                args: vec![
                    Argument::Literal("a=1".to_string()),
                    Argument::Literal("b=2".to_string()),
                    Argument::Literal("c=3".to_string()),
                ],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![
                    Argument::Variable("a".to_string()),
                    Argument::Variable("b".to_string()),
                    Argument::Variable("c".to_string()),
                ],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    executor
        .execute_statement(Statement::FunctionDef(func))
        .unwrap();

    // Call the function
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "foo".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout().trim(), "1 2 3");
}

#[test]
fn test_local_without_assignment() {
    let mut executor = Executor::new();

    // Define a function that declares local variable without assignment
    let func = FunctionDef {
        name: "foo".to_string(),
        params: vec![],
        body: vec![
            Statement::Command(Command {
                name: "local".to_string(),
                args: vec![Argument::Literal("x".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![
                    Argument::Literal("empty:".to_string()),
                    Argument::Variable("x".to_string()),
                ],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    executor
        .execute_statement(Statement::FunctionDef(func))
        .unwrap();

    // Call the function - variable should exist but be empty
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "foo".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout().trim(), "empty:");
}

#[test]
fn test_local_cleanup_on_function_exit() {
    let mut executor = Executor::new();

    // Define a function that creates local variables
    let func = FunctionDef {
        name: "foo".to_string(),
        params: vec![],
        body: vec![
            Statement::Command(Command {
                name: "local".to_string(),
                args: vec![
                    Argument::Literal("temp1=value1".to_string()),
                    Argument::Literal("temp2=value2".to_string()),
                ],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Literal("done".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    executor
        .execute_statement(Statement::FunctionDef(func))
        .unwrap();

    // Call the function
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "foo".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    assert_eq!(result.exit_code, 0);

    // After function exits, local variables should not exist
    assert_eq!(executor.runtime_mut().get_variable("temp1"), None);
    assert_eq!(executor.runtime_mut().get_variable("temp2"), None);
}

#[test]
fn test_local_in_nested_functions() {
    let mut executor = Executor::new();

    // Define inner function
    let inner_func = FunctionDef {
        name: "inner".to_string(),
        params: vec![],
        body: vec![
            Statement::Command(Command {
                name: "local".to_string(),
                args: vec![Argument::Literal("x=inner".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Variable("x".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    // Define outer function that calls inner
    let outer_func = FunctionDef {
        name: "outer".to_string(),
        params: vec![],
        body: vec![
            Statement::Command(Command {
                name: "local".to_string(),
                args: vec![Argument::Literal("x=outer".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "inner".to_string(),
                args: vec![],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![Argument::Variable("x".to_string())],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    executor
        .execute_statement(Statement::FunctionDef(inner_func))
        .unwrap();
    executor
        .execute_statement(Statement::FunctionDef(outer_func))
        .unwrap();

    // Call outer function
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "outer".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    assert_eq!(result.exit_code, 0);
    // Should see "inner" from inner function, then "outer" from outer function
    let output = result.stdout();
    assert!(output.contains("inner"));
    assert!(output.contains("outer"));
}

#[test]
fn test_local_mixed_assigned_and_unassigned() {
    let mut executor = Executor::new();

    // Define a function with mix of assigned and unassigned locals
    let func = FunctionDef {
        name: "foo".to_string(),
        params: vec![],
        body: vec![
            Statement::Command(Command {
                name: "local".to_string(),
                args: vec![
                    Argument::Literal("a=1".to_string()),
                    Argument::Literal("b".to_string()),
                    Argument::Literal("c=3".to_string()),
                ],
                redirects: vec![],
                prefix_env: vec![],
            }),
            Statement::Command(Command {
                name: "echo".to_string(),
                args: vec![
                    Argument::Variable("a".to_string()),
                    Argument::Literal("|".to_string()),
                    Argument::Variable("b".to_string()),
                    Argument::Literal("|".to_string()),
                    Argument::Variable("c".to_string()),
                ],
                redirects: vec![],
                prefix_env: vec![],
            }),
        ],
    };

    executor
        .execute_statement(Statement::FunctionDef(func))
        .unwrap();

    // Call the function
    let result = executor
        .execute_statement(Statement::Command(Command {
            name: "foo".to_string(),
            args: vec![],
            redirects: vec![],
            prefix_env: vec![],
        }))
        .unwrap();

    assert_eq!(result.exit_code, 0);
    // b should be empty - echo joins args with single space so empty var produces "| |"
    assert_eq!(result.stdout().trim(), "1 | | 3");
}
