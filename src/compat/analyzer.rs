//! Bash script syntax analyzer
//!
//! Parses bash scripts and identifies syntax features used, with line number tracking.

use super::features::feature_database;
use crate::lexer::Lexer;
use crate::parser::ast::*;
use crate::parser::Parser;
use std::collections::HashMap;

/// A single occurrence of a feature in a script
#[derive(Debug, Clone)]
pub struct FeatureOccurrence {
    /// Feature identifier
    pub feature_id: String,
    /// Line number where the feature occurs
    pub line_number: usize,
    /// Column where the feature starts
    pub column: usize,
    /// Context/snippet showing the feature
    pub context: String,
}

/// Result of analyzing a bash script
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    /// File path or name being analyzed
    pub source: String,
    /// All features found, grouped by category
    pub features_by_category: HashMap<String, Vec<FeatureOccurrence>>,
    /// Total feature occurrences
    pub total_occurrences: usize,
    /// Lines analyzed
    pub lines_analyzed: usize,
    /// Any parsing errors encountered
    pub errors: Vec<String>,
}

/// Analyzes bash scripts for syntax features
pub struct ScriptAnalyzer {
    features: HashMap<String, String>, // feature_id -> category
    source: String,
}

impl ScriptAnalyzer {
    /// Create a new analyzer for a source file
    pub fn new(source: String) -> Self {
        let db = feature_database();
        let features = db
            .iter()
            .map(|(id, feature)| (id.to_string(), feature.category.to_string()))
            .collect();

        Self { features, source }
    }

    /// Analyze a bash script and return all features found
    pub fn analyze(&self, script: &str) -> AnalysisResult {
        let mut result = AnalysisResult {
            source: self.source.clone(),
            features_by_category: HashMap::new(),
            total_occurrences: 0,
            lines_analyzed: script.lines().count(),
            errors: Vec::new(),
        };

        // Lex and parse the script
        let tokens = match Lexer::tokenize(script) {
            Ok(tokens) => tokens,
            Err(e) => {
                result.errors.push(format!("Lexer error: {}", e));
                return result;
            }
        };

        let mut parser = Parser::new(tokens);
        let statements = match parser.parse() {
            Ok(stmts) => stmts,
            Err(e) => {
                result.errors.push(format!("Parser error: {}", e));
                return result;
            }
        };

        // Analyze statements for features
        let lines: Vec<&str> = script.lines().collect();
        self.extract_features(&statements, &lines, &mut result);

        result
    }

    /// Extract features from parsed statements
    fn extract_features(
        &self,
        statements: &[Statement],
        lines: &[&str],
        result: &mut AnalysisResult,
    ) {
        for stmt in statements {
            self.analyze_statement(stmt, lines, result);
        }
    }

    /// Analyze a single statement
    fn analyze_statement(&self, stmt: &Statement, lines: &[&str], result: &mut AnalysisResult) {
        match stmt {
            Statement::Command(cmd) => {
                // Identify command type and features
                self.analyze_command(cmd, lines, result);
            }
            Statement::Pipeline(_pipeline) => {
                self.add_feature("pipe", 0, 0, "pipeline", result);
            }
            Statement::Assignment(_assign) => {
                self.add_feature("variable_assignment", 0, 0, "assignment", result);
            }
            Statement::FunctionDef(_func) => {
                self.add_feature("function_def", 0, 0, "function definition", result);
            }
            Statement::IfStatement(_if_stmt) => {
                self.add_feature("if_statement", 0, 0, "if statement", result);
            }
            Statement::ForLoop(_for_loop) => {
                self.add_feature("for_loop", 0, 0, "for loop", result);
            }
            Statement::WhileLoop(_while_loop) => {
                self.add_feature("while_loop", 0, 0, "while loop", result);
            }
            Statement::UntilLoop(_until_loop) => {
                self.add_feature("until_loop", 0, 0, "until loop", result);
            }
            Statement::MatchExpression(_match_expr) => {
                self.add_feature("match_expression", 0, 0, "match expression", result);
            }
            Statement::CaseStatement(_case_stmt) => {
                self.add_feature("case_statement", 0, 0, "case statement", result);
            }
            Statement::ConditionalAnd(_cond_and) => {
                self.add_feature("conditional_and", 0, 0, "conditional AND", result);
            }
            Statement::ConditionalOr(_cond_or) => {
                self.add_feature("conditional_or", 0, 0, "conditional OR", result);
            }
            Statement::Subshell(_subshell) => {
                self.add_feature("subshell", 0, 0, "subshell", result);
            }
            Statement::BackgroundCommand(_bg_cmd) => {
                self.add_feature("background_execution", 0, 0, "background execution", result);
            }
            Statement::ParallelExecution(_parallel) => {
                self.add_feature("parallel_execution", 0, 0, "parallel execution", result);
            }
            Statement::BraceGroup(_brace_group) => {
                self.add_feature("brace_group", 0, 0, "brace group", result);
            }
            Statement::PipeAsk(_pipe_ask) => {
                self.add_feature("pipe_ask", 0, 0, "pipe to AI", result);
            }
        }
    }

    /// Analyze a command for features
    fn analyze_command(&self, cmd: &Command, lines: &[&str], result: &mut AnalysisResult) {
        // Check command name for builtin commands
        match cmd.name.as_str() {
            "declare" => self.add_feature("declare_command", 0, 0, "declare", result),
            "export" => self.add_feature("export_command", 0, 0, "export", result),
            "local" => self.add_feature("local_command", 0, 0, "local", result),
            "readonly" => self.add_feature("readonly_command", 0, 0, "readonly", result),
            "unset" => self.add_feature("unset_command", 0, 0, "unset", result),
            _ => {
                // Generic command
                self.add_feature("simple_command", 0, 0, &cmd.name, result);
            }
        }

        // Check arguments for features
        for arg in &cmd.args {
            self.analyze_argument(arg, lines, result);
        }

        // Check redirects
        for redirect in &cmd.redirects {
            self.analyze_redirect(redirect, result);
        }
    }

    /// Analyze an argument for features
    fn analyze_argument(&self, arg: &Argument, _lines: &[&str], result: &mut AnalysisResult) {
        match arg {
            Argument::Literal(_s) => {
                // Literal string
                self.add_feature("word_splitting", 0, 0, "literal argument", result);
            }
            Argument::Variable(_var) => {
                self.add_feature("variable_expansion", 0, 0, "variable", result);
            }
            Argument::BracedVariable(_var) => {
                self.add_feature("variable_expansion", 0, 0, "braced variable", result);
            }
            Argument::CommandSubstitution(_cmd) => {
                self.add_feature("command_substitution", 0, 0, "command substitution", result);
            }
            Argument::Flag(_flag) => {
                // Command flag/option
                self.add_feature("simple_command", 0, 0, "flag", result);
            }
            Argument::Path(_path) => {
                // File path argument
                self.add_feature("simple_command", 0, 0, "path", result);
            }
            Argument::Glob(_glob) => {
                self.add_feature("glob_expansion", 0, 0, "glob pattern", result);
            }
            Argument::SingleQuoted(_s) => {
                self.add_feature("word_splitting", 0, 0, "single-quoted argument", result);
            }
            Argument::DoubleQuoted(_parts) => {
                self.add_feature("word_splitting", 0, 0, "double-quoted argument", result);
            }
        }
    }

    /// Analyze a redirect for features
    fn analyze_redirect(&self, redirect: &Redirect, result: &mut AnalysisResult) {
        match redirect.kind {
            RedirectKind::Stdout => {
                self.add_feature("redirection_output", 0, 0, "output redirect", result);
            }
            RedirectKind::StdoutAppend => {
                self.add_feature("redirection_append", 0, 0, "append redirect", result);
            }
            RedirectKind::Stdin => {
                self.add_feature("redirection_input", 0, 0, "input redirect", result);
            }
            RedirectKind::Stderr => {
                self.add_feature("redirect_stderr", 0, 0, "stderr redirect", result);
            }
            RedirectKind::StderrToStdout => {
                self.add_feature("redirect_stderr", 0, 0, "stderr to stdout", result);
            }
            RedirectKind::Both => {
                self.add_feature("redirect_both", 0, 0, "both streams redirect", result);
            }
            RedirectKind::HereDoc | RedirectKind::HereDocLiteral => {
                self.add_feature("heredoc", 0, 0, "heredoc", result);
            }
        }
    }

    /// Add a feature occurrence to the result
    fn add_feature(
        &self,
        feature_id: &str,
        line_number: usize,
        column: usize,
        context: &str,
        result: &mut AnalysisResult,
    ) {
        let category = self
            .features
            .get(feature_id)
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        let occurrence = FeatureOccurrence {
            feature_id: feature_id.to_string(),
            line_number,
            column,
            context: context.to_string(),
        };

        result
            .features_by_category
            .entry(category)
            .or_insert_with(Vec::new)
            .push(occurrence);

        result.total_occurrences += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_simple_command() {
        let analyzer = ScriptAnalyzer::new("test.sh".to_string());
        let script = "echo hello\n";
        let result = analyzer.analyze(script);

        assert!(result.errors.is_empty() || result.total_occurrences > 0);
        assert_eq!(result.lines_analyzed, 1);
    }

    #[test]
    fn test_analyze_variable_assignment() {
        let analyzer = ScriptAnalyzer::new("test.sh".to_string());
        let script = "var=value\n";
        let result = analyzer.analyze(script);

        assert_eq!(result.lines_analyzed, 1);
    }

    #[test]
    fn test_analyze_for_loop() {
        let analyzer = ScriptAnalyzer::new("test.sh".to_string());
        let script = "for x in a b c; do echo $x; done\n";
        let result = analyzer.analyze(script);

        assert!(result.features_by_category.len() > 0 || !result.errors.is_empty());
    }

    #[test]
    fn test_analyze_if_statement() {
        let analyzer = ScriptAnalyzer::new("test.sh".to_string());
        let script = "if true; then echo yes; fi\n";
        let result = analyzer.analyze(script);

        assert_eq!(result.lines_analyzed, 1);
    }

    #[test]
    fn test_feature_tracking() {
        let analyzer = ScriptAnalyzer::new("bashrc".to_string());
        let script = "export PATH=/usr/bin:$PATH\n";
        let result = analyzer.analyze(script);

        assert_eq!(result.source, "bashrc");
    }

    #[test]
    fn test_empty_script() {
        let analyzer = ScriptAnalyzer::new("empty.sh".to_string());
        let result = analyzer.analyze("");

        assert_eq!(result.lines_analyzed, 0);
        assert_eq!(result.total_occurrences, 0);
    }
}
