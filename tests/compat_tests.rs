//! Tests for the compatibility analyzer

use rush::compat::{AnalysisResult, ScriptAnalyzer};

#[test]
fn test_analyze_simple_script() {
    let analyzer = ScriptAnalyzer::new("test.sh".to_string());
    let script = "echo 'hello world'";
    let result = analyzer.analyze(script);

    assert_eq!(result.source, "test.sh");
    assert!(result.lines_analyzed > 0);
}

#[test]
fn test_analyze_with_variables() {
    let analyzer = ScriptAnalyzer::new("vars.sh".to_string());
    let script = "name=John\necho $name";
    let result = analyzer.analyze(script);

    assert_eq!(result.lines_analyzed, 2);
}

#[test]
fn test_analyze_for_loop() {
    let analyzer = ScriptAnalyzer::new("loop.sh".to_string());
    let script = "for item in a b c; do echo $item; done";
    let result = analyzer.analyze(script);

    assert_eq!(result.lines_analyzed, 1);
}

#[test]
fn test_analyze_if_statement() {
    let analyzer = ScriptAnalyzer::new("conditional.sh".to_string());
    let script = "if [ $count -gt 0 ]; then echo yes; fi";
    let result = analyzer.analyze(script);

    assert_eq!(result.lines_analyzed, 1);
}

#[test]
fn test_analyze_function_definition() {
    let analyzer = ScriptAnalyzer::new("functions.sh".to_string());
    let script = "greet() {\n  echo Hello $1\n}";
    let result = analyzer.analyze(script);

    assert_eq!(result.lines_analyzed, 3);
}

#[test]
fn test_feature_database_loaded() {
    let analyzer = ScriptAnalyzer::new("test.sh".to_string());
    let script = "echo test";
    let result = analyzer.analyze(script);

    // Should successfully analyze and track features
    assert!(result.errors.is_empty() || result.total_occurrences > 0);
}

#[test]
fn test_handles_empty_script() {
    let analyzer = ScriptAnalyzer::new("empty.sh".to_string());
    let result = analyzer.analyze("");

    assert_eq!(result.lines_analyzed, 0);
    assert_eq!(result.total_occurrences, 0);
}

#[test]
fn test_handles_complex_script() {
    let analyzer = ScriptAnalyzer::new("complex.sh".to_string());
    let script = r#"#!/bin/bash
set -e

export PATH="/usr/local/bin:$PATH"

for file in *.txt; do
    if [ -f "$file" ]; then
        cat "$file" | grep pattern
    fi
done

result=$?
echo "Exit code: $result"
"#;

    let result = analyzer.analyze(script);

    assert!(result.lines_analyzed > 0);
}

#[test]
fn test_handles_pipes_and_redirects() {
    let analyzer = ScriptAnalyzer::new("pipes.sh".to_string());
    let script = "cat input.txt | grep pattern > output.txt 2>&1";
    let result = analyzer.analyze(script);

    // Should parse without errors or track features
    assert!(result.lines_analyzed > 0);
}
