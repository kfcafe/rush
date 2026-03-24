/// Integration tests for structured pipeline operators:
/// where, sort, select, count, first, last, uniq
///
/// These tests run through the full parser + executor stack using
/// the library API directly (no subprocess), so they work without
/// a pre-built binary.
use rush::executor::{Executor, Output};
use rush::lexer::Lexer;
use rush::parser::Parser;

// ── Helper ────────────────────────────────────────────────────────────────────

fn run(cmd: &str) -> rush::executor::ExecutionResult {
    let mut executor = Executor::new();
    let tokens = Lexer::tokenize(cmd).expect("tokenize failed");
    let mut parser = Parser::new(tokens);
    let statements = parser.parse().expect("parse failed");
    executor.execute(statements).expect("execute failed")
}

fn structured_rows(output: &Output) -> &Vec<serde_json::Value> {
    match output {
        Output::Structured(serde_json::Value::Array(arr)) => arr,
        other => panic!("Expected Structured(Array), got {:?}", other),
    }
}

fn structured_scalar(output: &Output) -> &serde_json::Value {
    match output {
        Output::Structured(v) => v,
        other => panic!("Expected Structured, got {:?}", other),
    }
}

// ── count on text input ────────────────────────────────────────────────────────

/// `echo "a\nb\nc" | count` — text fallback: each line becomes a row
#[test]
fn test_text_fallback_count() {
    // printf is a builtin that produces newline-separated lines
    let result = run("printf 'a\\nb\\nc\\n' | count");
    // count emits a scalar number
    let val = structured_scalar(&result.output);
    assert_eq!(val.as_u64(), Some(3), "Expected 3 rows, got {:?}", val);
    assert_eq!(result.exit_code, 0);
}

// ── where ─────────────────────────────────────────────────────────────────────

/// Filter rows of structured data using `where field == value`
#[test]
fn test_where_filters_rows() {
    // Use echo to emit JSON, then filter. We pipe JSON text and rely on
    // the text-to-array coercion (JSON array parses directly).
    let result = run(
        r#"echo '[{"name":"foo","size":10},{"name":"bar","size":20},{"name":"foo","size":5}]' | where name == foo"#,
    );
    let rows = structured_rows(&result.output);
    assert_eq!(rows.len(), 2, "Expected 2 rows matching name==foo");
    for row in rows {
        assert_eq!(row["name"], serde_json::json!("foo"));
    }
    assert_eq!(result.exit_code, 0);
}

/// `where size > 10` with numeric comparison
#[test]
fn test_where_numeric_gt() {
    let result = run(r#"echo '[{"size":5},{"size":15},{"size":25}]' | where size > 10"#);
    let rows = structured_rows(&result.output);
    assert_eq!(rows.len(), 2, "Expected 2 rows with size > 10");
    assert_eq!(result.exit_code, 0);
}

/// `where line =~ error` — glob/substring match on text fallback rows
#[test]
fn test_where_match_text() {
    let result =
        run(r#"printf 'error: something\nwarning: other\nerror: again\n' | where line =~ error"#);
    let rows = structured_rows(&result.output);
    assert_eq!(rows.len(), 2, "Expected 2 lines containing 'error'");
    assert_eq!(result.exit_code, 0);
}

// ── sort ─────────────────────────────────────────────────────────────────────

/// Sort rows ascending by a field
#[test]
fn test_sort_by_field() {
    let result = run(r#"echo '[{"n":3},{"n":1},{"n":2}]' | sort n"#);
    let rows = structured_rows(&result.output);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["n"].as_i64(), Some(1));
    assert_eq!(rows[1]["n"].as_i64(), Some(2));
    assert_eq!(rows[2]["n"].as_i64(), Some(3));
    assert_eq!(result.exit_code, 0);
}

/// Sort rows descending with `-r` flag
#[test]
fn test_sort_reverse() {
    let result = run(r#"echo '[{"n":1},{"n":3},{"n":2}]' | sort -r n"#);
    let rows = structured_rows(&result.output);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["n"].as_i64(), Some(3));
    assert_eq!(rows[2]["n"].as_i64(), Some(1));
    assert_eq!(result.exit_code, 0);
}

// ── select ────────────────────────────────────────────────────────────────────

/// `select name size` — keep only named columns
#[test]
fn test_select_columns() {
    let result = run(r#"echo '[{"name":"foo","size":10,"extra":"drop"}]' | select name size"#);
    let rows = structured_rows(&result.output);
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert!(row.get("name").is_some(), "name field should be present");
    assert!(row.get("size").is_some(), "size field should be present");
    assert!(row.get("extra").is_none(), "extra field should be dropped");
    assert_eq!(result.exit_code, 0);
}

// ── count ─────────────────────────────────────────────────────────────────────

/// Count rows from structured JSON input
#[test]
fn test_count_rows() {
    let result = run(r#"echo '[1,2,3,4,5]' | count"#);
    let val = structured_scalar(&result.output);
    assert_eq!(val.as_u64(), Some(5), "Expected 5 rows");
    assert_eq!(result.exit_code, 0);
}

/// Count zero rows
#[test]
fn test_count_empty() {
    let result = run(r#"echo '[]' | count"#);
    let val = structured_scalar(&result.output);
    assert_eq!(val.as_u64(), Some(0));
    assert_eq!(result.exit_code, 0);
}

// ── first / last ──────────────────────────────────────────────────────────────

/// `first 3` keeps only the first N rows
#[test]
fn test_first_last() {
    let result = run(r#"echo '[1,2,3,4,5]' | first 3"#);
    let rows = structured_rows(&result.output);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].as_i64(), Some(1));
    assert_eq!(rows[2].as_i64(), Some(3));

    let result = run(r#"echo '[1,2,3,4,5]' | last 2"#);
    let rows = structured_rows(&result.output);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].as_i64(), Some(4));
    assert_eq!(rows[1].as_i64(), Some(5));
    assert_eq!(result.exit_code, 0);
}

/// `first` with no argument defaults to 1
#[test]
fn test_first_default_one() {
    let result = run(r#"echo '[10,20,30]' | first"#);
    let rows = structured_rows(&result.output);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].as_i64(), Some(10));
}

// ── uniq ─────────────────────────────────────────────────────────────────────

/// `uniq field` — deduplicate by a named field
#[test]
fn test_uniq_by_field() {
    let result = run(r#"echo '[{"name":"foo"},{"name":"bar"},{"name":"foo"}]' | uniq name"#);
    let rows = structured_rows(&result.output);
    assert_eq!(rows.len(), 2, "Expected 2 unique rows");
    assert_eq!(rows[0]["name"], serde_json::json!("foo"));
    assert_eq!(rows[1]["name"], serde_json::json!("bar"));
    assert_eq!(result.exit_code, 0);
}

// ── chained operators ─────────────────────────────────────────────────────────

/// `echo JSON | where size > 5 | sort name | select name size`
#[test]
fn test_chained_ops() {
    let result = run(
        r#"echo '[{"name":"c","size":10},{"name":"a","size":3},{"name":"b","size":8}]' | where size > 5 | sort name | select name size"#,
    );
    let rows = structured_rows(&result.output);
    assert_eq!(rows.len(), 2, "Expected 2 rows after where filter");
    // Sorted by name ascending: "b" then "c"
    assert_eq!(rows[0]["name"], serde_json::json!("b"));
    assert_eq!(rows[1]["name"], serde_json::json!("c"));
    // Extra fields dropped by select
    assert!(rows[0].get("extra").is_none());
    assert_eq!(result.exit_code, 0);
}

/// `count` at the end of a chain
#[test]
fn test_chained_count() {
    let result = run(r#"echo '[{"size":1},{"size":10},{"size":100}]' | where size > 5 | count"#);
    let val = structured_scalar(&result.output);
    assert_eq!(val.as_u64(), Some(2));
    assert_eq!(result.exit_code, 0);
}
