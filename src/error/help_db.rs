//! Help text database for common shell errors
//!
//! This module provides actionable help messages for common errors that users
//! encounter when using Rush shell. Each error code can be looked up to get
//! guidance on how to fix the issue.

use std::collections::HashMap;
use std::sync::LazyLock;

/// Help text entry for an error
#[derive(Debug, Clone)]
pub struct HelpEntry {
    /// Short title of the problem
    pub title: &'static str,
    /// Detailed explanation of the error
    pub explanation: &'static str,
    /// How to fix the issue
    pub fix: &'static str,
    /// Example showing the error and solution
    pub example: &'static str,
}

/// Get help text for an error code
pub fn get_help(error_code: &str) -> Option<&'static HelpEntry> {
    HELP_DATABASE.get(error_code).copied()
}

/// Help database mapping error codes to help entries
static HELP_DATABASE: LazyLock<HashMap<&'static str, &'static HelpEntry>> = LazyLock::new(|| {
    let mut map = HashMap::new();

    // File and path errors
    map.insert("FILE_NOT_FOUND", &HELP_FILE_NOT_FOUND);
    map.insert("NO_SUCH_FILE_OR_DIR", &HELP_FILE_NOT_FOUND);
    map.insert("IS_A_DIRECTORY", &HELP_IS_A_DIRECTORY);
    map.insert("PERMISSION_DENIED", &HELP_PERMISSION_DENIED);
    map.insert("FILE_EXISTS", &HELP_FILE_EXISTS);
    map.insert("NOT_A_DIRECTORY", &HELP_NOT_A_DIRECTORY);

    // Syntax and parsing errors
    map.insert("SYNTAX_ERROR", &HELP_SYNTAX_ERROR);
    map.insert("PARSE_ERROR", &HELP_PARSE_ERROR);
    map.insert("UNEXPECTED_TOKEN", &HELP_UNEXPECTED_TOKEN);
    map.insert("UNCLOSED_QUOTE", &HELP_UNCLOSED_QUOTE);
    map.insert("UNCLOSED_BRACE", &HELP_UNCLOSED_BRACE);
    map.insert("UNCLOSED_PAREN", &HELP_UNCLOSED_PAREN);
    map.insert("UNMATCHED_OPERATOR", &HELP_UNMATCHED_OPERATOR);

    // Variable and expansion errors
    map.insert("UNDEFINED_VARIABLE", &HELP_UNDEFINED_VARIABLE);
    map.insert("VARIABLE_NOT_FOUND", &HELP_UNDEFINED_VARIABLE);
    map.insert("INVALID_VARIABLE_NAME", &HELP_INVALID_VARIABLE_NAME);
    map.insert("READONLY_VARIABLE", &HELP_READONLY_VARIABLE);
    map.insert("EXPANSION_ERROR", &HELP_EXPANSION_ERROR);

    // Command errors
    map.insert("COMMAND_NOT_FOUND", &HELP_COMMAND_NOT_FOUND);
    map.insert("NOT_A_BUILTIN", &HELP_NOT_A_BUILTIN);
    map.insert("EXECUTION_ERROR", &HELP_EXECUTION_ERROR);
    map.insert("COMMAND_FAILED", &HELP_COMMAND_FAILED);
    map.insert("AMBIGUOUS_REDIRECT", &HELP_AMBIGUOUS_REDIRECT);

    // Function and control flow errors
    map.insert("INVALID_FUNCTION_NAME", &HELP_INVALID_FUNCTION_NAME);
    map.insert("FUNCTION_NOT_FOUND", &HELP_FUNCTION_NOT_FOUND);
    map.insert("INVALID_RETURN", &HELP_INVALID_RETURN);
    map.insert("INVALID_BREAK", &HELP_INVALID_BREAK);
    map.insert("INVALID_CONTINUE", &HELP_INVALID_CONTINUE);
    map.insert("TOO_MANY_ARGUMENTS", &HELP_TOO_MANY_ARGUMENTS);
    map.insert("TOO_FEW_ARGUMENTS", &HELP_TOO_FEW_ARGUMENTS);

    // Arithmetic and type errors
    map.insert("ARITHMETIC_ERROR", &HELP_ARITHMETIC_ERROR);
    map.insert("DIVISION_BY_ZERO", &HELP_DIVISION_BY_ZERO);
    map.insert("NOT_A_NUMBER", &HELP_NOT_A_NUMBER);
    map.insert("TYPE_ERROR", &HELP_TYPE_ERROR);

    // Option and flag errors
    map.insert("INVALID_OPTION", &HELP_INVALID_OPTION);
    map.insert("OPTION_REQUIRES_ARGUMENT", &HELP_OPTION_REQUIRES_ARGUMENT);
    map.insert("UNRECOGNIZED_OPTION", &HELP_UNRECOGNIZED_OPTION);

    // Job control errors
    map.insert("NO_SUCH_JOB", &HELP_NO_SUCH_JOB);
    map.insert("NO_JOBS", &HELP_NO_JOBS);

    // I/O and redirection errors
    map.insert("INVALID_FD", &HELP_INVALID_FD);
    map.insert("REDIRECT_ERROR", &HELP_REDIRECT_ERROR);

    // Substitution errors
    map.insert(
        "COMMAND_SUBSTITUTION_ERROR",
        &HELP_COMMAND_SUBSTITUTION_ERROR,
    );
    map.insert(
        "PROCESS_SUBSTITUTION_ERROR",
        &HELP_PROCESS_SUBSTITUTION_ERROR,
    );

    map
});

// FILE AND PATH ERRORS

const HELP_FILE_NOT_FOUND: HelpEntry = HelpEntry {
    title: "File or directory not found",
    explanation: "The file or directory you're trying to access doesn't exist at the \
        specified path. This is one of the most common errors in shell scripts.",
    fix: "1. Check the file path for typos\n\
        2. Verify the file actually exists with 'ls'\n\
        3. Use absolute paths instead of relative paths if the file is in a different directory\n\
        4. Check if the file was deleted or moved",
    example: "$ cat /tmp/myfile.txt\n\
        cat: /tmp/myfile.txt: No such file or directory\n\n\
        FIX: Check if the file exists:\n\
        $ ls /tmp/myfile.txt\n\n\
        Or use a different path:\n\
        $ cat ./myfile.txt",
};

const HELP_IS_A_DIRECTORY: HelpEntry = HelpEntry {
    title: "Path is a directory, not a file",
    explanation: "You tried to read from or write to a directory when a file was expected. \
        Commands like 'cat' expect file paths, not directory paths.",
    fix: "1. Check if you meant to specify a file inside the directory\n\
        2. If you need to list directory contents, use 'ls' instead\n\
        3. If you meant to create a file, specify the filename",
    example: "$ cat /tmp\n\
        /tmp: Is a directory\n\n\
        FIX: Specify a file inside the directory:\n\
        $ cat /tmp/myfile.txt\n\n\
        Or list directory contents:\n\
        $ ls /tmp",
};

const HELP_PERMISSION_DENIED: HelpEntry = HelpEntry {
    title: "Permission denied - insufficient access rights",
    explanation: "You don't have the necessary permissions to read, write, or execute \
        this file or directory. This is a security feature to protect files.",
    fix: "1. Check file permissions with 'ls -l'\n\
        2. Add necessary permissions with 'chmod' (e.g., chmod +r file.txt)\n\
        3. Use 'sudo' if you're trying to access system files\n\
        4. Ensure you're the file owner or in the correct group",
    example: "$ cat /root/secret.txt\n\
        Permission denied\n\n\
        FIX: Check permissions:\n\
        $ ls -l /root/secret.txt\n\n\
        Make readable by yourself:\n\
        $ chmod u+r /root/secret.txt",
};

const HELP_FILE_EXISTS: HelpEntry = HelpEntry {
    title: "File already exists",
    explanation: "You tried to create a file that already exists, and the operation \
        doesn't allow overwriting.",
    fix: "1. Use a different filename\n\
        2. Delete the existing file first (rm filename)\n\
        3. Use a command option to allow overwriting (e.g., -f flag)\n\
        4. Append to the file instead of creating new (>> instead of >)",
    example: "$ mkdir /tmp/mydir\n\
        mkdir: cannot create directory '/tmp/mydir': File exists\n\n\
        FIX: Use a different name or remove the existing one:\n\
        $ rm -rf /tmp/mydir\n\
        $ mkdir /tmp/mydir",
};

const HELP_NOT_A_DIRECTORY: HelpEntry = HelpEntry {
    title: "Expected a directory but found a file",
    explanation: "You tried to enter a path as if it were a directory, but it's a \
        regular file. This often happens when part of the path is a file instead of a directory.",
    fix: "1. Check the path components - one of them is a file, not a directory\n\
        2. Use the correct directory path\n\
        3. Ensure you didn't confuse a filename with a directory name",
    example: "$ cd /tmp/myfile.txt\n\
        cd: /tmp/myfile.txt: Not a directory\n\n\
        FIX: Go to the directory containing the file:\n\
        $ cd /tmp",
};

// SYNTAX AND PARSING ERRORS

const HELP_SYNTAX_ERROR: HelpEntry = HelpEntry {
    title: "Syntax error in command",
    explanation: "Your command has invalid syntax that violates shell grammar rules. \
        This could be unmatched quotes, brackets, pipes, or other structural issues.",
    fix: "1. Check for unclosed quotes (single or double)\n\
        2. Verify pipes (|) have commands on both sides\n\
        3. Check for unmatched brackets or parentheses\n\
        4. Ensure operators have proper operands",
    example: "$ echo 'hello world\n\
        > (waiting for closing quote)\n\
        $ echo 'hello world'\n\n\
        $ if [ $x = 5 ]\n\
        Syntax error: missing 'then' or 'fi'\n\n\
        FIX: Add the missing 'then':\n\
        $ if [ $x = 5 ]; then echo yes; fi",
};

const HELP_PARSE_ERROR: HelpEntry = HelpEntry {
    title: "Error parsing command",
    explanation: "The shell couldn't parse your command. This is similar to syntax \
        error but occurs during the parsing phase.",
    fix: "1. Review the entire command for structure\n\
        2. Break complex commands into simpler parts\n\
        3. Check the error message for which part failed\n\
        4. Try quoting arguments that contain special characters",
    example: "$ cmd1 && &&\n\
        Parse error: unexpected operator\n\n\
        FIX: Ensure operators connect valid commands:\n\
        $ cmd1 && cmd2",
};

const HELP_UNEXPECTED_TOKEN: HelpEntry = HelpEntry {
    title: "Unexpected token in command",
    explanation: "The parser encountered a token (word or symbol) that wasn't expected \
        at that position in the command.",
    fix: "1. Check for extra spaces or characters\n\
        2. Verify command structure matches expected format\n\
        3. Look at the token mentioned in the error\n\
        4. Check if you're using correct shell syntax",
    example: "$ [ 5 5 ]\n\
        Unexpected token: '5'\n\n\
        FIX: Add comparison operator:\n\
        $ [ 5 -eq 5 ]",
};

const HELP_UNCLOSED_QUOTE: HelpEntry = HelpEntry {
    title: "Unclosed quote in command",
    explanation: "You opened a quote (single or double) but never closed it. The shell \
        keeps reading trying to find the closing quote.",
    fix: "1. Find the opening quote in your command\n\
        2. Add the matching closing quote at the end\n\
        3. Remember: single quotes preserve everything literally\n\
        4. Double quotes allow variable expansion",
    example: "$ echo \"hello\n\
        > world\n\
        $ echo \"hello\"\n\n\
        $ cat 'file.txt\n\
        Unclosed quote\n\n\
        FIX: Close the quote:\n\
        $ cat 'file.txt'",
};

const HELP_UNCLOSED_BRACE: HelpEntry = HelpEntry {
    title: "Unclosed brace { } in command",
    explanation: "You opened a brace but didn't close it. Braces are used for compound \
        commands and variable expansions.",
    fix: "1. Find the opening brace '{'\n\
        2. Add the matching closing brace '}'\n\
        3. Check that braces are properly nested\n\
        4. In function definitions, ensure the closing brace is on its own line or after semicolon",
    example: "$ { echo a; echo b\n\
        Unclosed brace\n\n\
        FIX: Add the closing brace:\n\
        $ { echo a; echo b; }",
};

const HELP_UNCLOSED_PAREN: HelpEntry = HelpEntry {
    title: "Unclosed parenthesis ( ) in command",
    explanation: "You opened a parenthesis but didn't close it. Parentheses are used \
        for subshells and grouping.",
    fix: "1. Find the opening parenthesis '('\n\
        2. Add the matching closing parenthesis ')'\n\
        3. Ensure parentheses are balanced in nested structures\n\
        4. Don't confuse with arithmetic $((...)) which requires proper closing",
    example: "$ (echo a\n\
        Unclosed parenthesis\n\n\
        FIX: Add the closing parenthesis:\n\
        $ (echo a)",
};

const HELP_UNMATCHED_OPERATOR: HelpEntry = HelpEntry {
    title: "Unmatched operator",
    explanation: "An operator like ||, &&, or | appears without valid commands on \
        both sides, or in an invalid position.",
    fix: "1. Ensure every && and || has a command on both sides\n\
        2. Pipes | must have commands on both sides\n\
        3. Don't put operators at the start or end of a command\n\
        4. Check for accidental double operators (&&& or |||)",
    example: "$ echo hello &&\n\
        Unmatched operator\n\n\
        FIX: Add the second command:\n\
        $ echo hello && echo world",
};

// VARIABLE AND EXPANSION ERRORS

const HELP_UNDEFINED_VARIABLE: HelpEntry = HelpEntry {
    title: "Variable is undefined or not found",
    explanation: "You referenced a variable that doesn't exist or hasn't been set yet. \
        By default, undefined variables expand to empty strings, but some contexts treat them as errors.",
    fix: "1. Set the variable before using it: x=value\n\
        2. Check the variable name for typos\n\
        3. Use set -u to catch undefined variable uses\n\
        4. Use ${var:-default} to provide a default value\n\
        5. Check if the variable was exported from another shell",
    example: "$ echo $undefined_var\n\
        (prints empty line)\n\n\
        FIX: Set the variable:\n\
        $ myvar=hello\n\
        $ echo $myvar\n\
        hello\n\n\
        Or use a default:\n\
        $ echo ${undefined_var:-'not set'}",
};

const HELP_INVALID_VARIABLE_NAME: HelpEntry = HelpEntry {
    title: "Invalid variable name",
    explanation: "Variable names must start with a letter or underscore and contain \
        only letters, numbers, and underscores.",
    fix: "1. Variable names are case-sensitive: MyVar != myvar\n\
        2. Don't use hyphens in variable names (use underscores)\n\
        3. Don't use special characters or spaces\n\
        4. Start with letter or underscore",
    example: "$ my-var=5\n\
        Invalid variable name\n\n\
        FIX: Use underscores instead:\n\
        $ my_var=5\n\n\
        Bad: 2var=x (starts with number)\n\
        Good: var2=x",
};

const HELP_READONLY_VARIABLE: HelpEntry = HelpEntry {
    title: "Variable is read-only, cannot modify",
    explanation: "A variable was marked as read-only using the 'readonly' command, \
        and you're trying to change it.",
    fix: "1. Don't try to modify read-only variables\n\
        2. If you need to change it, you may need to restart the shell\n\
        3. Check if PATH or other system variables are read-only\n\
        4. Use 'readonly' command to see which variables are protected",
    example: "$ readonly MAX=10\n\
        $ MAX=20\n\
        error: MAX: is read-only\n\n\
        FIX: Use a different variable:\n\
        $ max_value=20",
};

const HELP_EXPANSION_ERROR: HelpEntry = HelpEntry {
    title: "Error expanding variable or expression",
    explanation: "An error occurred while trying to expand a variable reference or \
        substitution like ${var}, $(cmd), or arithmetic $((...)).",
    fix: "1. Check variable syntax is correct: ${var} or $var\n\
        2. For arithmetic, use: $((expression))\n\
        3. For command substitution, use: $(command) or `command`\n\
        4. Ensure referenced variables exist",
    example: "$ x=$((y + 5)) where y is undefined\n\
        Expansion error\n\n\
        FIX: Initialize the variable:\n\
        $ y=10\n\
        $ x=$((y + 5))",
};

// COMMAND ERRORS

const HELP_COMMAND_NOT_FOUND: HelpEntry = HelpEntry {
    title: "Command not found",
    explanation: "The command you're trying to run doesn't exist or isn't in your PATH. \
        This is the most common error when a program isn't installed.",
    fix: "1. Check if the command is spelled correctly\n\
        2. Install the missing program if needed\n\
        3. If it's in the current directory, use ./command\n\
        4. Check your PATH: echo $PATH\n\
        5. Use 'type' or 'which' to find where a command is",
    example: "$ greo hello.txt\n\
        greo: command not found\n\n\
        FIX: Check the spelling:\n\
        $ grep hello.txt\n\n\
        Or use the full path:\n\
        $ /usr/bin/grep hello.txt",
};

const HELP_NOT_A_BUILTIN: HelpEntry = HelpEntry {
    title: "Not a shell builtin",
    explanation: "You used the 'builtin' command to call something that isn't a \
        shell builtin, or tried to bypass a function but the actual command is external.",
    fix: "1. Check the spelling of the builtin name\n\
        2. Use 'help' to list available builtins\n\
        3. Don't use 'builtin' for external commands\n\
        4. For external commands, just use the command directly",
    example: "$ builtin grpe\n\
        grpe: not a shell builtin\n\n\
        FIX: Use without 'builtin' for external commands:\n\
        $ grep file.txt\n\n\
        Or check available builtins:\n\
        $ help",
};

const HELP_EXECUTION_ERROR: HelpEntry = HelpEntry {
    title: "Command execution error",
    explanation: "An error occurred while trying to execute a command. This is a \
        general execution failure.",
    fix: "1. Check the detailed error message\n\
        2. Verify command and arguments are correct\n\
        3. Check file permissions\n\
        4. Ensure required files exist\n\
        5. Check system resources (disk space, memory)",
    example: "$ ./script.sh\n\
        Execution error: Permission denied\n\n\
        FIX: Make the script executable:\n\
        $ chmod +x script.sh\n\
        $ ./script.sh",
};

const HELP_COMMAND_FAILED: HelpEntry = HelpEntry {
    title: "Command exited with error status",
    explanation: "The command ran but exited with a non-zero status code, indicating failure. \
        The exact reason depends on the specific command.",
    fix: "1. Check the command's documentation for error codes\n\
        2. Run the command manually to see the error\n\
        3. Add -v or --verbose flag for more details\n\
        4. Check error output (stderr) separately\n\
        5. Check if required input files exist",
    example: "$ grep pattern nonexistent.txt\n\
        grep: nonexistent.txt: No such file or directory\n\
        echo $?\n\
        2\n\n\
        FIX: Verify file exists:\n\
        $ grep pattern existing.txt",
};

const HELP_AMBIGUOUS_REDIRECT: HelpEntry = HelpEntry {
    title: "Ambiguous redirection",
    explanation: "A redirection operator is ambiguous, usually because a variable \
        expansion resulted in multiple words where a single filename was expected.",
    fix: "1. Quote variables in redirections: > \"$file\"\n\
        2. Ensure expansions result in a single filename\n\
        3. Use curly braces for clarity: ${var}\n\
        4. Don't put spaces around the redirection operator",
    example: "$ file='out.txt other.txt'\n\
        $ echo hello > $file\n\
        error: ambiguous redirect\n\n\
        FIX: Quote the variable:\n\
        $ file='out.txt'\n\
        $ echo hello > \"$file\"",
};

// FUNCTION AND CONTROL FLOW ERRORS

const HELP_INVALID_FUNCTION_NAME: HelpEntry = HelpEntry {
    title: "Invalid function name",
    explanation: "Function names must follow the same rules as variable names: start \
        with letter or underscore, contain only alphanumeric characters and underscores.",
    fix: "1. Use only letters, numbers, and underscores\n\
        2. Start with a letter or underscore\n\
        3. Don't use hyphens or special characters\n\
        4. Avoid names that conflict with builtins or commands",
    example: "$ function my-func { echo hello; }\n\
        Invalid function name\n\n\
        FIX: Use underscores:\n\
        $ function my_func { echo hello; }",
};

const HELP_FUNCTION_NOT_FOUND: HelpEntry = HelpEntry {
    title: "Function not found",
    explanation: "You tried to call a function that doesn't exist. Functions must be \
        defined before they're called.",
    fix: "1. Define the function before calling it\n\
        2. Check the function name for typos\n\
        3. Functions may not be exported to subshells\n\
        4. Use 'type' to check if a function exists",
    example: "$ my_func\n\
        my_func: function not found\n\n\
        FIX: Define the function:\n\
        $ function my_func { echo hello; }\n\
        $ my_func",
};

const HELP_INVALID_RETURN: HelpEntry = HelpEntry {
    title: "Return used outside a function",
    explanation: "The 'return' command can only be used inside a function or sourced script. \
        It's not valid at the top level of the shell.",
    fix: "1. Only use 'return' inside functions\n\
        2. Use 'exit' at the top level instead\n\
        3. Use 'exit' to exit sourced scripts if needed\n\
        4. Check that you're actually in a function",
    example: "$ return 1\n\
        error: return: can only 'return' from a function\n\n\
        FIX: Use 'exit' at top level:\n\
        $ exit 1\n\n\
        Or define a function:\n\
        $ function my_func { return 1; }",
};

const HELP_INVALID_BREAK: HelpEntry = HelpEntry {
    title: "Break used outside a loop",
    explanation: "The 'break' command can only be used inside a loop (for, while, or until). \
        It's used to exit the loop early.",
    fix: "1. Only use 'break' inside loops\n\
        2. Make sure you're actually inside a loop\n\
        3. 'break' is used in for, while, and until loops\n\
        4. To exit a function, use 'return' instead",
    example: "$ break\n\
        error: break: only valid in a loop context\n\n\
        FIX: Put it inside a loop:\n\
        $ for i in 1 2 3; do\n\
        >   if [ $i -eq 2 ]; then break; fi\n\
        >   echo $i\n\
        > done",
};

const HELP_INVALID_CONTINUE: HelpEntry = HelpEntry {
    title: "Continue used outside a loop",
    explanation: "The 'continue' command can only be used inside a loop. It skips to \
        the next iteration of the loop.",
    fix: "1. Only use 'continue' inside loops\n\
        2. Ensure you're in a for, while, or until loop\n\
        3. 'continue' skips remaining commands in current iteration\n\
        4. Loop must be active in the current context",
    example: "$ continue\n\
        error: continue: only valid in a loop context\n\n\
        FIX: Use inside a loop:\n\
        $ for i in 1 2 3; do\n\
        >   if [ $i -eq 2 ]; then continue; fi\n\
        >   echo $i\n\
        > done",
};

const HELP_TOO_MANY_ARGUMENTS: HelpEntry = HelpEntry {
    title: "Too many arguments provided",
    explanation: "A command or function received more arguments than it can handle. \
        Most builtins have specific argument requirements.",
    fix: "1. Check the command's documentation or usage help\n\
        2. Reduce the number of arguments\n\
        3. Combine arguments if appropriate\n\
        4. Use --help or -h to see accepted arguments",
    example: "$ echo -n hello world\n\
        error: echo: too many arguments\n\n\
        FIX: Check valid usage:\n\
        $ echo hello world",
};

const HELP_TOO_FEW_ARGUMENTS: HelpEntry = HelpEntry {
    title: "Too few arguments provided",
    explanation: "A command or function requires more arguments than were provided. \
        Many commands expect specific arguments.",
    fix: "1. Provide all required arguments\n\
        2. Check the command's documentation\n\
        3. Use --help to see required arguments\n\
        4. Put arguments in the correct order",
    example: "$ grep\n\
        error: grep: missing arguments\n\n\
        FIX: Provide pattern and file:\n\
        $ grep 'pattern' file.txt",
};

// ARITHMETIC AND TYPE ERRORS

const HELP_ARITHMETIC_ERROR: HelpEntry = HelpEntry {
    title: "Arithmetic operation failed",
    explanation: "An error occurred during arithmetic evaluation, such as invalid syntax \
        or type mismatches.",
    fix: "1. Check arithmetic expression syntax: $((expr))\n\
        2. Use only numbers and valid operators\n\
        3. Check for division by zero\n\
        4. Ensure variables are properly initialized",
    example: "$ x=$((5 / 0))\n\
        error: arithmetic: division by zero\n\n\
        FIX: Check denominator:\n\
        $ if [ $y -ne 0 ]; then x=$((5 / y)); fi",
};

const HELP_DIVISION_BY_ZERO: HelpEntry = HelpEntry {
    title: "Division by zero",
    explanation: "You attempted to divide a number by zero, which is mathematically \
        undefined.",
    fix: "1. Check the divisor before division\n\
        2. Use a guard condition: if [ $x -ne 0 ]\n\
        3. Provide a default value if division fails\n\
        4. Use || to provide an alternative",
    example: "$ x=0; result=$((10 / $x))\n\
        error: division by zero\n\n\
        FIX: Guard the operation:\n\
        $ x=2; result=$((10 / $x))",
};

const HELP_NOT_A_NUMBER: HelpEntry = HelpEntry {
    title: "Value is not a number",
    explanation: "You tried to use a non-numeric value in a context that requires a number, \
        such as arithmetic operations or comparisons.",
    fix: "1. Ensure the value is a valid number\n\
        2. Convert strings to numbers if needed\n\
        3. Check for leading zeros or special characters\n\
        4. Use quotes carefully to avoid word splitting",
    example: "$ x='hello'\n\
        $ y=$((x + 5))\n\
        error: not a number\n\n\
        FIX: Use numeric values:\n\
        $ x=10\n\
        $ y=$((x + 5))",
};

const HELP_TYPE_ERROR: HelpEntry = HelpEntry {
    title: "Type mismatch error",
    explanation: "An operation expected a specific type (number, string, array) but \
        received a different type.",
    fix: "1. Check variable content with echo $var\n\
        2. Use correct comparison operators: -eq for numbers, = for strings\n\
        3. Don't mix types in arithmetic\n\
        4. Quote variables appropriately",
    example: "$ if [ '5' -eq 'five' ]; then echo yes; fi\n\
        error: type error\n\n\
        FIX: Use numeric values:\n\
        $ if [ 5 -eq 5 ]; then echo yes; fi",
};

// OPTION AND FLAG ERRORS

const HELP_INVALID_OPTION: HelpEntry = HelpEntry {
    title: "Invalid option or flag",
    explanation: "You provided an option (flag) to a command that doesn't recognize it. \
        Different commands accept different options.",
    fix: "1. Check the command's help: command --help\n\
        2. Verify the option name and spelling\n\
        3. Use single dash for short options (-v) or double for long (--verbose)\n\
        4. Read the documentation for available options",
    example: "$ ls --invalid\n\
        ls: unrecognized option '--invalid'\n\n\
        FIX: Check valid options:\n\
        $ ls --help\n\
        $ ls -la",
};

const HELP_OPTION_REQUIRES_ARGUMENT: HelpEntry = HelpEntry {
    title: "Option requires an argument",
    explanation: "An option flag expects a value to follow it, but none was provided. \
        Some options need additional parameters.",
    fix: "1. Provide the required argument after the option\n\
        2. Use space or = to separate option and value\n\
        3. Check option documentation for required arguments\n\
        4. Example: -o value or -o=value",
    example: "$ grep -f\n\
        error: -f requires an argument\n\n\
        FIX: Provide a file:\n\
        $ grep -f patterns.txt",
};

const HELP_UNRECOGNIZED_OPTION: HelpEntry = HelpEntry {
    title: "Unrecognized option",
    explanation: "The command doesn't understand the option you provided. This is \
        similar to invalid option but emphasizes the option isn't supported.",
    fix: "1. Use --help to see valid options\n\
        2. Check you're using the right command\n\
        3. Different tools may use different option formats\n\
        4. Some options require specific versions of the tool",
    example: "$ cat -xyz file.txt\n\
        cat: invalid option -- 'z'\n\n\
        FIX: Check valid options:\n\
        $ cat --help",
};

// JOB CONTROL ERRORS

const HELP_NO_SUCH_JOB: HelpEntry = HelpEntry {
    title: "Job not found",
    explanation: "You referenced a job (with %1, %2, etc. or job name) that doesn't exist \
        or has already completed.",
    fix: "1. List current jobs: jobs\n\
        2. Check the job number\n\
        3. Jobs are numbered from 1 onwards\n\
        4. Completed jobs are removed from the list",
    example: "$ fg %5\n\
        fg: no such job: %5\n\n\
        FIX: Check available jobs:\n\
        $ jobs\n\
        $ fg %1",
};

const HELP_NO_JOBS: HelpEntry = HelpEntry {
    title: "No background jobs exist",
    explanation: "You tried to use job control (fg, bg, jobs) but there are no background \
        jobs running.",
    fix: "1. Start a background job first: command &\n\
        2. List jobs to see what's running: jobs\n\
        3. Job control only works with actual jobs\n\
        4. Foreground jobs don't show in job list",
    example: "$ fg\n\
        error: no current job\n\n\
        FIX: Start a background job:\n\
        $ sleep 100 &\n\
        $ fg",
};

// I/O AND REDIRECTION ERRORS

const HELP_INVALID_FD: HelpEntry = HelpEntry {
    title: "Invalid file descriptor",
    explanation: "You tried to use a file descriptor (number) that's not valid. \
        Valid descriptors are 0 (stdin), 1 (stdout), 2 (stderr), and 3+ for custom use.",
    fix: "1. Use valid file descriptor numbers\n\
        2. Don't use negative or very large numbers\n\
        3. Common descriptors: 0=stdin, 1=stdout, 2=stderr\n\
        4. Check you're using the right fd for the operation",
    example: "$ exec 999>&1\n\
        error: invalid file descriptor\n\n\
        FIX: Use lower numbers:\n\
        $ exec 3>&1",
};

const HELP_REDIRECT_ERROR: HelpEntry = HelpEntry {
    title: "Error during redirection",
    explanation: "An error occurred while setting up input/output redirection. \
        This could be file permission issues, missing files, or other problems.",
    fix: "1. Check file permissions\n\
        2. Verify the target directory exists\n\
        3. Ensure you have write permission for output redirection\n\
        4. Check disk space for output files",
    example: "$ echo hello > /root/file.txt\n\
        error: permission denied\n\n\
        FIX: Use a writable location:\n\
        $ echo hello > /tmp/file.txt",
};

// SUBSTITUTION ERRORS

const HELP_COMMAND_SUBSTITUTION_ERROR: HelpEntry = HelpEntry {
    title: "Error in command substitution",
    explanation: "The command inside $(...) or backticks failed or produced invalid output. \
        Command substitution runs a command and inserts its output.",
    fix: "1. Test the command outside substitution first\n\
        2. Use $(command) instead of `command` for better nesting\n\
        3. Handle command failures: $(cmd || echo default)\n\
        4. Check the command's exit status",
    example: "$ x=$(nonexistent_cmd)\n\
        error: command not found\n\n\
        FIX: Use valid command:\n\
        $ x=$(echo hello)\n\n\
        Or handle error:\n\
        $ x=$(echo hello 2>/dev/null || echo 'failed')",
};

const HELP_PROCESS_SUBSTITUTION_ERROR: HelpEntry = HelpEntry {
    title: "Error in process substitution",
    explanation: "Process substitution <(...) or >(...) encountered an error. \
        This advanced feature creates named pipes for concurrent processes.",
    fix: "1. Use <(command) to read from command's output\n\
        2. Use >(command) to write to command's input\n\
        3. Both processes must be valid\n\
        4. Check system's named pipe support",
    example: "$ diff <(sort file1.txt) <(sort file2.txt)",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_help_file_not_found() {
        let help = get_help("FILE_NOT_FOUND");
        assert!(help.is_some());
        let entry = help.unwrap();
        assert_eq!(entry.title, "File or directory not found");
        assert!(entry.explanation.contains("doesn't exist"));
        assert!(entry.fix.contains("typos"));
        assert!(entry.example.contains("No such file"));
    }

    #[test]
    fn test_get_help_nonexistent() {
        let help = get_help("NONEXISTENT_ERROR");
        assert!(help.is_none());
    }

    #[test]
    fn test_get_help_syntax_error() {
        let help = get_help("SYNTAX_ERROR");
        assert!(help.is_some());
        let entry = help.unwrap();
        assert!(entry.explanation.contains("invalid syntax"));
    }

    #[test]
    fn test_get_help_command_not_found() {
        let help = get_help("COMMAND_NOT_FOUND");
        assert!(help.is_some());
        let entry = help.unwrap();
        assert!(entry.title.contains("Command not found"));
        assert!(entry.fix.contains("PATH"));
    }

    #[test]
    fn test_help_entries_have_content() {
        for (code, entry) in HELP_DATABASE.iter() {
            assert!(!entry.title.is_empty(), "Empty title for {}", code);
            assert!(
                !entry.explanation.is_empty(),
                "Empty explanation for {}",
                code
            );
            assert!(!entry.fix.is_empty(), "Empty fix for {}", code);
            assert!(!entry.example.is_empty(), "Empty example for {}", code);
        }
    }

    #[test]
    fn test_minimum_error_count() {
        // Should have at least 30 error codes
        assert!(
            HELP_DATABASE.len() >= 30,
            "Help database has {} entries but expected at least 30",
            HELP_DATABASE.len()
        );
    }
}
