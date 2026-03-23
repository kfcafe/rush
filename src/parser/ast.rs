use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Statement {
    Command(Command),
    Pipeline(Pipeline),
    ParallelExecution(ParallelExecution),
    Assignment(Assignment),
    FunctionDef(FunctionDef),
    IfStatement(IfStatement),
    ForLoop(ForLoop),
    WhileLoop(WhileLoop),
    UntilLoop(UntilLoop),
    MatchExpression(MatchExpression),
    CaseStatement(CaseStatement),
    ConditionalAnd(ConditionalAnd),
    ConditionalOr(ConditionalOr),
    Subshell(Vec<Statement>),
    BackgroundCommand(Box<Statement>),
    /// Brace group: { commands; } - executes in current shell context
    BraceGroup(Vec<Statement>),
    /// Pipe to AI: cmd |? "prompt"
    PipeAsk(PipeAsk),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Command {
    pub name: String,
    pub args: Vec<Argument>,
    pub redirects: Vec<Redirect>,
    /// Prefix environment assignments (e.g., `FOO=bar cmd` sets FOO only for cmd)
    #[serde(default)]
    pub prefix_env: Vec<(String, String)>,
}

/// An element in a pipeline - either a regular command, subshell, or compound command
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PipelineElement {
    Command(Command),
    Subshell(Vec<Statement>),
    /// Compound commands (while, until, for, if, case, brace groups) as pipeline elements
    CompoundCommand(Box<Statement>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pipeline {
    pub commands: Vec<Command>,
    /// Extended pipeline elements supporting subshells alongside commands.
    /// When non-empty, this is the authoritative pipeline representation.
    /// `commands` is kept for backward compatibility.
    #[serde(default)]
    pub elements: Vec<PipelineElement>,
    /// Whether this pipeline is negated with `!` (inverts exit code)
    #[serde(default)]
    pub negated: bool,
}

/// A command that pipes its output to AI for processing
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipeAsk {
    /// The command whose output to send to AI
    pub command: Box<Statement>,
    /// The prompt for the AI (empty string if omitted)
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParallelExecution {
    pub commands: Vec<Command>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Assignment {
    pub name: String,
    pub value: Expression,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionDef {
    pub name: String,
    pub params: Vec<Parameter>,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub type_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IfStatement {
    pub condition: IfCondition,
    pub then_block: Vec<Statement>,
    pub elif_clauses: Vec<ElifClause>,
    pub else_block: Option<Vec<Statement>>,
}

/// Condition for an if statement: either shell-style (command exit code)
/// or expression-style (Rust-like expression evaluation).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IfCondition {
    /// Shell-style: condition is one or more commands; truthy if last exits 0
    Commands(Vec<Statement>),
    /// Rust-style: condition is an expression evaluated for truthiness
    Expression(Expression),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElifClause {
    pub condition: Vec<Statement>,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForLoop {
    pub variable: String,
    /// Word list to iterate over. Each argument is expanded individually
    /// (variables, globs, etc.). Empty means iterate over positional params.
    pub words: Vec<Argument>,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WhileLoop {
    pub condition: Vec<Statement>,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UntilLoop {
    pub condition: Vec<Statement>,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchExpression {
    pub value: Expression,
    pub arms: Vec<MatchArm>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaseStatement {
    pub word: Expression,
    pub arms: Vec<CaseArm>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaseArm {
    pub patterns: Vec<String>, // Multiple patterns separated by |
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConditionalAnd {
    pub left: Box<Statement>,
    pub right: Box<Statement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConditionalOr {
    pub left: Box<Statement>,
    pub right: Box<Statement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Pattern {
    Identifier(String),
    Literal(Literal),
    Wildcard,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expression {
    Literal(Literal),
    Variable(String),
    BinaryOp(BinaryOp),
    UnaryOp(UnaryOp),
    FunctionCall(FunctionCall),
    CommandSubstitution(String),
    MemberAccess(MemberAccess),
    VariableExpansion(VarExpansion),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VarExpansion {
    pub name: String,
    pub operator: VarExpansionOp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VarExpansionOp {
    /// ${VAR} - Simple expansion
    Simple,
    /// ${#VAR} - String length
    StringLength,
    /// ${VAR:-default} - Use default if unset
    UseDefault(String),
    /// ${VAR:=default} - Assign default if unset
    AssignDefault(String),
    /// ${VAR:?error} - Error if unset
    ErrorIfUnset(String),
    /// ${VAR:+alternate} - Use alternate if set and non-empty
    UseAlternate(String),
    /// ${VAR#pattern} - Remove shortest prefix match
    RemoveShortestPrefix(String),
    /// ${VAR##pattern} - Remove longest prefix match
    RemoveLongestPrefix(String),
    /// ${VAR%pattern} - Remove shortest suffix match
    RemoveShortestSuffix(String),
    /// ${VAR%%pattern} - Remove longest suffix match
    RemoveLongestSuffix(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BinaryOp {
    pub left: Box<Expression>,
    pub op: BinaryOperator,
    pub right: Box<Expression>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Equal,
    NotEqual,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnaryOp {
    pub op: UnaryOperator,
    pub operand: Box<Expression>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UnaryOperator {
    Not,
    Negate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub args: Vec<Expression>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemberAccess {
    pub object: Box<Expression>,
    pub member: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Literal {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ArgumentPart {
    Literal(String),
    Variable(String),
    BracedVariable(String),
    CommandSubstitution(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Argument {
    Literal(String),
    SingleQuoted(String),
    DoubleQuoted(Vec<ArgumentPart>),
    Variable(String),
    BracedVariable(String),
    CommandSubstitution(String),
    Flag(String),
    Path(String),
    Glob(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Redirect {
    pub kind: RedirectKind,
    pub target: Option<String>, // None for special cases like 2>&1
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RedirectKind {
    Stdout,         // >
    StdoutAppend,   // >>
    Stdin,          // <
    Stderr,         // 2>
    StderrToStdout, // 2>&1
    Both,           // &>
    HereDoc,        // <<WORD (body in target, expand vars)
    HereDocLiteral, // <<'WORD' or <<"WORD" (body in target, no expansion)
}
