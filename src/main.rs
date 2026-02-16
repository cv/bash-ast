//! bash-ast CLI tool
//!
//! Parses bash scripts and outputs JSON AST.

use bash_ast::server::{default_socket_path, Server};
use bash_ast::{init, parse_to_json, schema_json, to_bash, Command};
use std::env;
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const HELP: &str = r#"bash-ast - Parse bash scripts to JSON AST

USAGE:
    bash-ast [OPTIONS] [FILE]
    command | bash-ast [OPTIONS]
    bash-ast --server [SOCKET_PATH]

DESCRIPTION:
    Parses bash scripts using GNU Bash's actual parser (via FFI) and outputs
    a JSON representation of the Abstract Syntax Tree. This provides 100%
    compatibility with bash syntax.

ARGUMENTS:
    [FILE]    Bash script file to parse (or JSON AST with --to-bash).
              Use '-' to read from stdin explicitly.

OPTIONS:
    -h, --help             Print this help message and exit
    -V, --version          Print version information and exit
    -c, --compact          Output compact JSON (default: pretty-printed)
    -s, --schema           Print JSON Schema for the AST and exit
    -b, --to-bash          Convert JSON AST back to bash script
    -S, --server [PATH]    Start Unix socket server (default: $XDG_RUNTIME_DIR/bash-ast.sock)

EXAMPLES:
    # Parse a script file
    bash-ast script.sh

    # Parse from stdin (piped)
    echo 'echo hello' | bash-ast

    # Parse inline with here-string
    bash-ast <<< 'for i in a b c; do echo $i; done'

    # Read from stdin interactively (use '-' to wait for input)
    bash-ast -

    # Compact output for piping
    bash-ast -c script.sh | jq '.commands[]'

    # Print JSON Schema for the AST output
    bash-ast --schema > schema.json

    # Convert JSON AST back to bash
    bash-ast script.sh | bash-ast --to-bash

    # Start server mode (low-latency Unix socket)
    bash-ast --server
    bash-ast --server /tmp/my-bash-ast.sock

SERVER MODE:
    In server mode, bash-ast listens on a Unix socket for NDJSON requests.
    Each request/response is a single line of JSON.

    Methods:
      {"method":"parse","script":"echo hello"}     Parse bash to AST
      {"method":"to_bash","ast":{...}}             Convert AST to bash
      {"method":"schema"}                          Get JSON Schema
      {"method":"ping"}                            Health check

    Example client (bash):
      echo '{"method":"parse","script":"echo hi"}' | nc -U /tmp/bash-ast.sock

OUTPUT:
    On success, prints JSON AST to stdout and exits with code 0.
    On error, prints error message to stderr and exits with code 1.

    The JSON structure includes:
    - type: Command type (simple, pipeline, for, if, while, case, etc.)
    - line: Source line number
    - Command-specific fields (words, redirects, body, etc.)

    Use --schema to get a complete JSON Schema describing the output format.

SUPPORTED CONSTRUCTS:
    All bash command types are supported:
    • Simple commands      cmd arg1 arg2
    • Pipelines            cmd1 | cmd2
    • Lists                cmd1 && cmd2, cmd1 || cmd2, cmd1 ; cmd2
    • For loops            for var in list; do ...; done
    • C-style for          for ((i=0; i<n; i++)); do ...; done
    • While/Until loops    while cmd; do ...; done
    • If statements        if cmd; then ...; elif ...; else ...; fi
    • Case statements      case $var in pattern) ...; esac
    • Select statements    select var in list; do ...; done
    • Group commands       { cmd1; cmd2; }
    • Subshells            ( cmd1; cmd2 )
    • Functions            name() { ...; }
    • Arithmetic           (( expr ))
    • Conditionals         [[ expr ]]
    • Coprocesses          coproc name { ...; }

MORE INFO:
    Repository: https://github.com/cv/bash-ast
    License:    GPL-3.0 (due to linkage with GNU Bash)
"#;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let stdin = io::stdin();
    let is_tty = stdin.is_terminal();
    run(&args[1..], stdin.lock(), io::stdout(), io::stderr(), is_tty)
}

#[derive(Debug, Default)]
#[allow(clippy::struct_excessive_bools)]
struct Config {
    help: bool,
    version: bool,
    compact: bool,
    schema: bool,
    to_bash: bool,
    server: bool,
    socket_path: Option<String>,
    file: Option<String>,
}

fn parse_args(args: &[String]) -> Result<Config, String> {
    let mut config = Config::default();
    let mut positional = Vec::new();
    let mut args_iter = args.iter().peekable();

    while let Some(arg) = args_iter.next() {
        match arg.as_str() {
            "-h" | "--help" => config.help = true,
            "-V" | "--version" => config.version = true,
            "-c" | "--compact" => config.compact = true,
            "-s" | "--schema" => config.schema = true,
            "-b" | "--to-bash" => config.to_bash = true,
            "-S" | "--server" => {
                config.server = true;
                // Check if next arg is a socket path (not another option)
                if let Some(next) = args_iter.peek() {
                    if !next.starts_with('-') {
                        config.socket_path = Some(args_iter.next().unwrap().clone());
                    }
                }
            }
            "-" => positional.push(arg.clone()), // `-` means read from stdin
            s if s.starts_with('-') => {
                return Err(format!(
                    "Unknown option: {s}\nTry 'bash-ast --help' for usage."
                ));
            }
            _ => positional.push(arg.clone()),
        }
    }

    if config.server && !positional.is_empty() {
        return Err(
            "Cannot specify file when using --server mode.\nTry 'bash-ast --help' for usage."
                .to_string(),
        );
    }

    if positional.len() > 1 {
        return Err(
            "Too many arguments. Expected at most one file.\nTry 'bash-ast --help' for usage."
                .to_string(),
        );
    }

    config.file = positional.into_iter().next();
    Ok(config)
}

/// Run the CLI with the given arguments and input/output streams
fn run<R, W, E>(
    args: &[String],
    mut input: R,
    mut output: W,
    mut error: E,
    stdin_is_tty: bool,
) -> ExitCode
where
    R: BufRead,
    W: Write,
    E: Write,
{
    // Parse command line arguments
    let config = match parse_args(args) {
        Ok(c) => c,
        Err(e) => {
            let _ = writeln!(error, "Error: {e}");
            return ExitCode::from(2);
        }
    };

    // Handle --help, or show help if no file/stdin and stdin is a TTY (no piped input)
    // Users can use `-` to explicitly read from stdin even in a TTY
    let reading_stdin = config.file.as_deref() == Some("-");
    if config.help || (config.file.is_none() && !config.schema && stdin_is_tty && !reading_stdin) {
        let _ = write!(output, "{HELP}");
        return ExitCode::SUCCESS;
    }

    // Handle --version
    if config.version {
        let _ = writeln!(output, "bash-ast {VERSION}");
        return ExitCode::SUCCESS;
    }

    // Handle --schema
    if config.schema {
        let pretty = !config.compact;
        let _ = writeln!(output, "{}", schema_json(pretty));
        return ExitCode::SUCCESS;
    }

    // Handle --server
    if config.server {
        let socket_path = config.socket_path.unwrap_or_else(default_socket_path);
        let server = Server::with_path(&socket_path);
        if let Err(e) = server.run() {
            let _ = writeln!(error, "Server error: {e}");
            return ExitCode::from(1);
        }
        return ExitCode::SUCCESS;
    }

    // Read content from file or stdin (use "-" to explicitly read from stdin)
    let content = match config.file.as_deref() {
        Some("-") | None => {
            let mut content = String::new();
            if let Err(e) = input.read_to_string(&mut content) {
                let _ = writeln!(error, "Error reading stdin: {e}");
                return ExitCode::from(1);
            }
            content
        }
        Some(path) => match fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) => {
                let _ = writeln!(error, "Error reading '{path}': {e}");
                return ExitCode::from(1);
            }
        },
    };

    // Handle --to-bash: convert JSON AST to bash script
    if config.to_bash {
        let ast: Command = match serde_json::from_str(&content) {
            Ok(ast) => ast,
            Err(e) => {
                let _ = writeln!(error, "Error parsing JSON: {e}");
                return ExitCode::from(1);
            }
        };
        let _ = writeln!(output, "{}", to_bash(&ast));
        return ExitCode::SUCCESS;
    }

    // Initialize bash parser
    init();

    // Parse and output JSON
    let pretty = !config.compact;
    match parse_to_json(&content, pretty) {
        Ok(json) => {
            let _ = writeln!(output, "{json}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            let _ = writeln!(error, "Error: {e}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Test helper that captures output and runs the CLI
    struct TestRun {
        exit_code: ExitCode,
        stdout: String,
        stderr: String,
    }

    impl TestRun {
        /// Run CLI with given args and stdin content (simulates piped input)
        fn new(cli_args: &[&str], stdin: &str) -> Self {
            Self::with_tty(cli_args, stdin, false)
        }

        /// Run CLI with given args, stdin content, and TTY flag
        fn with_tty(cli_args: &[&str], stdin: &str, stdin_is_tty: bool) -> Self {
            let input = Cursor::new(stdin.to_string());
            let mut output = Vec::new();
            let mut error = Vec::new();

            let args: Vec<String> = cli_args.iter().map(|&s| s.to_string()).collect();
            let exit_code = run(&args, input, &mut output, &mut error, stdin_is_tty);

            Self {
                exit_code,
                stdout: String::from_utf8(output).unwrap(),
                stderr: String::from_utf8(error).unwrap(),
            }
        }

        fn success(&self) -> bool {
            self.exit_code == ExitCode::SUCCESS
        }
    }

    #[test]
    fn test_help_short() {
        let t = TestRun::new(&["-h"], "");
        assert!(t.success());
        assert!(t.stdout.contains("USAGE:"));
        assert!(t.stdout.contains("bash-ast"));
        assert!(t.stderr.is_empty());
    }

    #[test]
    fn test_help_long() {
        let t = TestRun::new(&["--help"], "");
        assert!(t.success());
        assert!(t.stdout.contains("EXAMPLES:"));
        assert!(t.stdout.contains("SUPPORTED CONSTRUCTS:"));
    }

    #[test]
    fn test_version_short() {
        let t = TestRun::new(&["-V"], "");
        assert!(t.success());
        assert!(t.stdout.contains("bash-ast"));
        assert!(t.stdout.contains(VERSION));
    }

    #[test]
    fn test_version_long() {
        let t = TestRun::new(&["--version"], "");
        assert!(t.success());
        assert!(t.stdout.starts_with("bash-ast "));
    }

    #[test]
    fn test_schema() {
        let t = TestRun::new(&["--schema"], "");
        assert!(t.success());
        assert!(t.stdout.contains("\"$schema\""));
        assert!(t.stdout.contains("\"title\": \"Command\""));
        assert!(t.stderr.is_empty());
    }

    #[test]
    fn test_schema_compact() {
        let t = TestRun::new(&["-s", "-c"], "");
        assert!(t.success());
        assert_eq!(t.stdout.lines().count(), 1);
        assert!(t.stdout.contains("\"$schema\""));
    }

    #[test]
    fn test_unknown_option() {
        let t = TestRun::new(&["--foo"], "");
        assert_eq!(t.exit_code, ExitCode::from(2));
        assert!(t.stderr.contains("Unknown option"));
        assert!(t.stderr.contains("--foo"));
    }

    #[test]
    fn test_too_many_args() {
        let t = TestRun::new(&["file1.sh", "file2.sh"], "");
        assert_eq!(t.exit_code, ExitCode::from(2));
        assert!(t.stderr.contains("Too many arguments"));
    }

    #[test]
    fn test_stdin_simple_command() {
        let t = TestRun::new(&[], "echo hello world");
        assert!(t.success());
        assert!(t.stdout.contains("\"type\": \"simple\""));
        assert!(t.stdout.contains("echo"));
        assert!(t.stderr.is_empty());
    }

    #[test]
    fn test_compact_output() {
        let t = TestRun::new(&["-c"], "echo hello");
        assert!(t.success());
        assert_eq!(t.stdout.lines().count(), 1);
        assert!(t.stdout.contains("\"type\":\"simple\""));
    }

    #[test]
    fn test_compact_long_option() {
        let t = TestRun::new(&["--compact"], "echo hello");
        assert!(t.success());
        assert_eq!(t.stdout.lines().count(), 1);
    }

    #[test]
    fn test_syntax_error() {
        let t = TestRun::new(&[], "if then fi");
        assert_eq!(t.exit_code, ExitCode::from(1));
        assert!(t.stderr.contains("Error"));
    }

    #[test]
    fn test_empty_input() {
        let t = TestRun::new(&[], "");
        assert_eq!(t.exit_code, ExitCode::from(1));
        assert!(t.stderr.contains("Error"));
    }

    #[test]
    fn test_file_not_found() {
        let t = TestRun::new(&["nonexistent.sh"], "");
        assert_eq!(t.exit_code, ExitCode::from(1));
        assert!(t.stderr.contains("Error reading"));
        assert!(t.stderr.contains("nonexistent.sh"));
    }

    #[test]
    fn test_complex_script() {
        let t = TestRun::new(&[], "for i in a b c; do echo $i; done");
        assert!(t.success());
        assert!(t.stdout.contains("\"type\": \"for\""));
        assert!(t.stderr.is_empty());
    }

    #[test]
    fn test_no_args_tty_shows_help() {
        // When stdin is a TTY and no args given, show help
        let t = TestRun::with_tty(&[], "", true);
        assert!(t.success());
        assert!(t.stdout.contains("USAGE:"));
        assert!(t.stdout.contains("bash-ast"));
        assert!(t.stderr.is_empty());
    }

    #[test]
    fn test_no_args_piped_empty_is_error() {
        // When stdin is piped (not TTY) but empty, it's an error
        let t = TestRun::with_tty(&[], "", false);
        assert_eq!(t.exit_code, ExitCode::from(1));
        assert!(t.stderr.contains("Error"));
    }

    #[test]
    fn test_dash_reads_stdin_even_on_tty() {
        // Using `-` explicitly reads from stdin, even if it's a TTY
        let t = TestRun::with_tty(&["-"], "echo hello", true);
        assert!(t.success());
        assert!(t.stdout.contains("\"type\": \"simple\""));
        assert!(t.stdout.contains("echo"));
    }

    #[test]
    fn test_dash_reads_stdin_piped() {
        // Using `-` works the same as no arg when piped
        let t = TestRun::new(&["-"], "echo hello");
        assert!(t.success());
        assert!(t.stdout.contains("\"type\": \"simple\""));
    }

    #[test]
    fn test_to_bash_simple() {
        // Test --to-bash converts JSON AST back to bash
        let json = r#"{"type":"simple","words":[{"word":"echo"},{"word":"hello"}],"redirects":[]}"#;
        let t = TestRun::new(&["--to-bash"], json);
        assert!(t.success());
        assert!(t.stdout.contains("echo hello"));
        assert!(t.stderr.is_empty());
    }

    #[test]
    fn test_to_bash_short_option() {
        // Test -b option
        let json = r#"{"type":"simple","words":[{"word":"ls"},{"word":"-la"}],"redirects":[]}"#;
        let t = TestRun::new(&["-b"], json);
        assert!(t.success());
        assert!(t.stdout.contains("ls -la"));
    }

    #[test]
    fn test_to_bash_invalid_json() {
        // Invalid JSON should error
        let t = TestRun::new(&["--to-bash"], "not valid json");
        assert_eq!(t.exit_code, ExitCode::from(1));
        assert!(t.stderr.contains("Error parsing JSON"));
    }

    #[test]
    fn test_to_bash_complex() {
        // Test a more complex AST
        let json = r#"{"type":"for","variable":"i","words":["a","b","c"],"body":{"type":"simple","words":[{"word":"echo"},{"word":"$i"}],"redirects":[]}}"#;
        let t = TestRun::new(&["--to-bash"], json);
        assert!(t.success());
        assert!(t.stdout.contains("for i in a b c; do echo $i; done"));
    }

    // ==================== Server Option Tests ====================

    #[test]
    fn test_help_shows_server_option() {
        // Help text should mention --server option
        let t = TestRun::new(&["--help"], "");
        assert!(t.success());
        assert!(t.stdout.contains("--server"));
        assert!(t.stdout.contains("-S"));
        assert!(t.stdout.contains("SERVER MODE"));
        assert!(t.stdout.contains("Unix socket"));
    }

    #[test]
    fn test_parse_args_server_default_path() {
        // --server without path should use default
        let args: Vec<String> = vec!["--server".to_string()];
        let config = parse_args(&args).unwrap();
        assert!(config.server);
        assert!(config.socket_path.is_none());
    }

    #[test]
    fn test_parse_args_server_with_path() {
        // --server with path should capture the path
        let args: Vec<String> = vec!["--server".to_string(), "/tmp/my.sock".to_string()];
        let config = parse_args(&args).unwrap();
        assert!(config.server);
        assert_eq!(config.socket_path, Some("/tmp/my.sock".to_string()));
    }

    #[test]
    fn test_parse_args_server_short() {
        // -S should work same as --server
        let args: Vec<String> = vec!["-S".to_string()];
        let config = parse_args(&args).unwrap();
        assert!(config.server);
        assert!(config.socket_path.is_none());
    }

    #[test]
    fn test_parse_args_server_short_with_path() {
        // -S with path should capture the path
        let args: Vec<String> = vec!["-S".to_string(), "/custom/path.sock".to_string()];
        let config = parse_args(&args).unwrap();
        assert!(config.server);
        assert_eq!(config.socket_path, Some("/custom/path.sock".to_string()));
    }

    #[test]
    fn test_parse_args_server_ignores_options_as_path() {
        // --server followed by another option should not consume it as path
        let args: Vec<String> = vec!["--server".to_string(), "--compact".to_string()];
        let config = parse_args(&args).unwrap();
        assert!(config.server);
        assert!(config.socket_path.is_none());
        assert!(config.compact);
    }

    #[test]
    fn test_parse_args_server_with_positional_error() {
        // --server mode followed by a positional after another option should error
        // e.g., --server --compact script.sh
        let args: Vec<String> = vec![
            "--server".to_string(),
            "--compact".to_string(),
            "script.sh".to_string(),
        ];
        let result = parse_args(&args);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Cannot specify file"));
    }
}
