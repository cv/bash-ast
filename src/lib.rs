//! bash-ast: Parse bash scripts to JSON AST using bash's actual parser
//!
//! This crate provides FFI bindings to GNU Bash's parser, allowing you to
//! parse bash scripts into a JSON-serializable AST representation with
//! 100% compatibility with bash syntax.
//!
//! # Example
//!
//! ```no_run
//! use bash_ast::{parse, init};
//!
//! // Initialize bash (call once at startup)
//! init();
//!
//! // Parse a script
//! let ast = parse("echo hello world").unwrap();
//!
//! // Serialize to JSON
//! let json = serde_json::to_string_pretty(&ast).unwrap();
//! println!("{}", json);
//! ```
//!
//! # Thread Safety
//!
//! **Important:** This crate is NOT thread-safe. The underlying bash parser
//! uses global state that cannot be safely accessed from multiple threads.
//!
//! - Call [`init()`] once from your main thread before parsing
//! - Perform all parsing operations from a single thread
//! - For tests, set `RUST_TEST_THREADS=1` or use `cargo test -- --test-threads=1`
//!
//! The [`init()`] function uses `std::sync::Once` internally, making it safe
//! to call multiple times (subsequent calls are no-ops).
//!
//! # License
//!
//! This crate is licensed under GPL-3.0 due to its linkage with GNU Bash.

mod ast;
mod bash_init;
mod convert;
mod ffi;

pub use ast::*;

use std::ffi::CString;
use thiserror::Error;

/// Maximum script size in bytes (10MB)
///
/// Scripts larger than this will be rejected with `ParseError::InputTooLarge`
/// to prevent resource exhaustion attacks.
pub const MAX_SCRIPT_SIZE: usize = 10 * 1024 * 1024;

/// Errors that can occur during parsing
#[derive(Debug, Error)]
pub enum ParseError {
    /// The input contained a syntax error
    #[error("Syntax error in script{}", .0.as_ref().map(|d| format!(": {d}")).unwrap_or_default())]
    SyntaxError(Option<String>),

    /// Failed to convert the parsed AST
    #[error("Failed to convert AST to Rust types{}", .0.as_ref().map(|d| format!(": {d}")).unwrap_or_default())]
    ConversionError(Option<String>),

    /// The input contained a NUL byte
    #[error("Invalid string: {0}")]
    InvalidString(#[from] std::ffi::NulError),

    /// The input was empty
    #[error("Empty input")]
    EmptyInput,

    /// The input exceeded the maximum allowed size
    #[error("Input too large (max {} bytes)", MAX_SCRIPT_SIZE)]
    InputTooLarge,
}

/// Initialize bash internals for parsing
///
/// This must be called once before any parsing operations.
/// It is safe to call multiple times - subsequent calls are no-ops.
///
/// # Thread Safety
///
/// While this function is safe to call from multiple threads (it uses
/// `std::sync::Once` internally), the actual parsing operations are
/// NOT thread-safe. Call this from your main thread, then ensure all
/// parsing happens on a single thread.
///
/// # Example
///
/// ```no_run
/// use bash_ast::init;
///
/// init();
/// // Now you can parse scripts (from a single thread)
/// ```
pub fn init() {
    bash_init::init();
}

/// Test utilities for bash-ast
///
/// This module provides helper functions for writing tests that use bash-ast.
/// These utilities handle initialization and provide better error messages.
///
/// # Example
///
/// ```no_run
/// use bash_ast::test_utils;
///
/// #[test]
/// fn my_test() {
///     test_utils::setup();
///     // Your test code here
/// }
/// ```
pub mod test_utils {
    use super::init;

    /// Initialize bash for testing
    ///
    /// This is a convenience function for tests that calls [`init()`].
    /// It's safe to call multiple times.
    ///
    /// **Important:** Tests using bash-ast must run single-threaded.
    /// Configure this via:
    /// - `.cargo/config.toml`: `RUST_TEST_THREADS = "1"`
    /// - Command line: `cargo test -- --test-threads=1`
    ///
    /// # Example
    ///
    /// ```no_run
    /// use bash_ast::test_utils;
    ///
    /// #[test]
    /// fn test_parsing() {
    ///     test_utils::setup();
    ///     let result = bash_ast::parse("echo hello");
    ///     assert!(result.is_ok());
    /// }
    /// ```
    pub fn setup() {
        init();
    }
}

/// Parse a bash script and return the AST
///
/// # Arguments
///
/// * `script` - The bash script to parse
///
/// # Returns
///
/// Returns the parsed command AST, or an error if parsing fails.
///
/// # Example
///
/// ```no_run
/// use bash_ast::{parse, init, Command};
///
/// init();
///
/// let cmd = parse("echo hello").unwrap();
/// match cmd {
///     Command::Simple { words, .. } => {
///         assert_eq!(words[0].word, "echo");
///         assert_eq!(words[1].word, "hello");
///     }
///     _ => panic!("Expected simple command"),
/// }
/// ```
///
/// # Errors
///
/// Returns `ParseError::InputTooLarge` if the script exceeds `MAX_SCRIPT_SIZE`.
/// Returns `ParseError::SyntaxError` if the script contains invalid bash syntax.
/// Returns `ParseError::InvalidString` if the script contains NUL bytes.
/// Returns `ParseError::EmptyInput` if the script is empty.
pub fn parse(script: &str) -> Result<Command, ParseError> {
    parse_internal(script, false)
}

/// Parse a bash script with error messages printed to stderr
///
/// Like `parse()`, but allows bash to print syntax error messages to stderr.
/// Useful for debugging or when you need detailed error information
/// (line numbers, unexpected tokens, etc.)
///
/// # Arguments
///
/// * `script` - The bash script to parse
///
/// # Returns
///
/// Returns the parsed command AST, or an error if parsing fails.
/// On syntax errors, bash will print details to stderr before returning.
///
/// # Example
///
/// ```no_run
/// use bash_ast::{parse_verbose, init};
///
/// init();
///
/// // This will print error details to stderr
/// let result = parse_verbose("if then fi");
/// assert!(result.is_err());
/// ```
pub fn parse_verbose(script: &str) -> Result<Command, ParseError> {
    parse_internal(script, true)
}

/// Internal parse implementation shared by `parse()` and `parse_verbose()`
fn parse_internal(script: &str, verbose: bool) -> Result<Command, ParseError> {
    if script.len() > MAX_SCRIPT_SIZE {
        return Err(ParseError::InputTooLarge);
    }

    if script.trim().is_empty() {
        return Err(ParseError::EmptyInput);
    }

    let c_script = CString::new(script)?;

    // SAFETY: safe_parse_script/safe_parse_verbose are C wrappers that use
    // setjmp/longjmp to safely catch parser errors. The CString is valid for
    // the duration of the call.
    unsafe {
        let cmd_ptr = if verbose {
            ffi::safe_parse_verbose(c_script.as_ptr().cast_mut(), 0)
        } else {
            ffi::safe_parse_script(c_script.as_ptr().cast_mut(), 0)
        };

        if cmd_ptr.is_null() {
            return Err(ParseError::SyntaxError(None));
        }

        let result = convert::convert_command(cmd_ptr).ok_or(ParseError::ConversionError(None));

        // Clean up the parsed command
        ffi::dispose_command(cmd_ptr);

        // Unwrap the artificial group that safe_parse_script adds (not for verbose)
        if verbose {
            result
        } else {
            result.map(unwrap_script_group)
        }
    }
}

/// Unwrap the group that `safe_parse_script` adds around scripts
///
/// Since we wrap scripts in `{ ... }` to parse them as a single command,
/// we need to unwrap that group to return the actual script content.
fn unwrap_script_group(cmd: Command) -> Command {
    match cmd {
        Command::Group { body, .. } => *body,
        other => other,
    }
}

/// Parse a bash script and return the AST as JSON
///
/// This is a convenience function that parses the script and
/// serializes the result to a JSON string.
///
/// # Arguments
///
/// * `script` - The bash script to parse
/// * `pretty` - If true, format the JSON with indentation
///
/// # Returns
///
/// Returns the JSON string representation of the AST.
///
/// # Example
///
/// ```no_run
/// use bash_ast::{parse_to_json, init};
///
/// init();
///
/// let json = parse_to_json("echo hello", true).unwrap();
/// println!("{}", json);
/// ```
pub fn parse_to_json(script: &str, pretty: bool) -> Result<String, Box<dyn std::error::Error>> {
    let ast = parse(script)?;

    let json = if pretty {
        serde_json::to_string_pretty(&ast)?
    } else {
        serde_json::to_string(&ast)?
    };

    Ok(json)
}

/// Generate JSON Schema for the Command AST
///
/// Returns a JSON Schema (draft-07) describing the structure of the AST
/// that `parse_to_json` outputs. This is useful for validation and
/// documentation.
///
/// # Example
///
/// ```no_run
/// use bash_ast::schema_json;
///
/// let schema = schema_json(true);
/// println!("{}", schema);
/// ```
///
/// # Arguments
///
/// * `pretty` - If true, format the JSON with indentation
#[must_use]
pub fn schema_json(pretty: bool) -> String {
    let schema = schemars::schema_for!(Command);
    if pretty {
        serde_json::to_string_pretty(&schema).expect("schema serialization cannot fail")
    } else {
        serde_json::to_string(&schema).expect("schema serialization cannot fail")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() {
        init();
    }

    #[test]
    fn test_simple_command() {
        setup();
        let cmd = parse("echo hello world").unwrap();

        if let Command::Simple { words, .. } = cmd {
            assert_eq!(words.len(), 3);
            assert_eq!(words[0].word, "echo");
            assert_eq!(words[1].word, "hello");
            assert_eq!(words[2].word, "world");
        } else {
            panic!("Expected Simple command");
        }
    }

    #[test]
    fn test_pipeline() {
        setup();
        let cmd = parse("cat file | grep pattern | wc -l").unwrap();

        if let Command::Pipeline { commands, .. } = cmd {
            assert_eq!(commands.len(), 3);
        } else {
            panic!("Expected Pipeline command");
        }
    }

    #[test]
    fn test_for_loop() {
        setup();
        let cmd = parse("for i in a b c; do echo $i; done").unwrap();

        if let Command::For {
            variable, words, ..
        } = cmd
        {
            assert_eq!(variable, "i");
            assert_eq!(
                words,
                Some(vec!["a".to_string(), "b".to_string(), "c".to_string()])
            );
        } else {
            panic!("Expected For command");
        }
    }

    #[test]
    fn test_if_statement() {
        setup();
        let cmd = parse("if test -f file; then echo exists; fi").unwrap();

        if let Command::If { .. } = cmd {
            // Success
        } else {
            panic!("Expected If command");
        }
    }

    #[test]
    fn test_function_def() {
        setup();
        let cmd = parse("foo() { echo bar; }").unwrap();

        if let Command::FunctionDef { name, .. } = cmd {
            assert_eq!(name, "foo");
        } else {
            panic!("Expected FunctionDef command");
        }
    }

    #[test]
    fn test_syntax_error() {
        setup();
        // This should return an error instead of crashing, thanks to
        // safe_parse_string_to_command which catches the longjmp
        let result = parse("if then fi");
        assert!(matches!(result, Err(ParseError::SyntaxError(_))));
    }

    #[test]
    fn test_empty_input() {
        setup();
        let result = parse("");
        assert!(matches!(result, Err(ParseError::EmptyInput)));
    }

    #[test]
    fn test_json_output() {
        setup();
        let json = parse_to_json("echo hello", false).unwrap();
        assert!(json.contains("\"type\":\"simple\""));
        assert!(json.contains("echo"));
        assert!(json.contains("hello"));
    }
}
