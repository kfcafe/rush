//! Tests for the suggestion engine for command and flag typos
//!
//! This module tests the rush suggestion engine that provides:
//! - Command typo detection (e.g., 'lss' → 'ls')
//! - Flag typo detection (e.g., '--hlep' → '--help')
//! - Context-aware suggestions using history and current directory
//! - Configurable suggestion thresholds

use rush::executor::SuggestionEngine;
use std::path::Path;

#[test]
fn test_suggests_command_typo_lss() {
    let engine = SuggestionEngine::new();
    let builtins = vec!["ls".to_string(), "cat".to_string(), "echo".to_string()];
    let aliases = vec![];
    let history = vec![];
    let cwd = Path::new(".");

    let suggestions = engine.suggest_command("lss", &builtins, &aliases, &history, cwd);

    assert!(
        !suggestions.is_empty(),
        "Should suggest a command for 'lss'"
    );
    assert_eq!(
        suggestions[0].text, "ls",
        "First suggestion should be 'ls' for typo 'lss'"
    );
}

#[test]
fn test_suggests_command_typo_eho() {
    let engine = SuggestionEngine::new();
    let builtins = vec!["echo".to_string(), "cat".to_string(), "grep".to_string()];
    let aliases = vec![];
    let history = vec![];
    let cwd = Path::new(".");

    let suggestions = engine.suggest_command("eho", &builtins, &aliases, &history, cwd);

    assert!(
        !suggestions.is_empty(),
        "Should suggest a command for 'eho'"
    );
    assert_eq!(
        suggestions[0].text, "echo",
        "First suggestion should be 'echo' for typo 'eho'"
    );
}

#[test]
fn test_suggests_flag_typo_hlep() {
    let engine = SuggestionEngine::new();
    let valid_flags = &["--help", "--version", "--verbose"];

    let suggestions = engine.suggest_flag("--hlep", valid_flags);

    assert!(
        !suggestions.is_empty(),
        "Should suggest a flag for '--hlep'"
    );
    assert_eq!(
        suggestions[0].text, "--help",
        "First suggestion should be '--help' for typo '--hlep'"
    );
}

#[test]
fn test_suggests_flag_typo_verbo() {
    let engine = SuggestionEngine::new();
    let valid_flags = &["--help", "--version", "--verbose", "-v"];

    let suggestions = engine.suggest_flag("--verbo", valid_flags);

    assert!(
        !suggestions.is_empty(),
        "Should suggest a flag for '--verbo'"
    );
    assert!(
        suggestions.iter().any(|s| s.text == "--verbose"),
        "Suggestions should include '--verbose' for typo '--verbo'"
    );
}

#[test]
fn test_format_suggestions_displays_all_parts() {
    let engine = SuggestionEngine::new();

    // Import the Suggestion and SuggestionKind types
    use rush::correction::{Suggestion, SuggestionKind};

    let suggestions = vec![
        Suggestion {
            text: "ls".to_string(),
            score: 95,
            kind: SuggestionKind::Builtin,
        },
        Suggestion {
            text: "list".to_string(),
            score: 80,
            kind: SuggestionKind::Executable,
        },
    ];

    let formatted = engine.format_suggestions(&suggestions);

    assert!(
        formatted.contains("Did you mean?"),
        "Should contain 'Did you mean?'"
    );
    assert!(
        formatted.contains("ls"),
        "Should contain the suggestion 'ls'"
    );
    assert!(
        formatted.contains("builtin"),
        "Should show the suggestion kind"
    );
}

#[test]
fn test_is_likely_typo_detects_close_matches() {
    let engine = SuggestionEngine::new();
    let builtins = vec!["echo".to_string(), "cat".to_string()];

    // "ehco" is close to "echo"
    assert!(
        engine.is_likely_typo("ehco", &builtins),
        "Should detect 'ehco' as likely typo of 'echo'"
    );
}

#[test]
fn test_is_likely_typo_rejects_far_matches() {
    let engine = SuggestionEngine::new();
    let builtins = vec!["echo".to_string(), "cat".to_string()];

    // "xyz" is not close to anything
    assert!(
        !engine.is_likely_typo("xyz", &builtins),
        "Should not detect 'xyz' as likely typo"
    );
}

#[test]
fn test_is_likely_flag_typo_detects_close_matches() {
    let engine = SuggestionEngine::new();
    let valid_flags = &["--help", "--version"];

    assert!(
        engine.is_likely_flag_typo("--hlep", valid_flags),
        "Should detect '--hlep' as likely typo of '--help'"
    );
}

#[test]
fn test_is_likely_flag_typo_rejects_far_matches() {
    let engine = SuggestionEngine::new();
    let valid_flags = &["--help", "--version"];

    assert!(
        !engine.is_likely_flag_typo("--xyz", valid_flags),
        "Should not detect '--xyz' as likely typo"
    );
}

#[test]
fn test_disabled_suggestions_returns_empty() {
    let mut engine = SuggestionEngine::new();
    engine.set_enabled(false);

    let builtins = vec!["echo".to_string()];
    let suggestions = engine.suggest_command("ehco", &builtins, &[], &[], Path::new("."));

    assert!(
        suggestions.is_empty(),
        "Disabled suggestions should return empty vector"
    );
}

#[test]
fn test_suggestions_respects_score_threshold() {
    use rush::executor::SuggestionConfig;

    let config = SuggestionConfig {
        enabled: true,
        max_display: 3,
        min_score: 90, // Very high threshold
    };
    let engine = SuggestionEngine::with_config(config);

    let builtins = vec!["ls".to_string()];
    // "xyz" won't meet the high threshold
    let suggestions = engine.suggest_command("xyz", &builtins, &[], &[], Path::new("."));

    assert!(
        suggestions.is_empty(),
        "Suggestions below score threshold should be filtered"
    );
}

#[test]
fn test_suggestions_limited_by_max_display() {
    use rush::executor::SuggestionConfig;

    let config = SuggestionConfig {
        enabled: true,
        max_display: 1, // Only show 1 suggestion
        min_score: 0,   // Allow all
    };
    let engine = SuggestionEngine::with_config(config);

    let builtins = vec!["ls".to_string(), "lsof".to_string(), "lsd".to_string()];
    let suggestions = engine.suggest_command("ls", &builtins, &[], &[], Path::new("."));

    assert!(
        suggestions.len() <= 1,
        "Should be limited to max_display suggestions"
    );
}

#[test]
fn test_suggestion_engine_clone() {
    let engine1 = SuggestionEngine::new();
    let engine2 = engine1.clone();

    assert_eq!(
        engine1.is_enabled(),
        engine2.is_enabled(),
        "Cloned engines should have same enabled state"
    );
}
