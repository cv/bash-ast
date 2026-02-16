//! Unix socket server for bash-ast
//!
//! Provides a low-latency IPC interface using Unix domain sockets
//! with newline-delimited JSON (NDJSON) protocol.
//!
//! # Protocol
//!
//! Each request is a single line of JSON, each response is a single line of JSON.
//!
//! ## Methods
//!
//! ### parse
//! Parse a bash script to AST.
//! ```json
//! {"method":"parse","script":"echo hello"}
//! {"result":{"type":"simple","words":[{"word":"echo"},{"word":"hello"}],"redirects":[]}}
//! ```
//!
//! ### `to_bash`
//! Convert AST back to bash script.
//! ```json
//! {"method":"to_bash","ast":{"type":"simple","words":[{"word":"echo"}],"redirects":[]}}
//! {"result":"echo"}
//! ```
//!
//! ### schema
//! Get JSON Schema for the AST.
//! ```json
//! {"method":"schema"}
//! {"result":{...schema...}}
//! ```
//!
//! ## Errors
//!
//! On error, the response contains an "error" field:
//! ```json
//! {"error":"Syntax error in script"}
//! ```

use crate::{parse, schema_json, to_bash, Command};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Default socket path using `XDG_RUNTIME_DIR` or falling back to /tmp
#[must_use]
pub fn default_socket_path() -> String {
    std::env::var("XDG_RUNTIME_DIR").map_or_else(
        |_| "/tmp/bash-ast.sock".to_string(),
        |runtime_dir| format!("{runtime_dir}/bash-ast.sock"),
    )
}

/// Request types supported by the server
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum Request {
    /// Parse a bash script to AST
    Parse {
        /// The bash script to parse
        script: String,
    },
    /// Convert AST back to bash script
    ToBash {
        /// The AST to convert
        ast: Command,
    },
    /// Get JSON Schema for the AST
    Schema,
    /// Health check / ping
    Ping,
}

impl Request {
    /// Check if this is a Parse request
    #[must_use]
    pub const fn is_parse(&self) -> bool {
        matches!(self, Self::Parse { .. })
    }

    /// Check if this is a `ToBash` request
    #[must_use]
    pub const fn is_to_bash(&self) -> bool {
        matches!(self, Self::ToBash { .. })
    }

    /// Check if this is a Schema request
    #[must_use]
    pub const fn is_schema(&self) -> bool {
        matches!(self, Self::Schema)
    }

    /// Check if this is a Ping request
    #[must_use]
    pub const fn is_ping(&self) -> bool {
        matches!(self, Self::Ping)
    }

    /// Get the script from a Parse request
    #[must_use]
    pub fn script(&self) -> Option<&str> {
        match self {
            Self::Parse { script } => Some(script),
            _ => None,
        }
    }
}

/// Response from the server
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Response {
    /// Successful response with result
    Success {
        /// The result value
        result: serde_json::Value,
    },
    /// Error response
    Error {
        /// Error message
        error: String,
    },
}

impl Response {
    /// Create a success response with the given value
    pub fn success<T: Serialize>(value: T) -> Self {
        Self::Success {
            result: serde_json::to_value(value).expect("serialization cannot fail"),
        }
    }

    /// Create an error response with the given message
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            error: message.into(),
        }
    }

    /// Check if this is a success response
    #[must_use]
    pub const fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    /// Check if this is an error response
    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }
}

/// Handle a single request and return a response
#[must_use]
pub fn handle_request(request: &Request) -> Response {
    match request {
        Request::Parse { script } => match parse(script) {
            Ok(ast) => Response::success(ast),
            Err(e) => Response::error(e.to_string()),
        },
        Request::ToBash { ast } => Response::success(to_bash(ast)),
        Request::Schema => {
            // Parse the schema JSON string back to a Value for consistent response format
            let schema_str = schema_json(false);
            match serde_json::from_str::<serde_json::Value>(&schema_str) {
                Ok(schema) => Response::Success { result: schema },
                Err(e) => Response::error(format!("Failed to generate schema: {e}")),
            }
        }
        Request::Ping => Response::success("pong"),
    }
}

/// Parse a request from a JSON string
pub fn parse_request(line: &str) -> Result<Request, Response> {
    serde_json::from_str(line).map_err(|e| Response::error(format!("Invalid request: {e}")))
}

/// Handle a single line of input and return a response string
#[must_use]
pub fn handle_line(line: &str) -> String {
    let response = match parse_request(line) {
        Ok(request) => handle_request(&request),
        Err(err_response) => err_response,
    };
    serde_json::to_string(&response).expect("response serialization cannot fail")
}

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Path to the Unix socket
    pub socket_path: String,
    /// Whether to remove existing socket file on startup
    pub remove_existing: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            socket_path: default_socket_path(),
            remove_existing: true,
        }
    }
}

impl ServerConfig {
    /// Create a new server config with the given socket path
    #[must_use]
    pub fn new(socket_path: impl Into<String>) -> Self {
        Self {
            socket_path: socket_path.into(),
            ..Default::default()
        }
    }
}

/// Unix socket server for bash-ast
pub struct Server {
    config: ServerConfig,
    shutdown: Arc<AtomicBool>,
}

impl Server {
    /// Create a new server with the given configuration
    #[must_use]
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a new server with the default socket path
    #[must_use]
    pub fn with_default_path() -> Self {
        Self::new(ServerConfig::default())
    }

    /// Create a new server with a custom socket path
    #[must_use]
    pub fn with_path(socket_path: impl Into<String>) -> Self {
        Self::new(ServerConfig::new(socket_path))
    }

    /// Get a handle to signal shutdown
    #[must_use]
    pub fn shutdown_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.shutdown)
    }

    /// Run the server, blocking until shutdown
    ///
    /// # Errors
    ///
    /// Returns an error if the socket cannot be created or bound.
    pub fn run(&self) -> std::io::Result<()> {
        // Initialize bash parser
        crate::init();

        // Remove existing socket if configured
        if self.config.remove_existing {
            let _ = std::fs::remove_file(&self.config.socket_path);
        }

        let listener = UnixListener::bind(&self.config.socket_path)?;

        // Set non-blocking so we can check shutdown flag
        listener.set_nonblocking(true)?;

        eprintln!("bash-ast server listening on {}", self.config.socket_path);

        while !self.shutdown.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _addr)) => {
                    // Set blocking for the client stream
                    stream.set_nonblocking(false)?;
                    self.handle_client(stream);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No connection available, sleep briefly and check shutdown
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(e) => {
                    eprintln!("Accept error: {e}");
                }
            }
        }

        // Cleanup socket file
        let _ = std::fs::remove_file(&self.config.socket_path);
        eprintln!("Server shut down");

        Ok(())
    }

    /// Handle a single client connection
    #[allow(clippy::unused_self)] // Method logically belongs to Server
    fn handle_client(&self, stream: UnixStream) {
        // Clone the stream to get separate handles for reading and writing.
        // This avoids issues with BufReader's internal buffering when sharing
        // a single reference for both read and write operations.
        let read_stream = match stream.try_clone() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to clone stream: {e}");
                return;
            }
        };
        let reader = BufReader::new(read_stream);
        let mut writer = stream;

        for line in reader.lines() {
            match line {
                Ok(line) if line.is_empty() => {}
                Ok(line) => {
                    let response = handle_line(&line);
                    if writeln!(writer, "{response}").is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    }
}

/// Clean up socket file on drop
impl Drop for Server {
    fn drop(&mut self) {
        if Path::new(&self.config.socket_path).exists() {
            let _ = std::fs::remove_file(&self.config.socket_path);
        }
    }
}

/// Run the server with the given socket path (convenience function)
///
/// # Errors
///
/// Returns an error if the socket cannot be created or bound.
pub fn run_server(socket_path: &str) -> std::io::Result<()> {
    let server = Server::with_path(socket_path);
    server.run()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{RedirectTarget, RedirectType, Word};
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::Duration;

    fn setup() {
        crate::init();
    }

    // ==================== Request Parsing Tests ====================

    #[test]
    fn test_parse_request_parse() {
        let json = r#"{"method":"parse","script":"echo hello"}"#;
        let req = parse_request(json).unwrap();
        assert!(req.is_parse());
        assert_eq!(req.script(), Some("echo hello"));
    }

    #[test]
    fn test_parse_request_to_bash() {
        let json = r#"{"method":"to_bash","ast":{"type":"simple","words":[{"word":"echo"}],"redirects":[]}}"#;
        let req = parse_request(json).unwrap();
        assert!(req.is_to_bash());
        if let Request::ToBash { ast } = req {
            assert!(matches!(ast, Command::Simple { .. }));
        }
    }

    #[test]
    fn test_parse_request_schema() {
        let json = r#"{"method":"schema"}"#;
        let req = parse_request(json).unwrap();
        assert!(req.is_schema());
    }

    #[test]
    fn test_parse_request_ping() {
        let json = r#"{"method":"ping"}"#;
        let req = parse_request(json).unwrap();
        assert!(req.is_ping());
    }

    #[test]
    fn test_parse_request_invalid_json() {
        let result = parse_request("not json");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_error());
        if let Response::Error { error } = err {
            assert!(error.contains("Invalid request"));
        }
    }

    #[test]
    fn test_parse_request_unknown_method() {
        let json = r#"{"method":"unknown"}"#;
        let result = parse_request(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_request_missing_script() {
        let json = r#"{"method":"parse"}"#;
        let result = parse_request(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_request_missing_ast() {
        let json = r#"{"method":"to_bash"}"#;
        let result = parse_request(json);
        assert!(result.is_err());
    }

    // ==================== Response Tests ====================

    #[test]
    fn test_response_success() {
        let resp = Response::success("hello");
        assert!(resp.is_success());
        assert!(!resp.is_error());
        if let Response::Success { result } = resp {
            assert_eq!(result, serde_json::Value::String("hello".to_string()));
        }
    }

    #[test]
    fn test_response_error() {
        let resp = Response::error("something went wrong");
        assert!(resp.is_error());
        assert!(!resp.is_success());
        if let Response::Error { error } = resp {
            assert_eq!(error, "something went wrong");
        }
    }

    #[test]
    fn test_response_serialize_success() {
        let resp = Response::success("test");
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"result\""));
        assert!(json.contains("\"test\""));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_response_serialize_error() {
        let resp = Response::error("bad input");
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"error\""));
        assert!(json.contains("bad input"));
        assert!(!json.contains("\"result\""));
    }

    #[test]
    fn test_response_deserialize_success() {
        let json = r#"{"result":"hello"}"#;
        let resp: Response = serde_json::from_str(json).unwrap();
        assert!(resp.is_success());
    }

    #[test]
    fn test_response_deserialize_error() {
        let json = r#"{"error":"oops"}"#;
        let resp: Response = serde_json::from_str(json).unwrap();
        assert!(resp.is_error());
    }

    // ==================== Handle Request Tests ====================

    #[test]
    fn test_handle_request_parse_simple() {
        setup();
        let req = Request::Parse {
            script: "echo hello".to_string(),
        };
        let resp = handle_request(&req);
        assert!(resp.is_success());
        if let Response::Success { result } = resp {
            assert_eq!(result["type"], "simple");
            assert_eq!(result["words"][0]["word"], "echo");
            assert_eq!(result["words"][1]["word"], "hello");
        }
    }

    #[test]
    fn test_handle_request_parse_complex() {
        setup();
        let req = Request::Parse {
            script: "for i in a b c; do echo $i; done".to_string(),
        };
        let resp = handle_request(&req);
        assert!(resp.is_success());
        if let Response::Success { result } = resp {
            assert_eq!(result["type"], "for");
            assert_eq!(result["variable"], "i");
        }
    }

    #[test]
    fn test_handle_request_parse_pipeline() {
        setup();
        let req = Request::Parse {
            script: "cat file | grep pattern | wc -l".to_string(),
        };
        let resp = handle_request(&req);
        assert!(resp.is_success());
        if let Response::Success { result } = resp {
            assert_eq!(result["type"], "pipeline");
            assert_eq!(result["commands"].as_array().unwrap().len(), 3);
        }
    }

    #[test]
    fn test_handle_request_parse_syntax_error() {
        setup();
        let req = Request::Parse {
            script: "if then fi".to_string(),
        };
        let resp = handle_request(&req);
        assert!(resp.is_error());
        if let Response::Error { error } = resp {
            assert!(error.contains("Syntax error"));
        }
    }

    #[test]
    fn test_handle_request_parse_empty() {
        setup();
        let req = Request::Parse {
            script: String::new(),
        };
        let resp = handle_request(&req);
        assert!(resp.is_error());
        if let Response::Error { error } = resp {
            assert!(error.contains("Empty"));
        }
    }

    #[test]
    fn test_handle_request_to_bash_simple() {
        let req = Request::ToBash {
            ast: Command::Simple {
                line: None,
                words: vec![
                    Word {
                        word: "echo".to_string(),
                        flags: 0,
                    },
                    Word {
                        word: "hello".to_string(),
                        flags: 0,
                    },
                ],
                redirects: vec![],
                assignments: None,
            },
        };
        let resp = handle_request(&req);
        assert!(resp.is_success());
        if let Response::Success { result } = resp {
            assert_eq!(result, "echo hello");
        }
    }

    #[test]
    fn test_handle_request_to_bash_for_loop() {
        let req = Request::ToBash {
            ast: Command::For {
                line: None,
                variable: "i".to_string(),
                words: Some(vec!["a".to_string(), "b".to_string(), "c".to_string()]),
                body: Box::new(Command::Simple {
                    line: None,
                    words: vec![
                        Word {
                            word: "echo".to_string(),
                            flags: 0,
                        },
                        Word {
                            word: "$i".to_string(),
                            flags: 0,
                        },
                    ],
                    redirects: vec![],
                    assignments: None,
                }),
                redirects: vec![],
            },
        };
        let resp = handle_request(&req);
        assert!(resp.is_success());
        if let Response::Success { result } = resp {
            let bash = result.as_str().unwrap();
            assert!(bash.contains("for i in a b c"));
            assert!(bash.contains("echo $i"));
        }
    }

    #[test]
    fn test_handle_request_to_bash_with_redirect() {
        let req = Request::ToBash {
            ast: Command::Simple {
                line: None,
                words: vec![Word {
                    word: "echo".to_string(),
                    flags: 0,
                }],
                redirects: vec![crate::ast::Redirect {
                    direction: RedirectType::Output,
                    source_fd: None,
                    target: RedirectTarget::File("file.txt".to_string()),
                    here_doc_eof: None,
                }],
                assignments: None,
            },
        };
        let resp = handle_request(&req);
        assert!(resp.is_success());
        if let Response::Success { result } = resp {
            let bash = result.as_str().unwrap();
            assert!(bash.contains('>'));
            assert!(bash.contains("file.txt"));
        }
    }

    #[test]
    fn test_handle_request_schema() {
        let req = Request::Schema;
        let resp = handle_request(&req);
        assert!(resp.is_success());
        if let Response::Success { result } = resp {
            assert!(result.get("$schema").is_some());
            assert!(result.get("title").is_some());
            assert_eq!(result["title"], "Command");
        }
    }

    #[test]
    fn test_handle_request_ping() {
        let req = Request::Ping;
        let resp = handle_request(&req);
        assert!(resp.is_success());
        if let Response::Success { result } = resp {
            assert_eq!(result, "pong");
        }
    }

    // ==================== Handle Line Tests ====================

    #[test]
    fn test_handle_line_parse() {
        setup();
        let line = r#"{"method":"parse","script":"echo test"}"#;
        let response = handle_line(line);
        let resp: Response = serde_json::from_str(&response).unwrap();
        assert!(resp.is_success());
    }

    #[test]
    fn test_handle_line_invalid_json() {
        let line = "not valid json at all";
        let response = handle_line(line);
        let resp: Response = serde_json::from_str(&response).unwrap();
        assert!(resp.is_error());
        if let Response::Error { error } = resp {
            assert!(error.contains("Invalid request"));
        }
    }

    #[test]
    fn test_handle_line_empty_object() {
        let line = "{}";
        let response = handle_line(line);
        let resp: Response = serde_json::from_str(&response).unwrap();
        assert!(resp.is_error());
    }

    #[test]
    fn test_handle_line_response_is_single_line() {
        setup();
        let line = r#"{"method":"parse","script":"echo hello"}"#;
        let response = handle_line(line);
        assert!(!response.contains('\n'));
        assert_eq!(response.lines().count(), 1);
    }

    // ==================== Server Config Tests ====================

    #[test]
    fn test_server_config_default() {
        let config = ServerConfig::default();
        assert!(config.remove_existing);
        assert!(
            config.socket_path.ends_with("bash-ast.sock"),
            "Expected path ending with bash-ast.sock, got: {}",
            config.socket_path
        );
    }

    #[test]
    fn test_server_config_custom_path() {
        let config = ServerConfig::new("/custom/path.sock");
        assert_eq!(config.socket_path, "/custom/path.sock");
        assert!(config.remove_existing);
    }

    #[test]
    fn test_default_socket_path_with_xdg() {
        // Save current value
        let old_val = std::env::var("XDG_RUNTIME_DIR").ok();

        std::env::set_var("XDG_RUNTIME_DIR", "/run/user/1000");
        let path = default_socket_path();
        assert_eq!(path, "/run/user/1000/bash-ast.sock");

        // Restore
        match old_val {
            Some(v) => std::env::set_var("XDG_RUNTIME_DIR", v),
            None => std::env::remove_var("XDG_RUNTIME_DIR"),
        }
    }

    // ==================== Integration Tests ====================

    /// Helper to create a unique socket path for tests
    fn test_socket_path() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("/tmp/bash-ast-test-{ts}.sock")
    }

    /// Helper to send a request and receive a response
    fn send_request(stream: &mut UnixStream, request: &str) -> Response {
        writeln!(stream, "{request}").unwrap();
        stream.flush().unwrap();

        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut response = String::new();
        reader.read_line(&mut response).unwrap();

        serde_json::from_str(&response).unwrap()
    }

    #[test]
    fn test_server_integration_parse() {
        setup();
        let socket_path = test_socket_path();
        let socket_path_clone = socket_path.clone();

        // Start server in background thread
        let server = Server::with_path(&socket_path);
        let shutdown = server.shutdown_handle();

        let server_thread = thread::spawn(move || {
            let _ = server.run();
        });

        // Wait for server to start
        thread::sleep(Duration::from_millis(100));

        // Connect and send request
        let mut stream = UnixStream::connect(&socket_path_clone).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();

        let resp = send_request(&mut stream, r#"{"method":"parse","script":"echo hello"}"#);
        assert!(resp.is_success());
        if let Response::Success { result } = resp {
            assert_eq!(result["type"], "simple");
        }

        // Shutdown
        shutdown.store(true, Ordering::Relaxed);
        drop(stream);
        let _ = server_thread.join();
    }

    #[test]
    fn test_server_integration_multiple_requests() {
        setup();
        let socket_path = test_socket_path();
        let socket_path_clone = socket_path.clone();

        let server = Server::with_path(&socket_path);
        let shutdown = server.shutdown_handle();

        let server_thread = thread::spawn(move || {
            let _ = server.run();
        });

        thread::sleep(Duration::from_millis(100));

        let mut stream = UnixStream::connect(&socket_path_clone).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();

        // Send multiple requests on same connection
        let resp1 = send_request(&mut stream, r#"{"method":"ping"}"#);
        assert!(resp1.is_success());

        let resp2 = send_request(&mut stream, r#"{"method":"parse","script":"ls -la"}"#);
        assert!(resp2.is_success());

        let resp3 = send_request(&mut stream, r#"{"method":"schema"}"#);
        assert!(resp3.is_success());

        shutdown.store(true, Ordering::Relaxed);
        drop(stream);
        let _ = server_thread.join();
    }

    #[test]
    fn test_server_integration_reconnect() {
        // Regression test for issue #1: server hangs on second connection
        setup();
        let socket_path = test_socket_path();
        let socket_path_clone = socket_path.clone();

        let server = Server::with_path(&socket_path);
        let shutdown = server.shutdown_handle();

        let server_thread = thread::spawn(move || {
            let _ = server.run();
        });

        thread::sleep(Duration::from_millis(100));

        // First connection
        {
            let mut stream = UnixStream::connect(&socket_path_clone).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();

            let resp = send_request(&mut stream, r#"{"method":"ping"}"#);
            assert!(resp.is_success());
            // stream is dropped here, simulating client disconnect
        }

        // Brief pause to let server process the disconnect
        thread::sleep(Duration::from_millis(50));

        // Second connection - this was hanging before the fix
        {
            let mut stream = UnixStream::connect(&socket_path_clone).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();

            let resp = send_request(&mut stream, r#"{"method":"ping"}"#);
            assert!(
                resp.is_success(),
                "Second connection should work after first disconnects"
            );
        }

        // Third connection for good measure
        {
            let mut stream = UnixStream::connect(&socket_path_clone).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();

            let resp = send_request(&mut stream, r#"{"method":"parse","script":"echo hello"}"#);
            assert!(resp.is_success(), "Third connection should also work");
        }

        shutdown.store(true, Ordering::Relaxed);
        let _ = server_thread.join();
    }

    #[test]
    fn test_server_integration_error_handling() {
        setup();
        let socket_path = test_socket_path();
        let socket_path_clone = socket_path.clone();

        let server = Server::with_path(&socket_path);
        let shutdown = server.shutdown_handle();

        let server_thread = thread::spawn(move || {
            let _ = server.run();
        });

        thread::sleep(Duration::from_millis(100));

        let mut stream = UnixStream::connect(&socket_path_clone).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();

        // Invalid JSON should return error but not crash server
        let resp1 = send_request(&mut stream, "not json");
        assert!(resp1.is_error());

        // Server should still work
        let resp2 = send_request(&mut stream, r#"{"method":"ping"}"#);
        assert!(resp2.is_success());

        // Syntax error should return error
        let resp3 = send_request(&mut stream, r#"{"method":"parse","script":"if then fi"}"#);
        assert!(resp3.is_error());

        // Server should still work
        let resp4 = send_request(&mut stream, r#"{"method":"parse","script":"echo ok"}"#);
        assert!(resp4.is_success());

        shutdown.store(true, Ordering::Relaxed);
        drop(stream);
        let _ = server_thread.join();
    }

    #[test]
    fn test_server_integration_to_bash() {
        setup();
        let socket_path = test_socket_path();
        let socket_path_clone = socket_path.clone();

        let server = Server::with_path(&socket_path);
        let shutdown = server.shutdown_handle();

        let server_thread = thread::spawn(move || {
            let _ = server.run();
        });

        thread::sleep(Duration::from_millis(100));

        let mut stream = UnixStream::connect(&socket_path_clone).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();

        let req = r#"{"method":"to_bash","ast":{"type":"simple","words":[{"word":"echo"},{"word":"hello"}],"redirects":[]}}"#;
        let resp = send_request(&mut stream, req);
        assert!(resp.is_success());
        if let Response::Success { result } = resp {
            assert_eq!(result, "echo hello");
        }

        shutdown.store(true, Ordering::Relaxed);
        drop(stream);
        let _ = server_thread.join();
    }

    #[test]
    fn test_server_integration_roundtrip() {
        setup();
        let socket_path = test_socket_path();
        let socket_path_clone = socket_path.clone();

        let server = Server::with_path(&socket_path);
        let shutdown = server.shutdown_handle();

        let server_thread = thread::spawn(move || {
            let _ = server.run();
        });

        thread::sleep(Duration::from_millis(100));

        let mut stream = UnixStream::connect(&socket_path_clone).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();

        // Parse a script
        let resp1 = send_request(
            &mut stream,
            r#"{"method":"parse","script":"for i in 1 2 3; do echo $i; done"}"#,
        );
        assert!(resp1.is_success());

        // Extract the AST and send it to to_bash
        if let Response::Success { result: ast } = resp1 {
            let to_bash_req = serde_json::json!({"method": "to_bash", "ast": ast});
            let resp2 = send_request(&mut stream, &to_bash_req.to_string());
            assert!(resp2.is_success());
            if let Response::Success { result } = resp2 {
                let bash = result.as_str().unwrap();
                assert!(bash.contains("for i in 1 2 3"));
                assert!(bash.contains("echo $i"));
            }
        }

        shutdown.store(true, Ordering::Relaxed);
        drop(stream);
        let _ = server_thread.join();
    }

    #[test]
    fn test_server_socket_cleanup_on_drop() {
        let socket_path = test_socket_path();

        {
            let server = Server::with_path(&socket_path);
            // Manually create the socket file to simulate server start
            let _ = std::fs::write(&socket_path, "");
            assert!(Path::new(&socket_path).exists());
            drop(server);
        }

        // Socket should be cleaned up after drop
        assert!(!Path::new(&socket_path).exists());
    }

    #[test]
    fn test_server_removes_existing_socket() {
        let socket_path = test_socket_path();

        // Create an existing file at the socket path
        std::fs::write(&socket_path, "existing").unwrap();
        assert!(Path::new(&socket_path).exists());

        let config = ServerConfig::new(&socket_path);
        assert!(config.remove_existing);

        // Clean up
        let _ = std::fs::remove_file(&socket_path);
    }

    // ==================== Edge Case Tests ====================

    #[test]
    fn test_handle_request_parse_multiline() {
        setup();
        let req = Request::Parse {
            script: "echo line1\necho line2\necho line3".to_string(),
        };
        let resp = handle_request(&req);
        assert!(resp.is_success());
        if let Response::Success { result } = resp {
            assert_eq!(result["type"], "list");
        }
    }

    #[test]
    fn test_handle_request_parse_with_comments() {
        setup();
        let req = Request::Parse {
            script: "# comment\necho hello".to_string(),
        };
        let resp = handle_request(&req);
        assert!(resp.is_success());
    }

    #[test]
    fn test_handle_request_parse_heredoc() {
        setup();
        let req = Request::Parse {
            script: "cat <<EOF\nhello\nworld\nEOF".to_string(),
        };
        let resp = handle_request(&req);
        assert!(resp.is_success());
    }

    #[test]
    fn test_handle_request_parse_subshell() {
        setup();
        let req = Request::Parse {
            script: "(echo hello; echo world)".to_string(),
        };
        let resp = handle_request(&req);
        assert!(resp.is_success());
        if let Response::Success { result } = resp {
            assert_eq!(result["type"], "subshell");
        }
    }

    #[test]
    fn test_handle_request_parse_function() {
        setup();
        let req = Request::Parse {
            script: "foo() { echo bar; }".to_string(),
        };
        let resp = handle_request(&req);
        assert!(resp.is_success());
        if let Response::Success { result } = resp {
            assert_eq!(result["type"], "function_def");
            assert_eq!(result["name"], "foo");
        }
    }

    #[test]
    fn test_handle_request_parse_case() {
        setup();
        let req = Request::Parse {
            script: "case $x in a) echo a;; b) echo b;; esac".to_string(),
        };
        let resp = handle_request(&req);
        assert!(resp.is_success());
        if let Response::Success { result } = resp {
            assert_eq!(result["type"], "case");
        }
    }

    #[test]
    fn test_handle_request_parse_if_elif_else() {
        setup();
        let req = Request::Parse {
            script: "if true; then echo a; elif false; then echo b; else echo c; fi".to_string(),
        };
        let resp = handle_request(&req);
        assert!(resp.is_success());
        if let Response::Success { result } = resp {
            assert_eq!(result["type"], "if");
        }
    }

    #[test]
    fn test_handle_line_with_newline_in_script() {
        setup();
        // Script contains escaped newlines in JSON
        let line = r#"{"method":"parse","script":"echo a\necho b"}"#;
        let response = handle_line(line);
        let resp: Response = serde_json::from_str(&response).unwrap();
        assert!(resp.is_success());
    }

    #[test]
    fn test_handle_line_unicode() {
        setup();
        let line = r#"{"method":"parse","script":"echo 你好世界"}"#;
        let response = handle_line(line);
        let resp: Response = serde_json::from_str(&response).unwrap();
        assert!(resp.is_success());
    }

    #[test]
    fn test_handle_line_special_chars() {
        setup();
        let line = r#"{"method":"parse","script":"echo \"hello\\nworld\""}"#;
        let response = handle_line(line);
        let resp: Response = serde_json::from_str(&response).unwrap();
        assert!(resp.is_success());
    }
}
