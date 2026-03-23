//! Feature database for bash syntax compatibility analysis
//!
//! Defines 30+ bash syntax features categorized by their compatibility level.

use std::collections::HashMap;

/// Category of a bash feature
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeatureCategory {
    /// POSIX shell standard features
    Posix,
    /// Bash-specific extensions
    BashSpecific,
    /// Zsh-specific extensions
    ZshSpecific,
}

impl std::fmt::Display for FeatureCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeatureCategory::Posix => write!(f, "POSIX"),
            FeatureCategory::BashSpecific => write!(f, "Bash-specific"),
            FeatureCategory::ZshSpecific => write!(f, "Zsh-specific"),
        }
    }
}

/// Bash syntax feature definition
#[derive(Debug, Clone)]
pub struct BashFeature {
    /// Unique identifier for the feature
    pub id: &'static str,
    /// Human-readable name
    pub name: &'static str,
    /// Description of what the feature does
    pub description: &'static str,
    /// Category (POSIX, Bash-specific, Zsh-specific)
    pub category: FeatureCategory,
    /// Example syntax
    pub example: &'static str,
}

/// Get the feature database
pub fn feature_database() -> HashMap<&'static str, BashFeature> {
    let mut db = HashMap::new();

    // POSIX features
    db.insert(
        "simple_command",
        BashFeature {
            id: "simple_command",
            name: "Simple Command",
            description: "Basic command execution (e.g., 'echo hello')",
            category: FeatureCategory::Posix,
            example: "echo hello",
        },
    );

    db.insert(
        "variable_assignment",
        BashFeature {
            id: "variable_assignment",
            name: "Variable Assignment",
            description: "Assign values to variables",
            category: FeatureCategory::Posix,
            example: "var=value",
        },
    );

    db.insert(
        "variable_expansion",
        BashFeature {
            id: "variable_expansion",
            name: "Variable Expansion",
            description: "Expand variable values with $var or ${var}",
            category: FeatureCategory::Posix,
            example: "$var or ${var}",
        },
    );

    db.insert(
        "double_quoted_string",
        BashFeature {
            id: "double_quoted_string",
            name: "Double Quoted String",
            description: "Strings with variable and command substitution",
            category: FeatureCategory::Posix,
            example: "\"$var and $(cmd)\"",
        },
    );

    db.insert(
        "single_quoted_string",
        BashFeature {
            id: "single_quoted_string",
            name: "Single Quoted String",
            description: "Literal strings, no substitution",
            category: FeatureCategory::Posix,
            example: "'literal string'",
        },
    );

    db.insert(
        "command_substitution",
        BashFeature {
            id: "command_substitution",
            name: "Command Substitution",
            description: "Execute command and use its output",
            category: FeatureCategory::Posix,
            example: "$(cmd) or `cmd`",
        },
    );

    db.insert(
        "pipe",
        BashFeature {
            id: "pipe",
            name: "Pipe",
            description: "Connect stdout of one command to stdin of another",
            category: FeatureCategory::Posix,
            example: "cmd1 | cmd2",
        },
    );

    db.insert(
        "redirection_input",
        BashFeature {
            id: "redirection_input",
            name: "Input Redirection",
            description: "Redirect stdin from file",
            category: FeatureCategory::Posix,
            example: "cmd < file",
        },
    );

    db.insert(
        "redirection_output",
        BashFeature {
            id: "redirection_output",
            name: "Output Redirection",
            description: "Redirect stdout to file",
            category: FeatureCategory::Posix,
            example: "cmd > file",
        },
    );

    db.insert(
        "redirection_append",
        BashFeature {
            id: "redirection_append",
            name: "Append Redirection",
            description: "Append stdout to file",
            category: FeatureCategory::Posix,
            example: "cmd >> file",
        },
    );

    db.insert(
        "for_loop",
        BashFeature {
            id: "for_loop",
            name: "For Loop",
            description: "Iterate over values",
            category: FeatureCategory::Posix,
            example: "for x in a b c; do echo $x; done",
        },
    );

    db.insert(
        "while_loop",
        BashFeature {
            id: "while_loop",
            name: "While Loop",
            description: "Loop while condition is true",
            category: FeatureCategory::Posix,
            example: "while true; do echo hi; done",
        },
    );

    db.insert(
        "if_statement",
        BashFeature {
            id: "if_statement",
            name: "If Statement",
            description: "Conditional execution",
            category: FeatureCategory::Posix,
            example: "if cmd; then echo yes; fi",
        },
    );

    db.insert(
        "function_def",
        BashFeature {
            id: "function_def",
            name: "Function Definition",
            description: "Define reusable functions",
            category: FeatureCategory::Posix,
            example: "func() { echo hello; }",
        },
    );

    db.insert(
        "case_statement",
        BashFeature {
            id: "case_statement",
            name: "Case Statement",
            description: "Multi-way branch on pattern matching",
            category: FeatureCategory::Posix,
            example: "case $x in a) echo a;; b) echo b;; esac",
        },
    );

    db.insert(
        "heredoc",
        BashFeature {
            id: "heredoc",
            name: "Heredoc",
            description: "Multi-line string input",
            category: FeatureCategory::Posix,
            example: "cat << EOF\ntext\nEOF",
        },
    );

    // Bash-specific features
    db.insert(
        "parameter_expansion",
        BashFeature {
            id: "parameter_expansion",
            name: "Parameter Expansion",
            description: "Advanced variable expansion (${var:-default})",
            category: FeatureCategory::BashSpecific,
            example: "${var:-default}",
        },
    );

    db.insert(
        "array_variables",
        BashFeature {
            id: "array_variables",
            name: "Array Variables",
            description: "Indexed arrays arr=(a b c)",
            category: FeatureCategory::BashSpecific,
            example: "arr=(a b c); echo ${arr[0]}",
        },
    );

    db.insert(
        "associative_arrays",
        BashFeature {
            id: "associative_arrays",
            name: "Associative Arrays",
            description: "Hash maps declare -A map; map[key]=value",
            category: FeatureCategory::BashSpecific,
            example: "declare -A map; map[key]=value",
        },
    );

    db.insert(
        "process_substitution",
        BashFeature {
            id: "process_substitution",
            name: "Process Substitution",
            description: "Treat command output as file <(cmd)",
            category: FeatureCategory::BashSpecific,
            example: "diff <(cmd1) <(cmd2)",
        },
    );

    db.insert(
        "extended_globbing",
        BashFeature {
            id: "extended_globbing",
            name: "Extended Globbing",
            description: "Advanced pathname expansion patterns",
            category: FeatureCategory::BashSpecific,
            example: "?(pattern) *(pattern) +(pattern)",
        },
    );

    db.insert(
        "test_operator",
        BashFeature {
            id: "test_operator",
            name: "Test Operator",
            description: "Bash [[ ]] test operator with regex",
            category: FeatureCategory::BashSpecific,
            example: "[[ $var =~ regex ]]",
        },
    );

    db.insert(
        "arithmetic_expansion",
        BashFeature {
            id: "arithmetic_expansion",
            name: "Arithmetic Expansion",
            description: "Evaluate arithmetic expressions $((2+2))",
            category: FeatureCategory::BashSpecific,
            example: "$((2+2))",
        },
    );

    db.insert(
        "arithmetic_condition",
        BashFeature {
            id: "arithmetic_condition",
            name: "Arithmetic Condition",
            description: "Conditional arithmetic (( a > b ))",
            category: FeatureCategory::BashSpecific,
            example: "(( a > b ))",
        },
    );

    db.insert(
        "background_execution",
        BashFeature {
            id: "background_execution",
            name: "Background Execution",
            description: "Run command in background with &",
            category: FeatureCategory::BashSpecific,
            example: "cmd &",
        },
    );

    db.insert(
        "until_loop",
        BashFeature {
            id: "until_loop",
            name: "Until Loop",
            description: "Loop until condition is true",
            category: FeatureCategory::BashSpecific,
            example: "until false; do echo hi; done",
        },
    );

    db.insert(
        "conditional_and",
        BashFeature {
            id: "conditional_and",
            name: "Conditional AND",
            description: "Execute right if left succeeds (&&)",
            category: FeatureCategory::BashSpecific,
            example: "cmd1 && cmd2",
        },
    );

    db.insert(
        "conditional_or",
        BashFeature {
            id: "conditional_or",
            name: "Conditional OR",
            description: "Execute right if left fails (||)",
            category: FeatureCategory::BashSpecific,
            example: "cmd1 || cmd2",
        },
    );

    db.insert(
        "subshell",
        BashFeature {
            id: "subshell",
            name: "Subshell",
            description: "Execute commands in subshell (cmd)",
            category: FeatureCategory::BashSpecific,
            example: "(cmd1; cmd2)",
        },
    );

    db.insert(
        "string_concatenation",
        BashFeature {
            id: "string_concatenation",
            name: "String Concatenation",
            description: "Implicit string concatenation",
            category: FeatureCategory::BashSpecific,
            example: "\"hello\"world or $var$other",
        },
    );

    db.insert(
        "word_splitting",
        BashFeature {
            id: "word_splitting",
            name: "Word Splitting",
            description: "Split variables on whitespace",
            category: FeatureCategory::BashSpecific,
            example: "$var (unquoted)",
        },
    );

    db.insert(
        "glob_expansion",
        BashFeature {
            id: "glob_expansion",
            name: "Glob Expansion",
            description: "Pathname expansion with * ? [ ]",
            category: FeatureCategory::BashSpecific,
            example: "*.txt or dir/**/*",
        },
    );

    db.insert(
        "declare_command",
        BashFeature {
            id: "declare_command",
            name: "Declare Command",
            description: "Declare variables with attributes",
            category: FeatureCategory::BashSpecific,
            example: "declare -r VAR=value",
        },
    );

    db.insert(
        "export_command",
        BashFeature {
            id: "export_command",
            name: "Export Command",
            description: "Export variables to environment",
            category: FeatureCategory::BashSpecific,
            example: "export VAR=value",
        },
    );

    db.insert(
        "local_command",
        BashFeature {
            id: "local_command",
            name: "Local Command",
            description: "Create local variables in functions",
            category: FeatureCategory::BashSpecific,
            example: "local var=value",
        },
    );

    db.insert(
        "readonly_command",
        BashFeature {
            id: "readonly_command",
            name: "Readonly Command",
            description: "Make variables read-only",
            category: FeatureCategory::BashSpecific,
            example: "readonly VAR=value",
        },
    );

    db.insert(
        "unset_command",
        BashFeature {
            id: "unset_command",
            name: "Unset Command",
            description: "Unset variables or functions",
            category: FeatureCategory::BashSpecific,
            example: "unset VAR",
        },
    );

    db.insert(
        "error_handling",
        BashFeature {
            id: "error_handling",
            name: "Error Handling",
            description: "Check exit status with $?",
            category: FeatureCategory::BashSpecific,
            example: "echo $?",
        },
    );

    db.insert(
        "positional_parameters",
        BashFeature {
            id: "positional_parameters",
            name: "Positional Parameters",
            description: "Access command arguments $1, $2, $@",
            category: FeatureCategory::BashSpecific,
            example: "$1, $@, $#",
        },
    );

    db.insert(
        "special_parameters",
        BashFeature {
            id: "special_parameters",
            name: "Special Parameters",
            description: "Special variables like $?, $$, $!",
            category: FeatureCategory::BashSpecific,
            example: "$?, $$, $!, $0",
        },
    );

    db.insert(
        "redirect_stderr",
        BashFeature {
            id: "redirect_stderr",
            name: "Redirect Stderr",
            description: "Redirect stderr to file or stdout",
            category: FeatureCategory::BashSpecific,
            example: "cmd 2> file or cmd 2>&1",
        },
    );

    db.insert(
        "redirect_both",
        BashFeature {
            id: "redirect_both",
            name: "Redirect Both Streams",
            description: "Redirect both stdout and stderr",
            category: FeatureCategory::BashSpecific,
            example: "cmd &> file or cmd > file 2>&1",
        },
    );

    // Zsh-specific features
    db.insert(
        "floating_point_math",
        BashFeature {
            id: "floating_point_math",
            name: "Floating Point Math",
            description: "Native floating point arithmetic",
            category: FeatureCategory::ZshSpecific,
            example: "echo $((1.5 + 2.3))",
        },
    );

    db.insert(
        "builtin_functions",
        BashFeature {
            id: "builtin_functions",
            name: "Builtin Functions",
            description: "Zsh-specific builtin functions",
            category: FeatureCategory::ZshSpecific,
            example: "emulate, disable, enable",
        },
    );

    db
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_database() {
        let db = feature_database();
        assert!(db.len() >= 30, "Database should have at least 30 features");
    }

    #[test]
    fn test_posix_features_exist() {
        let db = feature_database();
        assert!(db.contains_key("simple_command"));
        assert!(db.contains_key("for_loop"));
        assert!(db.contains_key("if_statement"));
        assert!(db.contains_key("heredoc"));
    }

    #[test]
    fn test_bash_specific_features_exist() {
        let db = feature_database();
        assert!(db.contains_key("array_variables"));
        assert!(db.contains_key("process_substitution"));
        assert!(db.contains_key("arithmetic_expansion"));
    }

    #[test]
    fn test_feature_categories() {
        let db = feature_database();
        let mut posix_count = 0;
        let mut bash_count = 0;
        let mut zsh_count = 0;

        for feature in db.values() {
            match feature.category {
                FeatureCategory::Posix => posix_count += 1,
                FeatureCategory::BashSpecific => bash_count += 1,
                FeatureCategory::ZshSpecific => zsh_count += 1,
            }
        }

        assert!(posix_count > 0, "Should have POSIX features");
        assert!(bash_count > 0, "Should have Bash-specific features");
        assert!(zsh_count > 0, "Should have Zsh-specific features");
    }
}

// ============================================================================
// Rush Compatibility Database - 57 Bash Features Catalogued
// ============================================================================

/// Support status for a feature in Rush
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RushSupportStatus {
    /// Feature is fully supported
    Supported,
    /// Feature is planned but not yet implemented
    Planned,
    /// Feature will not be supported (by design or low priority)
    NotSupported,
}

impl RushSupportStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RushSupportStatus::Supported => "supported",
            RushSupportStatus::Planned => "planned",
            RushSupportStatus::NotSupported => "not-supported",
        }
    }
}

/// Extended feature metadata mapping bash features to Rush support
#[derive(Debug, Clone)]
pub struct RushCompatFeature {
    /// Unique identifier
    pub id: &'static str,
    /// Human-readable name
    pub name: &'static str,
    /// Description
    pub description: &'static str,
    /// Support status in Rush
    pub rush_status: RushSupportStatus,
    /// Bash version that introduced this
    pub bash_version: &'static str,
    /// Example code
    pub bash_example: &'static str,
    /// Workaround if unsupported
    pub workaround: Option<&'static str>,
    /// When Rush added support
    pub rush_version: Option<&'static str>,
    /// Additional notes
    pub notes: &'static str,
}

/// Get Rush compatibility features database
pub fn rush_compat_features() -> Vec<RushCompatFeature> {
    vec![
        // VARIABLES - 11 features
        RushCompatFeature {
            id: "env-vars",
            name: "Environment Variables",
            description: "Access and modify environment variables",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "export PATH=/usr/bin:$PATH",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support via export builtin",
        },
        RushCompatFeature {
            id: "positional-params",
            name: "Positional Parameters",
            description: "$0, $1, $2... for function/script arguments",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "echo $1 $2 $3",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support in function calls",
        },
        RushCompatFeature {
            id: "special-params",
            name: "Special Parameters",
            description: "$#, $*, $@, $?, $-, $$, $!, etc.",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "echo $# $? $$",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Core special params supported",
        },
        RushCompatFeature {
            id: "array-vars",
            name: "Array Variables",
            description: "Indexed and associative arrays",
            rush_status: RushSupportStatus::Planned,
            bash_version: "2.0",
            bash_example: "arr=(a b c); echo ${arr[0]} ${arr[@]}",
            workaround: Some("Use multiple variables or shift parameters"),
            rush_version: None,
            notes: "In planning phase",
        },
        RushCompatFeature {
            id: "readonly-vars",
            name: "Readonly Variables",
            description: "Make variables immutable with readonly",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "readonly VAR=value",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support via readonly builtin",
        },
        RushCompatFeature {
            id: "local-vars",
            name: "Local Variables",
            description: "Function-scoped variables",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "func() { local x=5; }",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support via local builtin",
        },
        RushCompatFeature {
            id: "var-expansion",
            name: "Variable Expansion",
            description: "$VAR, ${VAR}, with defaults and subscripts",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "echo ${VAR:-default} ${VAR:?error}",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Basic expansion supported",
        },
        RushCompatFeature {
            id: "indirect-expansion",
            name: "Indirect Variable Expansion",
            description: "${!VAR} to access variable whose name is in VAR",
            rush_status: RushSupportStatus::NotSupported,
            bash_version: "2.0",
            bash_example: "ref=PATH; echo ${!ref}",
            workaround: Some("Store values directly or use associative arrays"),
            rush_version: None,
            notes: "Complex feature",
        },
        RushCompatFeature {
            id: "name-refs",
            name: "Name References (nameref)",
            description: "declare -n to create variable references",
            rush_status: RushSupportStatus::NotSupported,
            bash_version: "4.3",
            bash_example: "declare -n ref=VAR; ref=value",
            workaround: Some("Use local and pass by name"),
            rush_version: None,
            notes: "Advanced bash feature",
        },
        RushCompatFeature {
            id: "var-typing",
            name: "Variable Typing (declare flags)",
            description: "declare -i (integer), -a (array), -A (assoc), etc.",
            rush_status: RushSupportStatus::Planned,
            bash_version: "2.0",
            bash_example: "declare -i num=5; declare -a arr",
            workaround: Some("Manually manage types"),
            rush_version: None,
            notes: "Partially supported",
        },
        // CONTROL FLOW - 12 features
        RushCompatFeature {
            id: "if-else",
            name: "If-Else Statements",
            description: "Conditional branching with if/elif/else/fi",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "if [ $x -eq 1 ]; then echo yes; fi",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "case-statement",
            name: "Case Statements",
            description: "Pattern matching with case/esac",
            rush_status: RushSupportStatus::Planned,
            bash_version: "2.0",
            bash_example: "case $x in 1) echo one;; esac",
            workaround: Some("Use nested if-elif-else"),
            rush_version: None,
            notes: "In planning",
        },
        RushCompatFeature {
            id: "for-loop",
            name: "For Loops",
            description: "Iterate over values with for/do/done",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "for i in 1 2 3; do echo $i; done",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "for-c-style",
            name: "C-Style For Loops",
            description: "for ((i=0; i<10; i++)) syntax",
            rush_status: RushSupportStatus::Planned,
            bash_version: "2.0",
            bash_example: "for ((i=0; i<10; i++)); do echo $i; done",
            workaround: Some("Use while loop"),
            rush_version: None,
            notes: "Planned",
        },
        RushCompatFeature {
            id: "while-loop",
            name: "While Loops",
            description: "Loop while condition is true",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "while [ $x -lt 10 ]; do ((x++)); done",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "until-loop",
            name: "Until Loops",
            description: "Loop until condition becomes true",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "until [ $x -ge 10 ]; do ((x++)); done",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "break-continue",
            name: "Break and Continue",
            description: "break and continue statements for loop control",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "while true; do break; done",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "select-loop",
            name: "Select Loops",
            description: "select loop for interactive menu creation",
            rush_status: RushSupportStatus::NotSupported,
            bash_version: "2.0",
            bash_example: "select opt in opt1 opt2; do echo $opt; done",
            workaround: Some("Implement menu manually"),
            rush_version: None,
            notes: "Interactive feature",
        },
        RushCompatFeature {
            id: "return-stmt",
            name: "Return Statements",
            description: "Return from function with exit code",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "func() { return 42; }",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "exit-stmt",
            name: "Exit Statements",
            description: "Exit shell with exit code",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "exit 1",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "function-defs",
            name: "Function Definitions",
            description: "Define functions with function keyword or ()",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "func() { echo hello; }",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Both syntaxes",
        },
        // BUILTINS - 15 features
        RushCompatFeature {
            id: "echo",
            name: "Echo Builtin",
            description: "Output text to stdout",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "echo hello world",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "printf",
            name: "Printf Builtin",
            description: "Formatted output like C printf",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "printf '%s\\n' hello",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Core specifiers",
        },
        RushCompatFeature {
            id: "read",
            name: "Read Builtin",
            description: "Read input from stdin into variables",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "read -p 'Enter: ' var",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "test-builtin",
            name: "Test Builtin ([ ])",
            description: "File and string conditionals",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "[ -f file.txt ] && echo exists",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "cd-builtin",
            name: "Cd Builtin",
            description: "Change directory",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "cd /path/to/dir",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "pwd-builtin",
            name: "Pwd Builtin",
            description: "Print working directory",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "pwd",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "export-builtin",
            name: "Export Builtin",
            description: "Export variables to child processes",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "export VAR=value",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "source-builtin",
            name: "Source Builtin",
            description: "Execute script in current shell context",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "source ./script.sh",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "alias-builtin",
            name: "Alias Builtin",
            description: "Create command aliases",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "alias ll='ls -la'",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "unset-builtin",
            name: "Unset Builtin",
            description: "Remove variables or functions",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "unset VAR",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "type-builtin",
            name: "Type Builtin",
            description: "Show how a command would be executed",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "type ls",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "jobs-builtin",
            name: "Jobs Builtin",
            description: "List background jobs",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "jobs",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "trap-builtin",
            name: "Trap Builtin",
            description: "Trap signals and run commands",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "trap 'echo cleaned' EXIT",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "kill-builtin",
            name: "Kill Builtin",
            description: "Terminate processes by PID or job",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "kill %1",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "shift-builtin",
            name: "Shift Builtin",
            description: "Remove positional parameters",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "shift 2",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        // SYNTAX - 10 features
        RushCompatFeature {
            id: "command-subst",
            name: "Command Substitution",
            description: "$(cmd) and `cmd` to substitute output",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "result=$(ls) or result=`ls`",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "process-subst",
            name: "Process Substitution",
            description: "<(cmd) and >(cmd) for stdin/stdout",
            rush_status: RushSupportStatus::NotSupported,
            bash_version: "3.0",
            bash_example: "diff <(sort a) <(sort b)",
            workaround: Some("Use temp files or pipes"),
            rush_version: None,
            notes: "Advanced feature",
        },
        RushCompatFeature {
            id: "pipe-operator",
            name: "Pipe Operator",
            description: "| to connect stdout to stdin",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "ls | grep txt",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "redirect-append",
            name: "Append Redirection",
            description: ">> to append to files",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "echo hello >> file.txt",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "redirect-stderr",
            name: "Stderr Redirection",
            description: "2>, 2>>, 2>&1 for error redirection",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "cmd 2> error.log",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "heredoc",
            name: "Heredoc Syntax",
            description: "<<EOF multi-line string input",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "cat <<EOF\nMulti\nLine\nEOF",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "logical-and-or",
            name: "Logical AND/OR",
            description: "&& and || operators for conditional execution",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "cmd1 && cmd2 || cmd3",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "background-job",
            name: "Background Jobs",
            description: "& to run command in background",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "long_cmd &",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "subshell",
            name: "Subshells",
            description: "(cmd) to run in subshell",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "(cd /tmp && pwd)",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "comment-syntax",
            name: "Comments",
            description: "# for single-line comments",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "# This is a comment",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        // EXPANSIONS - 9 features
        RushCompatFeature {
            id: "tilde-expansion",
            name: "Tilde Expansion",
            description: "~ expands to home directory",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "cd ~",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "glob-expansion",
            name: "Glob Expansion",
            description: "*, ?, [abc], {a,b,c} for pattern matching",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "ls *.txt",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "arithmetic-expand",
            name: "Arithmetic Expansion",
            description: "$((expr)) for arithmetic evaluation",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "echo $((5 + 3))",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "brace-expansion",
            name: "Brace Expansion",
            description: "{a,b,c} and {1..5} for list generation",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "echo {a,b,c} or echo {1..10}",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "string-slicing",
            name: "String Slicing",
            description: "${VAR:offset:length} for substring extraction",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "echo ${VAR:0:5}",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "pattern-removal",
            name: "Pattern Removal",
            description: "${VAR#pattern}, ${VAR##pattern}, etc.",
            rush_status: RushSupportStatus::Planned,
            bash_version: "2.0",
            bash_example: "echo ${VAR#prefix}",
            workaround: Some("Use external tools"),
            rush_version: None,
            notes: "Planned",
        },
        RushCompatFeature {
            id: "pattern-substitution",
            name: "Pattern Substitution",
            description: "${VAR/pattern/replacement}",
            rush_status: RushSupportStatus::Planned,
            bash_version: "2.0",
            bash_example: "echo ${VAR/old/new}",
            workaround: Some("Use sed"),
            rush_version: None,
            notes: "Planned",
        },
        RushCompatFeature {
            id: "default-expansion",
            name: "Default Value Expansion",
            description: "${VAR:-default}, ${VAR:=default}, ${VAR:?error}",
            rush_status: RushSupportStatus::Supported,
            bash_version: "2.0",
            bash_example: "echo ${VAR:-default_value}",
            workaround: None,
            rush_version: Some("0.1"),
            notes: "Full support",
        },
        RushCompatFeature {
            id: "case-conversion",
            name: "Case Conversion Expansion",
            description: "${VAR^}, ${VAR^^}, ${VAR,}, ${VAR,,}",
            rush_status: RushSupportStatus::NotSupported,
            bash_version: "4.0",
            bash_example: "echo ${VAR^^}",
            workaround: Some("Use tr or other tools"),
            rush_version: None,
            notes: "Bash 4.0+",
        },
    ]
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;

    #[test]
    fn test_compat_features_count() {
        let features = rush_compat_features();
        assert!(
            features.len() >= 50,
            "Must have at least 50 features, got {}",
            features.len()
        );
    }

    #[test]
    fn test_compat_feature_stats() {
        let features = rush_compat_features();
        let supported = features
            .iter()
            .filter(|f| f.rush_status == RushSupportStatus::Supported)
            .count();
        let planned = features
            .iter()
            .filter(|f| f.rush_status == RushSupportStatus::Planned)
            .count();
        let not_supported = features
            .iter()
            .filter(|f| f.rush_status == RushSupportStatus::NotSupported)
            .count();

        assert_eq!(features.len(), supported + planned + not_supported);
        assert!(supported > 0, "Should have supported features");
        assert!(planned > 0, "Should have planned features");
        assert!(
            not_supported > 0,
            "Should have unsupported features with workarounds"
        );
    }

    #[test]
    fn test_unsupported_have_workarounds() {
        let features = rush_compat_features();
        for feature in features.iter() {
            if feature.rush_status == RushSupportStatus::NotSupported {
                assert!(
                    feature.workaround.is_some(),
                    "Feature '{}' is not supported but has no workaround",
                    feature.id
                );
            }
        }
    }
}
