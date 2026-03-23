//! Bash compatibility report generator
//!
//! Generates human-readable compatibility reports showing which bash features
//! are supported, have warnings, or are unsupported in Rush.

use super::analyzer::AnalysisResult;
use super::database::CompatDatabase;
use super::features::RushSupportStatus;
use super::migrate::{MigrationEngine, MigrationSuggestion};

/// A single issue found during compatibility analysis
#[derive(Debug, Clone)]
pub struct CompatibilityIssue {
    /// Feature identifier
    pub feature_id: String,
    /// Line number where issue occurs
    pub line_number: usize,
    /// Human-readable description
    pub description: String,
    /// Workaround suggestion if available
    pub workaround: Option<String>,
}

/// Represents supported/warning/unsupported features with line numbers
#[derive(Debug, Clone)]
pub struct FeatureGroup {
    /// Features in this group
    pub issues: Vec<CompatibilityIssue>,
    /// Count of unique features
    pub feature_count: usize,
}

/// Full compatibility report for a script
#[derive(Debug, Clone)]
pub struct CompatibilityReport {
    /// Script filename
    pub filename: String,
    /// Total lines analyzed
    pub lines_analyzed: usize,
    /// Supported features with their occurrences
    pub supported: FeatureGroup,
    /// Features with warnings (planned, etc)
    pub warnings: FeatureGroup,
    /// Unsupported features
    pub unsupported: FeatureGroup,
    /// Overall compatibility percentage
    pub compatibility_percentage: f32,
    /// Migration suggestions for bash-isms
    pub migration_suggestions: Vec<MigrationSuggestion>,
}

impl CompatibilityReport {
    /// Generate a compatibility report from analysis results
    pub fn generate(source_filename: &str, analysis: &AnalysisResult) -> Self {
        let db = CompatDatabase::all_features();

        let mut supported = FeatureGroup {
            issues: Vec::new(),
            feature_count: 0,
        };
        let mut warnings = FeatureGroup {
            issues: Vec::new(),
            feature_count: 0,
        };
        let mut unsupported = FeatureGroup {
            issues: Vec::new(),
            feature_count: 0,
        };

        // Process each feature occurrence from analysis
        for (_category, occurrences) in &analysis.features_by_category {
            for occurrence in occurrences {
                // Find the feature definition from database
                if let Some(feature) = db.iter().find(|f| f.id == occurrence.feature_id) {
                    let issue = CompatibilityIssue {
                        feature_id: occurrence.feature_id.clone(),
                        line_number: occurrence.line_number,
                        description: feature.name.to_string(),
                        workaround: feature.workaround.map(|s| s.to_string()),
                    };

                    match feature.rush_status {
                        RushSupportStatus::Supported => {
                            supported.issues.push(issue);
                            supported.feature_count += 1;
                        }
                        RushSupportStatus::Planned => {
                            warnings.issues.push(issue);
                            warnings.feature_count += 1;
                        }
                        RushSupportStatus::NotSupported => {
                            unsupported.issues.push(issue);
                            unsupported.feature_count += 1;
                        }
                    }
                }
            }
        }

        // Sort issues by line number for consistent output
        supported.issues.sort_by_key(|i| i.line_number);
        warnings.issues.sort_by_key(|i| i.line_number);
        unsupported.issues.sort_by_key(|i| i.line_number);

        // Calculate compatibility percentage
        let total = supported.feature_count + warnings.feature_count + unsupported.feature_count;
        let compatibility_percentage = if total > 0 {
            ((supported.feature_count as f32) / (total as f32)) * 100.0
        } else {
            100.0 // Empty script is "compatible"
        };

        // Generate migration suggestions
        let migration_suggestions = MigrationEngine::suggest_migrations(analysis);

        Self {
            filename: source_filename.to_string(),
            lines_analyzed: analysis.lines_analyzed,
            supported,
            warnings,
            unsupported,
            compatibility_percentage,
            migration_suggestions,
        }
    }

    /// Format the report as a human-readable string with color codes
    pub fn format_report(&self) -> String {
        let mut output = String::new();

        // Header
        output.push_str(&format!("Bash Compatibility Report: {}\n", self.filename));
        output.push_str("═════════════════════════════════════════\n\n");

        // Overall compatibility percentage
        let compatibility_str = if self.compatibility_percentage >= 75.0 {
            format!("\x1b[32m{:.0}%\x1b[0m", self.compatibility_percentage) // Green
        } else if self.compatibility_percentage >= 50.0 {
            format!("\x1b[33m{:.0}%\x1b[0m", self.compatibility_percentage) // Yellow
        } else {
            format!("\x1b[31m{:.0}%\x1b[0m", self.compatibility_percentage) // Red
        };

        output.push_str(&format!("Overall: {} compatible\n", compatibility_str));
        output.push_str(&format!("Lines analyzed: {}\n\n", self.lines_analyzed));

        // Supported features
        if !self.supported.issues.is_empty() {
            output.push_str(&format!(
                "\x1b[32m✓ SUPPORTED\x1b[0m ({} features):\n",
                self.supported.feature_count
            ));
            for issue in &self.supported.issues {
                output.push_str(&format!(
                    "  • Line {}: {}\n",
                    issue.line_number, issue.description
                ));
            }
            output.push('\n');
        }

        // Warnings
        if !self.warnings.issues.is_empty() {
            output.push_str(&format!(
                "\x1b[33m⚠ WARNINGS\x1b[0m ({} issues):\n",
                self.warnings.feature_count
            ));
            for issue in &self.warnings.issues {
                output.push_str(&format!(
                    "  • Line {}: {} (planned, not yet supported)\n",
                    issue.line_number, issue.description
                ));
                if let Some(ref workaround) = issue.workaround {
                    output.push_str(&format!("    Workaround: {}\n", workaround));
                }
            }
            output.push('\n');
        }

        // Unsupported features
        if !self.unsupported.issues.is_empty() {
            output.push_str(&format!(
                "\x1b[31m✗ UNSUPPORTED\x1b[0m ({} features):\n",
                self.unsupported.feature_count
            ));
            for issue in &self.unsupported.issues {
                output.push_str(&format!(
                    "  • Line {}: {}\n",
                    issue.line_number, issue.description
                ));
                if let Some(ref workaround) = issue.workaround {
                    output.push_str(&format!("    Fix: {}\n", workaround));
                }
            }
            output.push('\n');
        }

        // Summary section
        if self.supported.issues.is_empty()
            && self.warnings.issues.is_empty()
            && self.unsupported.issues.is_empty()
        {
            output.push_str("\x1b[32mNo bash-specific features detected.\x1b[0m\n");
        } else {
            output.push_str("Summary:\n");
            output.push_str(&format!(
                "  {} features supported\n",
                self.supported.feature_count
            ));
            output.push_str(&format!(
                "  {} features with warnings\n",
                self.warnings.feature_count
            ));
            output.push_str(&format!(
                "  {} features unsupported\n",
                self.unsupported.feature_count
            ));
        }

        // Migration suggestions section
        if !self.migration_suggestions.is_empty() {
            output.push('\n');
            output.push_str(&MigrationEngine::format_suggestions(
                &self.migration_suggestions,
            ));
        }

        output
    }

    /// Determine exit code based on compatibility
    /// 0: Fully compatible (no issues)
    /// 1: Warnings present (planned features)
    /// 2: Incompatible (unsupported features)
    pub fn exit_code(&self) -> i32 {
        if !self.unsupported.issues.is_empty() {
            2
        } else if !self.warnings.issues.is_empty() {
            1
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_code_compatible() {
        let report = CompatibilityReport {
            filename: "test.sh".to_string(),
            lines_analyzed: 1,
            supported: FeatureGroup {
                issues: vec![],
                feature_count: 1,
            },
            warnings: FeatureGroup {
                issues: vec![],
                feature_count: 0,
            },
            unsupported: FeatureGroup {
                issues: vec![],
                feature_count: 0,
            },
            compatibility_percentage: 100.0,
            migration_suggestions: vec![],
        };

        assert_eq!(report.exit_code(), 0);
    }

    #[test]
    fn test_exit_code_warnings() {
        let report = CompatibilityReport {
            filename: "test.sh".to_string(),
            lines_analyzed: 1,
            supported: FeatureGroup {
                issues: vec![],
                feature_count: 1,
            },
            warnings: FeatureGroup {
                issues: vec![CompatibilityIssue {
                    feature_id: "test".to_string(),
                    line_number: 1,
                    description: "Test warning".to_string(),
                    workaround: None,
                }],
                feature_count: 1,
            },
            unsupported: FeatureGroup {
                issues: vec![],
                feature_count: 0,
            },
            compatibility_percentage: 50.0,
            migration_suggestions: vec![],
        };

        assert_eq!(report.exit_code(), 1);
    }

    #[test]
    fn test_exit_code_unsupported() {
        let report = CompatibilityReport {
            filename: "test.sh".to_string(),
            lines_analyzed: 1,
            supported: FeatureGroup {
                issues: vec![],
                feature_count: 0,
            },
            warnings: FeatureGroup {
                issues: vec![],
                feature_count: 0,
            },
            unsupported: FeatureGroup {
                issues: vec![CompatibilityIssue {
                    feature_id: "test".to_string(),
                    line_number: 1,
                    description: "Test unsupported".to_string(),
                    workaround: Some("Use alternative".to_string()),
                }],
                feature_count: 1,
            },
            compatibility_percentage: 0.0,
            migration_suggestions: vec![],
        };

        assert_eq!(report.exit_code(), 2);
    }

    #[test]
    fn test_format_report() {
        let report = CompatibilityReport {
            filename: "test.sh".to_string(),
            lines_analyzed: 5,
            supported: FeatureGroup {
                issues: vec![CompatibilityIssue {
                    feature_id: "echo".to_string(),
                    line_number: 1,
                    description: "Echo Builtin".to_string(),
                    workaround: None,
                }],
                feature_count: 1,
            },
            warnings: FeatureGroup {
                issues: vec![],
                feature_count: 0,
            },
            unsupported: FeatureGroup {
                issues: vec![],
                feature_count: 0,
            },
            compatibility_percentage: 100.0,
            migration_suggestions: vec![],
        };

        let formatted = report.format_report();
        assert!(formatted.contains("test.sh"));
        assert!(formatted.contains("100%"));
        assert!(formatted.contains("SUPPORTED"));
    }
}
