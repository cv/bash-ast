//! Rust AST types for representing bash commands
//!
//! These types mirror bash's internal command representation but in idiomatic Rust.
//! They are serializable to JSON via serde.

use schemars::JsonSchema;
use serde::Serialize;

/// A bash command - the top-level AST node
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    /// Simple command: `cmd arg1 arg2`
    Simple {
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        words: Vec<Word>,
        redirects: Vec<Redirect>,
        #[serde(skip_serializing_if = "Option::is_none")]
        assignments: Option<Vec<String>>,
    },

    /// Pipeline: `cmd1 | cmd2 | cmd3`
    Pipeline {
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        commands: Vec<Self>,
        /// True if pipeline is negated with !
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        negated: bool,
    },

    /// List/connection: commands joined by &&, ||, ;, &
    List {
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        op: ListOp,
        left: Box<Self>,
        right: Box<Self>,
    },

    /// For loop: `for var in list; do ...; done`
    For {
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        variable: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        words: Option<Vec<String>>,
        body: Box<Self>,
    },

    /// While loop: `while test; do ...; done`
    While {
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        test: Box<Self>,
        body: Box<Self>,
    },

    /// Until loop: `until test; do ...; done`
    Until {
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        test: Box<Self>,
        body: Box<Self>,
    },

    /// If statement: `if test; then ...; [elif ...; then ...;] [else ...;] fi`
    If {
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        condition: Box<Self>,
        then_branch: Box<Self>,
        #[serde(skip_serializing_if = "Option::is_none")]
        else_branch: Option<Box<Self>>,
    },

    /// Case statement: `case word in pattern) ...;; esac`
    Case {
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        word: String,
        clauses: Vec<CaseClause>,
    },

    /// Select statement: `select var in list; do ...; done`
    Select {
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        variable: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        words: Option<Vec<String>>,
        body: Box<Self>,
    },

    /// Brace group: `{ ...; }`
    Group {
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        body: Box<Self>,
    },

    /// Subshell: `( ... )`
    Subshell {
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        body: Box<Self>,
    },

    /// Function definition: `name() { ...; }`
    FunctionDef {
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        name: String,
        body: Box<Self>,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_file: Option<String>,
    },

    /// Arithmetic evaluation: `(( expr ))`
    Arithmetic {
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        expression: String,
    },

    /// C-style for loop: `for ((init; test; step)); do ...; done`
    ArithmeticFor {
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        init: String,
        test: String,
        step: String,
        body: Box<Self>,
    },

    /// Conditional expression: `[[ expr ]]`
    Conditional {
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        expr: ConditionalExpr,
    },

    /// Coprocess: `coproc [name] { ...; }`
    Coproc {
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        body: Box<Self>,
    },
}

/// A word in a command
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Word {
    /// The word text
    pub word: String,
    /// Word flags (`W_HASDOLLAR`, `W_QUOTED`, etc.)
    #[serde(skip_serializing_if = "is_zero")]
    pub flags: u32,
}

/// Helper for serde `skip_serializing_if` (requires reference signature)
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// List operator connecting commands
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ListOp {
    /// `&&` - AND list
    And,
    /// `||` - OR list
    Or,
    /// `;` - Sequential
    Semi,
    /// `&` - Asynchronous
    Amp,
    /// Newline separator
    Newline,
}

/// A case clause in a case statement
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CaseClause {
    /// Patterns to match
    pub patterns: Vec<String>,
    /// Command to execute if matched
    pub action: Option<Box<Command>>,
    /// Clause flags (fallthrough, testnext)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<CaseClauseFlags>,
}

/// Flags for case clause behavior
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CaseClauseFlags {
    /// `;&` - fallthrough to next clause
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub fallthrough: bool,
    /// `;;&` - test next pattern
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub test_next: bool,
}

/// A redirect operation
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Redirect {
    /// The type of redirection
    pub direction: RedirectType,
    /// Source file descriptor (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_fd: Option<i32>,
    /// Target (filename or fd number)
    pub target: RedirectTarget,
    /// For here-documents, the delimiter word
    #[serde(skip_serializing_if = "Option::is_none")]
    pub here_doc_eof: Option<String>,
}

/// Redirect direction/type
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RedirectType {
    /// `<` - input redirection
    Input,
    /// `>` - output redirection
    Output,
    /// `>>` - append output
    Append,
    /// `<<` - here-document
    HereDoc,
    /// `<<<` - here-string
    HereString,
    /// `<>` - open for reading and writing
    InputOutput,
    /// `>|` - clobber (force overwrite)
    Clobber,
    /// `<&` - duplicate input fd
    DupInput,
    /// `>&` - duplicate output fd
    DupOutput,
    /// `<&-` or `>&-` - close fd
    Close,
    /// `&>` or `>&` - redirect stdout and stderr
    ErrAndOut,
    /// `&>>` - append stdout and stderr
    AppendErrAndOut,
    /// `<&n-` - move input fd
    MoveInput,
    /// `>&n-` - move output fd
    MoveOutput,
}

/// Target of a redirect
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum RedirectTarget {
    /// A filename
    File(String),
    /// A file descriptor number
    Fd(i32),
}

/// Conditional expression for [[ ... ]]
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "cond_type", rename_all = "snake_case")]
pub enum ConditionalExpr {
    /// `[[ -flag arg ]]` - unary test
    Unary { op: String, arg: String },
    /// `[[ arg1 op arg2 ]]` - binary test
    Binary {
        op: String,
        left: String,
        right: String,
    },
    /// `[[ expr1 && expr2 ]]` - AND
    And { left: Box<Self>, right: Box<Self> },
    /// `[[ expr1 || expr2 ]]` - OR
    Or { left: Box<Self>, right: Box<Self> },
    /// `[[ ! expr ]]` - negation (inside the expression)
    Not { expr: Box<Self> },
    /// A single word/term in a conditional
    Term { word: String },
    /// A grouped expression `( expr )`
    Expr { expr: Box<Self> },
}

impl Command {
    /// Get the line number where this command starts, if known
    #[must_use]
    pub const fn line(&self) -> Option<u32> {
        match self {
            Self::Simple { line, .. }
            | Self::Pipeline { line, .. }
            | Self::List { line, .. }
            | Self::For { line, .. }
            | Self::While { line, .. }
            | Self::Until { line, .. }
            | Self::If { line, .. }
            | Self::Case { line, .. }
            | Self::Select { line, .. }
            | Self::Group { line, .. }
            | Self::Subshell { line, .. }
            | Self::FunctionDef { line, .. }
            | Self::Arithmetic { line, .. }
            | Self::ArithmeticFor { line, .. }
            | Self::Conditional { line, .. }
            | Self::Coproc { line, .. } => *line,
        }
    }
}
