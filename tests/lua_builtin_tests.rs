//! Integration tests for Lua builtin registration and structured data dispatch.
//!
//! These tests verify that:
//! - Lua scripts can register builtins via `rush.register_builtin`
//! - Registered builtins are visible through `Builtins::is_builtin` and `builtin_names`
//! - `Builtins::execute` dispatches to the Lua implementation
//! - Structured outputs (list, record, table) are returned as `Output::Structured`
//! - Plain text outputs are returned as `Output::Text`

use rush::builtins::{Builtins, LuaBuiltin};
use rush::executor::Output;
use rush::lua::LuaRuntime;
use rush::runtime::Runtime;
use std::sync::Arc;

fn make_lua_with_script(script: &str) -> Arc<LuaRuntime> {
    let rt = LuaRuntime::new().expect("LuaRuntime::new failed");
    if !script.is_empty() {
        rt.load_script(script).expect("Lua script failed to load");
    }
    Arc::new(rt)
}

// ---------------------------------------------------------------------------
// Registration and lookup
// ---------------------------------------------------------------------------

#[test]
fn lua_builtin_is_visible_in_is_builtin() {
    let lua = make_lua_with_script(
        r#"
        rush.register_builtin("greet", {
            description = "Say hello",
            run = function(args) return "Hello, " .. (args[1] or "world") end
        })
        "#,
    );

    let builtins = Builtins::with_lua(lua);
    assert!(builtins.is_builtin("greet"), "greet should be a builtin");
    assert!(!builtins.is_builtin("nonexistent_xyz"));
}

#[test]
fn lua_builtin_appears_in_builtin_names() {
    let lua = make_lua_with_script(
        r#"
        rush.register_builtin("my_cmd", {
            description = "test",
            run = function(args) return "ok" end
        })
        "#,
    );

    let builtins = Builtins::with_lua(lua);
    let names = builtins.builtin_names();
    assert!(
        names.contains(&"my_cmd".to_string()),
        "my_cmd should appear in builtin_names"
    );
    // Native builtins should still be present
    assert!(names.contains(&"echo".to_string()));
}

#[test]
fn lua_builtin_registration_exposes_lua_builtin_type() {
    let lua = make_lua_with_script(
        r#"
        rush.register_builtin("typed_cmd", {
            description = "A typed builtin",
            run = function(args) return "hi" end
        })
        "#,
    );

    let builtins = lua.get_registered_builtins();
    let found: Option<&LuaBuiltin> = builtins.iter().find(|b| b.name == "typed_cmd");
    assert!(
        found.is_some(),
        "typed_cmd should be in get_registered_builtins"
    );
    assert_eq!(found.unwrap().description, "A typed builtin");
}

// ---------------------------------------------------------------------------
// Dispatch — text output
// ---------------------------------------------------------------------------

#[test]
fn lua_builtin_returns_text_string() {
    let lua = make_lua_with_script(
        r#"
        rush.register_builtin("say_hi", {
            description = "returns a string",
            run = function(args) return args[1] or "hi" end
        })
        "#,
    );

    let builtins = Builtins::with_lua(lua);
    let mut runtime = Runtime::new();
    let result = builtins
        .execute("say_hi", vec!["world".into()], &mut runtime)
        .expect("execute failed");

    assert_eq!(result.exit_code, 0);
    match result.output {
        Output::Text(s) => assert_eq!(s, "world"),
        other => panic!("Expected Text output, got {:?}", other),
    }
}

#[test]
fn lua_builtin_null_return_is_empty_text() {
    let lua = make_lua_with_script(
        r#"
        rush.register_builtin("noop", {
            description = "returns nothing",
            run = function(args) end
        })
        "#,
    );

    let builtins = Builtins::with_lua(lua);
    let mut runtime = Runtime::new();
    let result = builtins
        .execute("noop", vec![], &mut runtime)
        .expect("execute failed");

    assert_eq!(result.exit_code, 0);
    match result.output {
        Output::Text(s) => assert!(
            s.is_empty(),
            "null return should be empty text, got: {:?}",
            s
        ),
        other => panic!("Expected empty Text output, got {:?}", other),
    }
}

#[test]
fn lua_builtin_int_return_is_text() {
    let lua = make_lua_with_script(
        r#"
        rush.register_builtin("count", {
            description = "returns a number",
            run = function(args) return 42 end
        })
        "#,
    );

    let builtins = Builtins::with_lua(lua);
    let mut runtime = Runtime::new();
    let result = builtins
        .execute("count", vec![], &mut runtime)
        .expect("execute failed");

    assert_eq!(result.exit_code, 0);
    match result.output {
        Output::Text(s) => assert_eq!(s, "42"),
        other => panic!("Expected Text output, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Dispatch — structured output
// ---------------------------------------------------------------------------

#[test]
fn lua_builtin_list_return_is_structured() {
    let lua = make_lua_with_script(
        r#"
        rush.register_builtin("list_things", {
            description = "returns a list",
            run = function(args)
                return {"alpha", "beta", "gamma"}
            end
        })
        "#,
    );

    let builtins = Builtins::with_lua(lua);
    let mut runtime = Runtime::new();
    let result = builtins
        .execute("list_things", vec![], &mut runtime)
        .expect("execute failed");

    assert_eq!(result.exit_code, 0);
    match result.output {
        Output::Structured(json) => {
            let arr = json.as_array().expect("expected JSON array");
            assert_eq!(arr.len(), 3);
        }
        other => panic!("Expected Structured output for list, got {:?}", other),
    }
}

#[test]
fn lua_builtin_record_return_is_structured() {
    let lua = make_lua_with_script(
        r#"
        rush.register_builtin("get_info", {
            description = "returns a record",
            run = function(args)
                return { name = "rush", version = "0.1" }
            end
        })
        "#,
    );

    let builtins = Builtins::with_lua(lua);
    let mut runtime = Runtime::new();
    let result = builtins
        .execute("get_info", vec![], &mut runtime)
        .expect("execute failed");

    assert_eq!(result.exit_code, 0);
    match result.output {
        Output::Structured(json) => {
            let obj = json.as_object().expect("expected JSON object");
            // Value::Record serializes with a "type" tag and inner data.
            // Check that the data is there in some form.
            let json_str = serde_json::to_string(&obj).unwrap();
            assert!(
                json_str.contains("rush"),
                "expected 'rush' in JSON: {}",
                json_str
            );
        }
        other => panic!("Expected Structured output for record, got {:?}", other),
    }
}

#[test]
fn lua_builtin_table_return_is_structured() {
    let lua = make_lua_with_script(
        r#"
        rush.register_builtin("list_files", {
            description = "returns a table",
            run = function(args)
                return {
                    columns = {"name", "size"},
                    rows = {
                        {name = "foo.txt", size = 100},
                        {name = "bar.txt", size = 200},
                    }
                }
            end
        })
        "#,
    );

    let builtins = Builtins::with_lua(lua);
    let mut runtime = Runtime::new();
    let result = builtins
        .execute("list_files", vec![], &mut runtime)
        .expect("execute failed");

    assert_eq!(result.exit_code, 0);
    match result.output {
        Output::Structured(json) => {
            let json_str = serde_json::to_string(&json).unwrap();
            // Value::Table serializes with columns and rows fields
            assert!(
                json_str.contains("foo.txt"),
                "expected 'foo.txt' in JSON: {}",
                json_str
            );
            assert!(
                json_str.contains("bar.txt"),
                "expected 'bar.txt' in JSON: {}",
                json_str
            );
        }
        other => panic!("Expected Structured output for table, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Native builtins are unaffected
// ---------------------------------------------------------------------------

#[test]
fn native_builtins_still_work_with_lua_attached() {
    let lua = make_lua_with_script("");
    let builtins = Builtins::with_lua(lua);
    let mut runtime = Runtime::new();
    let result = builtins
        .execute("echo", vec!["test".into()], &mut runtime)
        .expect("echo failed");

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout(), "test\n");
}

#[test]
fn unknown_builtin_returns_error() {
    let lua = make_lua_with_script("");
    let builtins = Builtins::with_lua(lua);
    let mut runtime = Runtime::new();
    let err = builtins.execute("totally_unknown_cmd", vec![], &mut runtime);
    assert!(err.is_err(), "unknown command should return an error");
}

// ---------------------------------------------------------------------------
// set_lua_runtime
// ---------------------------------------------------------------------------

#[test]
fn set_lua_runtime_attaches_lua_after_construction() {
    let mut builtins = Builtins::new();
    assert!(
        !builtins.is_builtin("late_cmd"),
        "should not exist before attach"
    );

    let lua = make_lua_with_script(
        r#"
        rush.register_builtin("late_cmd", {
            description = "registered after construction",
            run = function(args) return "late" end
        })
        "#,
    );
    builtins.set_lua_runtime(lua);

    assert!(builtins.is_builtin("late_cmd"), "should exist after attach");
}
