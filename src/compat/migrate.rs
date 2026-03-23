//! Bash-to-Rush migration suggestion engine
//!
//! Suggests automated rewrites for bash-isms, with optional --fix mode
//! to auto-apply safe transformations.

use super::analyzer::AnalysisResult;
use std::collections::HashMap;

/// Complexity level of a migration suggestion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationComplexity {
    /// Safe, straightforward replacement (safe to auto-apply)
    Simple,
    /// Requires minor review but generally safe
    Moderate,
    /// Requires careful review and testing
    Complex,
}

impl std::fmt::Display for MigrationComplexity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationComplexity::Simple => write!(f, "simple"),
            MigrationComplexity::Moderate => write!(f, "moderate"),
            MigrationComplexity::Complex => write!(f, "complex"),
        }
    }
}

/// A suggested migration for a bash-ism to Rush-native code
#[derive(Debug, Clone)]
pub struct MigrationSuggestion {
    /// Feature identifier
    pub feature_id: String,
    /// Line number where the bash-ism occurs
    pub line_number: usize,
    /// Short bash pattern description
    pub bash_pattern: String,
    /// Suggested Rush replacement
    pub rush_solution: String,
    /// Difficulty level of the migration
    pub complexity: MigrationComplexity,
    /// Detailed explanation
    pub explanation: String,
}

/// A diff preview showing proposed changes
#[derive(Debug, Clone)]
pub struct DiffPreview {
    /// Original line
    pub original: String,
    /// Proposed replacement
    pub replacement: String,
    /// Line number
    pub line_number: usize,
}

/// Migration engine for bash-to-Rush conversion
pub struct MigrationEngine;

impl MigrationEngine {
    /// Get the migration database
    fn migration_database(
    ) -> HashMap<&'static str, (String, String, MigrationComplexity, &'static str)> {
        let mut db = HashMap::new();

        // Process substitution: $(...) or `...`
        db.insert(
            "process_subst_dollar",
            (
                "$(command)".to_string(),
                "$(command)".to_string(),
                MigrationComplexity::Simple,
                "Process substitution is fully supported in Rush",
            ),
        );

        // Array references
        db.insert(
            "array_index",
            (
                "${array[0]}".to_string(),
                "${array[0]}".to_string(),
                MigrationComplexity::Simple,
                "Array indexing is supported in Rush",
            ),
        );

        // String operations
        db.insert(
            "substring_expansion",
            (
                "${var:offset:length}".to_string(),
                "${var:offset:length}".to_string(),
                MigrationComplexity::Simple,
                "Substring expansion is supported in Rush",
            ),
        );

        // Pattern removal
        db.insert(
            "pattern_removal_prefix",
            (
                "${var#pattern}".to_string(),
                "${var#pattern}".to_string(),
                MigrationComplexity::Simple,
                "Prefix pattern removal is supported in Rush",
            ),
        );

        db.insert(
            "pattern_removal_suffix",
            (
                "${var%pattern}".to_string(),
                "${var%pattern}".to_string(),
                MigrationComplexity::Simple,
                "Suffix pattern removal is supported in Rush",
            ),
        );

        // Default values
        db.insert(
            "default_value",
            (
                "${var:-default}".to_string(),
                "${var:-default}".to_string(),
                MigrationComplexity::Simple,
                "Default value expansion is supported in Rush",
            ),
        );

        // String case transformation
        db.insert(
            "string_to_upper",
            (
                "${var^^}".to_string(),
                "${var^^}".to_string(),
                MigrationComplexity::Simple,
                "String case transformation is supported in Rush",
            ),
        );

        db.insert(
            "string_to_lower",
            (
                "${var,,}".to_string(),
                "${var,,}".to_string(),
                MigrationComplexity::Simple,
                "String case transformation is supported in Rush",
            ),
        );

        // Arithmetic expansion
        db.insert(
            "arithmetic",
            (
                "$((expr))".to_string(),
                "$((expr))".to_string(),
                MigrationComplexity::Simple,
                "Arithmetic expansion is supported in Rush",
            ),
        );

        // Command substitution alternatives
        db.insert(
            "command_subst_backtick",
            (
                "`command`".to_string(),
                "$(command)".to_string(),
                MigrationComplexity::Simple,
                "Backtick command substitution should be replaced with $(...)",
            ),
        );

        // Here-doc
        db.insert(
            "heredoc",
            (
                "<<EOF".to_string(),
                "<<EOF".to_string(),
                MigrationComplexity::Moderate,
                "Here-documents are supported in Rush",
            ),
        );

        // Test command
        db.insert(
            "test_bracket",
            (
                "[ condition ]".to_string(),
                "[[ condition ]]".to_string(),
                MigrationComplexity::Moderate,
                "Consider using [[ ]] for more robust condition testing",
            ),
        );

        // Function definitions
        db.insert(
            "function_def",
            (
                "func() { ... }".to_string(),
                "function func() { ... }".to_string(),
                MigrationComplexity::Simple,
                "Function definition syntax is fully supported",
            ),
        );

        // Local variables
        db.insert(
            "local_var",
            (
                "local var=value".to_string(),
                "local var=value".to_string(),
                MigrationComplexity::Simple,
                "Local variable declarations are supported in Rush",
            ),
        );

        // Global exports
        db.insert(
            "export_var",
            (
                "export VAR=value".to_string(),
                "export VAR=value".to_string(),
                MigrationComplexity::Simple,
                "Variable exports are supported in Rush",
            ),
        );

        // Read builtin
        db.insert(
            "read_builtin",
            (
                "read var".to_string(),
                "read var".to_string(),
                MigrationComplexity::Moderate,
                "Read builtin is supported with compatible options",
            ),
        );

        // Source/dot notation
        db.insert(
            "source_dot",
            (
                ". file".to_string(),
                "source file".to_string(),
                MigrationComplexity::Simple,
                "Use 'source' keyword instead of dot notation for clarity",
            ),
        );

        // Null coalescing pattern
        db.insert(
            "null_coalesce",
            (
                "${var:+alternate}".to_string(),
                "${var:+alternate}".to_string(),
                MigrationComplexity::Simple,
                "Null coalescing with + modifier is supported in Rush",
            ),
        );

        db
    }

    /// Suggest migrations for a script analysis
    pub fn suggest_migrations(analysis: &AnalysisResult) -> Vec<MigrationSuggestion> {
        let db = Self::migration_database();
        let mut suggestions = Vec::new();

        // For now, we provide suggestions based on analysis results
        // In practice, we'd analyze the script content directly
        for (_category, occurrences) in &analysis.features_by_category {
            for occurrence in occurrences {
                // Check if this feature has a migration suggestion
                if let Some((pattern, solution, complexity, explanation)) =
                    db.get(occurrence.feature_id.as_str())
                {
                    // Some features are informational (no change needed)
                    if pattern == solution && *complexity == MigrationComplexity::Simple {
                        // Skip simple, already-supported features
                        continue;
                    }

                    suggestions.push(MigrationSuggestion {
                        feature_id: occurrence.feature_id.clone(),
                        line_number: occurrence.line_number,
                        bash_pattern: pattern.clone(),
                        rush_solution: solution.clone(),
                        complexity: *complexity,
                        explanation: explanation.to_string(),
                    });
                }
            }
        }

        // Sort by line number for consistent output
        suggestions.sort_by_key(|s| s.line_number);

        suggestions
    }

    /// Generate a diff preview for suggested migrations
    pub fn preview_diff(
        script_content: &str,
        suggestions: &[MigrationSuggestion],
    ) -> Vec<DiffPreview> {
        let lines: Vec<&str> = script_content.lines().collect();
        let mut diffs = Vec::new();

        for suggestion in suggestions {
            if suggestion.line_number > 0 && suggestion.line_number <= lines.len() {
                let original_line = lines[suggestion.line_number - 1];

                // Perform basic pattern replacement
                let replacement = Self::apply_migration_to_line(original_line, suggestion);

                if replacement != original_line {
                    diffs.push(DiffPreview {
                        original: original_line.to_string(),
                        replacement,
                        line_number: suggestion.line_number,
                    });
                }
            }
        }

        diffs
    }

    /// Apply a migration suggestion to a specific line
    fn apply_migration_to_line(line: &str, suggestion: &MigrationSuggestion) -> String {
        // Handle specific migration patterns
        match suggestion.feature_id.as_str() {
            "command_subst_backtick" => {
                // Replace `cmd` with $(cmd)
                let result = line.to_string();

                // Simple backtick replacement (handles basic cases)
                if result.contains('`') {
                    let mut new_line = String::new();
                    let mut in_backticks = false;
                    let mut backtick_content = String::new();

                    for ch in result.chars() {
                        if ch == '`' {
                            if in_backticks {
                                // End of backtick section
                                new_line.push_str(&format!("$({})", backtick_content));
                                backtick_content.clear();
                                in_backticks = false;
                            } else {
                                // Start of backtick section
                                in_backticks = true;
                            }
                        } else if in_backticks {
                            backtick_content.push(ch);
                        } else {
                            new_line.push(ch);
                        }
                    }

                    // Handle unclosed backtick (malformed, leave as-is)
                    if in_backticks {
                        return line.to_string();
                    }

                    return new_line;
                }
                result
            }
            "source_dot" => {
                // Replace '. file' with 'source file'
                if line.trim().starts_with(". ") {
                    let indent = line.len() - line.trim_start().len();
                    let rest = &line[indent + 2..];
                    format!("{}source {}", " ".repeat(indent), rest)
                } else {
                    line.to_string()
                }
            }
            _ => {
                // For other migrations, return the original line unchanged
                // (would require AST-based replacement for accuracy)
                line.to_string()
            }
        }
    }

    /// Apply safe transformations to script content
    pub fn apply_fixes(script_content: &str, suggestions: &[MigrationSuggestion]) -> String {
        let lines: Vec<&str> = script_content.lines().collect();
        let mut result = Vec::new();
        let mut applied = std::collections::HashSet::new();

        // Build a map of line number to suggestions for efficient lookup
        let mut suggestions_by_line: HashMap<usize, Vec<&MigrationSuggestion>> = HashMap::new();
        for suggestion in suggestions {
            if suggestion.complexity == MigrationComplexity::Simple {
                suggestions_by_line
                    .entry(suggestion.line_number)
                    .or_insert_with(Vec::new)
                    .push(suggestion);
                applied.insert(suggestion.line_number);
            }
        }

        // Apply migrations line by line
        for (idx, line) in lines.iter().enumerate() {
            let line_number = idx + 1;

            if let Some(line_suggestions) = suggestions_by_line.get(&line_number) {
                let mut modified_line = line.to_string();

                for suggestion in line_suggestions {
                    modified_line = Self::apply_migration_to_line(&modified_line, suggestion);
                }

                result.push(modified_line);
            } else {
                result.push(line.to_string());
            }
        }

        result.join("\n")
    }

    /// Format migration suggestions for display
    pub fn format_suggestions(suggestions: &[MigrationSuggestion]) -> String {
        if suggestions.is_empty() {
            return "No migration suggestions.\n".to_string();
        }

        let mut output = String::new();
        output.push_str("\x1b[36mMigration Suggestions\x1b[0m\n");
        output.push_str("═════════════════════════════════════════\n\n");

        let mut simple = Vec::new();
        let mut moderate = Vec::new();
        let mut complex = Vec::new();

        // Categorize by complexity
        for suggestion in suggestions {
            match suggestion.complexity {
                MigrationComplexity::Simple => simple.push(suggestion),
                MigrationComplexity::Moderate => moderate.push(suggestion),
                MigrationComplexity::Complex => complex.push(suggestion),
            }
        }

        // Display simple (safe to auto-apply)
        if !simple.is_empty() {
            output.push_str(&format!(
                "\x1b[32mSAFE TO AUTO-APPLY\x1b[0m ({} suggestions):\n",
                simple.len()
            ));
            for suggestion in simple {
                output.push_str(&format!(
                    "  • Line {}: {} → {}\n",
                    suggestion.line_number, suggestion.bash_pattern, suggestion.rush_solution
                ));
                output.push_str(&format!("    {}\n", suggestion.explanation));
            }
            output.push('\n');
        }

        // Display moderate (review recommended)
        if !moderate.is_empty() {
            output.push_str(&format!(
                "\x1b[33mREVIEW RECOMMENDED\x1b[0m ({} suggestions):\n",
                moderate.len()
            ));
            for suggestion in moderate {
                output.push_str(&format!(
                    "  • Line {}: {} → {}\n",
                    suggestion.line_number, suggestion.bash_pattern, suggestion.rush_solution
                ));
                output.push_str(&format!("    {}\n", suggestion.explanation));
            }
            output.push('\n');
        }

        // Display complex (manual intervention needed)
        if !complex.is_empty() {
            output.push_str(&format!(
                "\x1b[35mMANUAL INTERVENTION\x1b[0m ({} suggestions):\n",
                complex.len()
            ));
            for suggestion in complex {
                output.push_str(&format!(
                    "  • Line {}: {} → {}\n",
                    suggestion.line_number, suggestion.bash_pattern, suggestion.rush_solution
                ));
                output.push_str(&format!("    {}\n", suggestion.explanation));
            }
            output.push('\n');
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backtick_replacement() {
        let line = "result=`echo hello`";
        let suggestion = MigrationSuggestion {
            feature_id: "command_subst_backtick".to_string(),
            line_number: 1,
            bash_pattern: "`cmd`".to_string(),
            rush_solution: "$(cmd)".to_string(),
            complexity: MigrationComplexity::Simple,
            explanation: "Test".to_string(),
        };

        let result = MigrationEngine::apply_migration_to_line(line, &suggestion);
        assert_eq!(result, "result=$(echo hello)");
    }

    #[test]
    fn test_source_dot_replacement() {
        let line = ". ./script.sh";
        let suggestion = MigrationSuggestion {
            feature_id: "source_dot".to_string(),
            line_number: 1,
            bash_pattern: ". file".to_string(),
            rush_solution: "source file".to_string(),
            complexity: MigrationComplexity::Simple,
            explanation: "Test".to_string(),
        };

        let result = MigrationEngine::apply_migration_to_line(line, &suggestion);
        assert_eq!(result, "source ./script.sh");
    }

    #[test]
    fn test_migration_database() {
        let db = MigrationEngine::migration_database();
        assert!(!db.is_empty());
        assert!(db.contains_key("command_subst_backtick"));
        assert!(db.contains_key("source_dot"));
    }

    #[test]
    fn test_format_suggestions_empty() {
        let suggestions = Vec::new();
        let formatted = MigrationEngine::format_suggestions(&suggestions);
        assert!(formatted.contains("No migration suggestions"));
    }

    #[test]
    fn test_format_suggestions_with_data() {
        let suggestions = vec![MigrationSuggestion {
            feature_id: "command_subst_backtick".to_string(),
            line_number: 1,
            bash_pattern: "`cmd`".to_string(),
            rush_solution: "$(cmd)".to_string(),
            complexity: MigrationComplexity::Simple,
            explanation: "Use modern syntax".to_string(),
        }];

        let formatted = MigrationEngine::format_suggestions(&suggestions);
        assert!(formatted.contains("SAFE TO AUTO-APPLY"));
        assert!(formatted.contains("Line 1"));
    }

    #[test]
    fn test_apply_fixes() {
        let script = "result=`echo hello`\necho $result";
        let suggestions = vec![MigrationSuggestion {
            feature_id: "command_subst_backtick".to_string(),
            line_number: 1,
            bash_pattern: "`cmd`".to_string(),
            rush_solution: "$(cmd)".to_string(),
            complexity: MigrationComplexity::Simple,
            explanation: "Test".to_string(),
        }];

        let result = MigrationEngine::apply_fixes(script, &suggestions);
        assert!(result.contains("$(echo hello)"));
    }

    #[test]
    fn test_complexity_display() {
        assert_eq!(MigrationComplexity::Simple.to_string(), "simple");
        assert_eq!(MigrationComplexity::Moderate.to_string(), "moderate");
        assert_eq!(MigrationComplexity::Complex.to_string(), "complex");
    }
}
