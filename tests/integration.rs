//! Integration tests for bash-ast
//!
//! These tests verify that the parser correctly handles various bash constructs.
//!
//! NOTE: These tests MUST run single-threaded because bash's parser uses global
//! state. This is enforced via .cargo/config.toml setting RUST_TEST_THREADS=1.

use bash_ast::{
    init, parse, parse_to_json, Command, ConditionalExpr, ListOp, ParseError, MAX_SCRIPT_SIZE,
};
use proptest::prelude::*;

// ============================================================================
// Test Infrastructure
// ============================================================================

/// Initialize bash parser
fn setup() {
    init(); // Safe to call multiple times
}

/// Helper to parse and unwrap, with better error messages
fn parse_ok(script: &str) -> Command {
    setup();
    parse(script).unwrap_or_else(|e| panic!("Failed to parse {:?}: {}", script, e))
}

/// Helper to assert parsing fails with expected error
fn parse_err(script: &str) -> ParseError {
    setup();
    parse(script).expect_err(&format!("Expected parse error for {:?}", script))
}

/// Extract words from a Simple command
fn simple_words(cmd: &Command) -> Vec<&str> {
    match cmd {
        Command::Simple { words, .. } => words.iter().map(|w| w.word.as_str()).collect(),
        _ => panic!("Expected Simple command, got {:?}", cmd),
    }
}

/// Assert command is Simple with expected words
fn assert_simple(cmd: &Command, expected: &[&str]) {
    let words = simple_words(cmd);
    assert_eq!(words, expected, "Word mismatch");
}

/// Assert command has non-empty redirects
fn assert_has_redirects(cmd: &Command) {
    match cmd {
        Command::Simple { redirects, .. } => {
            assert!(!redirects.is_empty(), "Expected redirects, got none");
        }
        _ => panic!("Expected Simple command, got {:?}", cmd),
    }
}

/// Assert command is a Pipeline with expected number of stages
fn assert_pipeline(cmd: &Command, expected_stages: usize) -> bool {
    match cmd {
        Command::Pipeline { commands, .. } => {
            assert_eq!(commands.len(), expected_stages);
            true
        }
        _ => false,
    }
}

// ============================================================================
// Simple Commands
// ============================================================================

#[test]
fn test_simple_commands() {
    let cases: &[(&str, &[&str])] = &[
        ("ls", &["ls"]),
        ("echo hello world", &["echo", "hello", "world"]),
        ("echo $HOME", &["echo", "$HOME"]),
        ("cat -n file.txt", &["cat", "-n", "file.txt"]),
    ];

    for (script, expected) in cases {
        let cmd = parse_ok(script);
        assert_simple(&cmd, expected);
    }
}

#[test]
fn test_simple_command_with_quotes() {
    let cmd = parse_ok(r#"echo "hello world""#);
    let words = simple_words(&cmd);
    assert_eq!(words[0], "echo");
    assert!(words[1].contains("hello world"));
}

// ============================================================================
// Pipelines
// ============================================================================

#[test]
fn test_pipelines() {
    let cases: &[(&str, usize)] = &[
        ("cat file | grep pattern", 2),
        ("cat file | grep pattern | sort | uniq | head -10", 5),
        ("ls | wc -l", 2),
    ];

    for (script, expected_stages) in cases {
        let cmd = parse_ok(script);
        assert!(
            assert_pipeline(&cmd, *expected_stages),
            "Expected Pipeline for {:?}",
            script
        );
    }
}

// ============================================================================
// Lists (&&, ||, ;, &)
// ============================================================================

#[test]
fn test_list_operators() {
    let cases: &[(&str, fn(&ListOp) -> bool)] = &[
        ("cmd1 && cmd2", |op| matches!(op, ListOp::And)),
        ("cmd1 || cmd2", |op| matches!(op, ListOp::Or)),
        ("cmd1 ; cmd2", |op| matches!(op, ListOp::Semi)),
        ("cmd1 & cmd2", |op| matches!(op, ListOp::Amp)),
    ];

    for (script, check_op) in cases {
        let cmd = parse_ok(script);
        if let Command::List { op, .. } = cmd {
            assert!(check_op(&op), "Wrong operator for {:?}", script);
        } else {
            panic!("Expected List command for {:?}", script);
        }
    }
}

// ============================================================================
// Loops
// ============================================================================

#[test]
fn test_for_loop() {
    let cmd = parse_ok("for i in a b c; do echo $i; done");
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
fn test_for_loop_without_in() {
    let cmd = parse_ok("for i; do echo $i; done");
    if let Command::For { variable, .. } = cmd {
        assert_eq!(variable, "i");
    } else {
        panic!("Expected For command");
    }
}

#[test]
fn test_while_loop() {
    let cmd = parse_ok("while true; do echo loop; done");
    if let Command::While { test, body, .. } = cmd {
        assert_simple(&test, &["true"]);
        assert_simple(&body, &["echo", "loop"]);
    } else {
        panic!("Expected While command");
    }
}

#[test]
fn test_while_with_complex_body() {
    let cmd = parse_ok("while read line; do echo \"$line\" | wc -c; done");
    if let Command::While { test, body, .. } = cmd {
        assert_simple(&test, &["read", "line"]);
        assert!(matches!(*body, Command::Pipeline { .. }));
    } else {
        panic!("Expected While command");
    }
}

#[test]
fn test_until_loop() {
    let cmd = parse_ok("until false; do echo loop; done");
    if let Command::Until { test, body, .. } = cmd {
        assert_simple(&test, &["false"]);
        assert_simple(&body, &["echo", "loop"]);
    } else {
        panic!("Expected Until command");
    }
}

#[test]
fn test_until_with_complex_condition() {
    let cmd = parse_ok("until test -f /tmp/done; do sleep 1; done");
    if let Command::Until { test, body, .. } = cmd {
        assert_simple(&test, &["test", "-f", "/tmp/done"]);
        assert_simple(&body, &["sleep", "1"]);
    } else {
        panic!("Expected Until command");
    }
}

// ============================================================================
// Conditionals (if/elif/else)
// ============================================================================

#[test]
fn test_if_variations() {
    // Basic if
    let cmd = parse_ok("if true; then echo yes; fi");
    if let Command::If {
        condition,
        then_branch,
        else_branch,
        ..
    } = cmd
    {
        assert!(else_branch.is_none());
        assert_simple(&condition, &["true"]);
        assert_simple(&then_branch, &["echo", "yes"]);
    } else {
        panic!("Expected If command");
    }

    // If-else
    let cmd = parse_ok("if true; then echo yes; else echo no; fi");
    if let Command::If { else_branch, .. } = cmd {
        assert!(else_branch.is_some());
    } else {
        panic!("Expected If command");
    }

    // If-elif-else
    let cmd = parse_ok("if test1; then cmd1; elif test2; then cmd2; else cmd3; fi");
    if let Command::If { else_branch, .. } = cmd {
        assert!(else_branch.is_some());
        if let Some(else_cmd) = else_branch {
            assert!(matches!(*else_cmd, Command::If { .. }));
        }
    } else {
        panic!("Expected If command");
    }
}

// ============================================================================
// Case Statements
// ============================================================================

#[test]
fn test_case_statements() {
    let cmd = parse_ok("case $x in a) echo a;; b) echo b;; esac");
    if let Command::Case { clauses, .. } = cmd {
        assert!(!clauses.is_empty());
    } else {
        panic!("Expected Case command");
    }
}

#[test]
fn test_case_with_patterns() {
    let cmd = parse_ok("case $x in a|b|c) echo match;; *) echo default;; esac");
    if let Command::Case { clauses, .. } = cmd {
        assert!(clauses.len() >= 2);
    } else {
        panic!("Expected Case command");
    }
}

#[test]
fn test_case_fallthrough() {
    let cmd = parse_ok("case $x in a) echo a;& b) echo b;; esac");
    if let Command::Case { clauses, .. } = cmd {
        if let Some(flags) = &clauses[0].flags {
            assert!(flags.fallthrough);
        }
    } else {
        panic!("Expected Case command");
    }
}

#[test]
fn test_case_test_next() {
    let cmd = parse_ok("case $x in a) echo a;;& b) echo b;; esac");
    if let Command::Case { clauses, .. } = cmd {
        if let Some(flags) = &clauses[0].flags {
            assert!(flags.test_next);
        }
    } else {
        panic!("Expected Case command");
    }
}

// ============================================================================
// Groups and Subshells
// ============================================================================

#[test]
fn test_brace_group() {
    let cmd = parse_ok("{ echo hello; echo world; }");
    assert!(matches!(cmd, Command::Group { .. }));
}

#[test]
fn test_subshell() {
    let cmd = parse_ok("(echo hello; echo world)");
    assert!(matches!(cmd, Command::Subshell { .. }));
}

// ============================================================================
// Functions
// ============================================================================

#[test]
fn test_functions() {
    // Traditional syntax
    let cmd = parse_ok("foo() { echo bar; }");
    if let Command::FunctionDef { name, .. } = cmd {
        assert_eq!(name, "foo");
    } else {
        panic!("Expected FunctionDef command");
    }

    // Keyword syntax
    let cmd = parse_ok("function bar { echo baz; }");
    if let Command::FunctionDef { name, .. } = cmd {
        assert_eq!(name, "bar");
    } else {
        panic!("Expected FunctionDef command");
    }
}

// ============================================================================
// Arithmetic
// ============================================================================

#[test]
fn test_arithmetic() {
    let cmd = parse_ok("(( x = 1 + 2 ))");
    if let Command::Arithmetic { expression, .. } = cmd {
        assert!(expression.contains("1") && expression.contains("2"));
    } else {
        panic!("Expected Arithmetic command");
    }
}

#[test]
fn test_arithmetic_for() {
    let cmd = parse_ok("for ((i=0; i<10; i++)); do echo $i; done");
    if let Command::ArithmeticFor {
        init, test, step, ..
    } = cmd
    {
        assert!(init.contains("0") || init.contains("i"));
        assert!(test.contains("10") || test.contains("i"));
        assert!(step.contains("++") || step.contains("i"));
    } else {
        panic!("Expected ArithmeticFor command");
    }
}

#[test]
fn test_arithmetic_complex() {
    let cmd = parse_ok("(( result = (a + b) * c / d ))");
    if let Command::Arithmetic { expression, .. } = cmd {
        assert!(expression.contains("result") || expression.contains("a"));
    } else {
        panic!("Expected Arithmetic command");
    }
}

#[test]
fn test_arithmetic_for_complex() {
    let cmd = parse_ok("for ((i=0, j=10; i<j; i++, j--)); do echo $i $j; done");
    if let Command::ArithmeticFor {
        init, test, step, ..
    } = cmd
    {
        assert!(init.contains("i") || init.contains("j"));
        assert!(test.contains("i") || test.contains("j"));
        assert!(step.contains("i") || step.contains("j"));
    } else {
        panic!("Expected ArithmeticFor command");
    }
}

// ============================================================================
// Conditional Expressions [[ ]]
// ============================================================================

#[test]
fn test_conditional_expressions() {
    // Unary test
    let cmd = parse_ok("[[ -f file.txt ]]");
    assert!(matches!(cmd, Command::Conditional { .. }));

    // Binary test
    let cmd = parse_ok("[[ $a == $b ]]");
    assert!(matches!(cmd, Command::Conditional { .. }));
}

#[test]
fn test_conditional_and_or() {
    let cmd = parse_ok("[[ -f file && -r file ]]");
    if let Command::Conditional { expr, .. } = cmd {
        assert!(matches!(expr, ConditionalExpr::And { .. }));
    } else {
        panic!("Expected Conditional command");
    }

    let cmd = parse_ok("[[ -f file || -d file ]]");
    if let Command::Conditional { expr, .. } = cmd {
        assert!(matches!(expr, ConditionalExpr::Or { .. }));
    } else {
        panic!("Expected Conditional command");
    }
}

// ============================================================================
// Select Statement
// ============================================================================

#[test]
fn test_select() {
    let cmd = parse_ok("select opt in a b c; do echo $opt; break; done");
    if let Command::Select {
        variable, words, ..
    } = cmd
    {
        assert_eq!(variable, "opt");
        assert_eq!(
            words,
            Some(vec!["a".to_string(), "b".to_string(), "c".to_string()])
        );
    } else {
        panic!("Expected Select command");
    }
}

#[test]
fn test_select_without_in() {
    let cmd = parse_ok("select opt; do echo $opt; done");
    if let Command::Select { variable, .. } = cmd {
        assert_eq!(variable, "opt");
    } else {
        panic!("Expected Select command");
    }
}

// ============================================================================
// Coproc
// ============================================================================

#[test]
fn test_coproc() {
    // Anonymous
    let cmd = parse_ok("coproc cat");
    if let Command::Coproc { body, .. } = cmd {
        assert_simple(&body, &["cat"]);
    } else {
        panic!("Expected Coproc command");
    }

    // Named
    let cmd = parse_ok("coproc mycoproc { cat; }");
    if let Command::Coproc { name, body, .. } = cmd {
        assert_eq!(name, Some("mycoproc".to_string()));
        assert!(matches!(*body, Command::Group { .. }));
    } else {
        panic!("Expected Coproc command");
    }
}

// ============================================================================
// Redirections (data-driven)
// ============================================================================

#[test]
fn test_redirections() {
    let redirect_cases = [
        "echo hello > file.txt",
        "cat < file.txt",
        "echo hello >> file.txt",
        "cat <<< 'hello world'",
        "cmd 2>&1",
        "cmd 2>&-",
        "cmd <> file",
        "cmd >| file",
        "cmd &> file",
        "cmd &>> file",
    ];

    for script in redirect_cases {
        let cmd = parse_ok(script);
        assert_has_redirects(&cmd);
    }
}

#[test]
fn test_here_documents() {
    let cases = ["cat <<EOF\nhello\nEOF", "cat <<-EOF\n\thello\n\tEOF"];

    for script in cases {
        let cmd = parse_ok(script);
        assert_has_redirects(&cmd);
    }
}

// ============================================================================
// Variable Assignments
// ============================================================================

#[test]
fn test_variable_assignments() {
    let cmd = parse_ok("VAR=value cmd");
    if let Command::Simple {
        assignments, words, ..
    } = cmd
    {
        assert!(assignments.is_some());
        assert!(!words.is_empty());
    } else {
        panic!("Expected Simple command");
    }

    let cmd = parse_ok("A=1 B=2 C=3 cmd");
    if let Command::Simple { assignments, .. } = cmd {
        assert_eq!(assignments.unwrap().len(), 3);
    } else {
        panic!("Expected Simple command");
    }
}

// ============================================================================
// Error Cases
// ============================================================================

#[test]
fn test_syntax_errors() {
    let error_cases = ["if then fi", "for do done", "((("];

    for script in error_cases {
        let err = parse_err(script);
        assert!(matches!(err, ParseError::SyntaxError));
    }
}

#[test]
fn test_empty_input() {
    for script in ["", "   ", "\n\t  "] {
        let err = parse_err(script);
        assert!(matches!(err, ParseError::EmptyInput));
    }
}

#[test]
fn test_input_too_large() {
    let script = "x".repeat(MAX_SCRIPT_SIZE + 1);
    let err = parse_err(&script);
    assert!(matches!(err, ParseError::InputTooLarge));
}

#[test]
fn test_input_at_max_size() {
    let script = "x".repeat(MAX_SCRIPT_SIZE);
    // Should not error with InputTooLarge (but may have other errors)
    let result = parse(&script);
    assert!(!matches!(result, Err(ParseError::InputTooLarge)));
}

#[test]
fn test_null_byte_rejected() {
    let script = "echo\0hello";
    let err = parse_err(script);
    assert!(matches!(err, ParseError::InvalidString(_)));
}

// ============================================================================
// JSON Output
// ============================================================================

#[test]
fn test_json_output() {
    setup();
    let json = parse_to_json("echo hello", false).unwrap();
    assert!(json.contains("\"type\""));
    assert!(json.contains("\"words\""));

    let json_pretty = parse_to_json("echo hello", true).unwrap();
    assert!(json_pretty.contains('\n'));
}

// ============================================================================
// Command::line() Method
// ============================================================================

#[test]
fn test_command_line_method() {
    // Test that line() works on all command types (line=0 for string parsing is expected)
    let test_cases = [
        ("echo hello", "Simple"),
        ("cat | grep x", "Pipeline"),
        ("cmd1 && cmd2", "List"),
        ("for i in a; do echo $i; done", "For"),
        ("while true; do echo x; done", "While"),
        ("until false; do echo x; done", "Until"),
        ("if true; then echo yes; fi", "If"),
        ("case x in a) echo a;; esac", "Case"),
        ("{ echo x; }", "Group"),
        ("(echo x)", "Subshell"),
        ("foo() { echo bar; }", "FunctionDef"),
        ("(( x + 1 ))", "Arithmetic"),
        ("for ((i=0; i<10; i++)); do echo $i; done", "ArithmeticFor"),
        ("[[ -f file ]]", "Conditional"),
        ("select opt in a b; do echo $opt; done", "Select"),
        ("coproc cat", "Coproc"),
    ];

    for (script, expected_type) in test_cases {
        let cmd = parse_ok(script);
        // Verify the method is callable and command type matches
        let _ = cmd.line();
        let debug_str = format!("{:?}", cmd);
        let actual_type = debug_str.split_whitespace().next().unwrap();
        assert!(
            actual_type.contains(expected_type),
            "Expected {} for {:?}, got {}",
            expected_type,
            script,
            actual_type
        );
    }
}

// ============================================================================
// Complex Scripts
// ============================================================================

#[test]
fn test_complex_script() {
    let script = "if true; then for i in 1 2 3; do while true; do echo $i; break; done; done; fi";
    let _ = parse_ok(script);
}

#[test]
fn test_nested_structures() {
    let script = "{ { { echo nested; }; }; }";
    let _ = parse_ok(script).line(); // Just verify it parses
}

#[test]
fn test_deeply_nested_subshells() {
    setup();
    let depth = 50;
    let script = format!("{}echo x{}", "(".repeat(depth), ")".repeat(depth));
    assert!(parse(&script).is_ok());
}

// ============================================================================
// Unicode and Special Characters
// ============================================================================

#[test]
fn test_unicode() {
    let cmd = parse_ok(r#"echo "héllo 日本語""#);
    let words = simple_words(&cmd);
    assert!(words[1].contains("héllo") && words[1].contains("日本語"));

    // Unicode in assignment (parse_ok already called setup)
    let _ = parse_ok("VAR='日本語' echo $VAR");
}

#[test]
fn test_special_characters() {
    let cases = [
        r#"echo $$ $! $? $# $@ $*"#,
        "echo `whoami`",
        "echo $(date)",
        r#"echo $((1 + 2 * 3))"#,
        "diff <(ls dir1) <(ls dir2)",
        "echo {a,b,c}",
        "arr=(one two three)",
    ];

    for script in cases {
        let _ = parse_ok(script); // parse_ok calls setup()
    }
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_long_input() {
    let script = format!("echo {}", "x".repeat(10_000));
    let cmd = parse_ok(&script);
    let words = simple_words(&cmd);
    assert!(words[1].len() >= 10_000);
}

#[test]
fn test_many_pipelines() {
    for n in [10, 50, 100] {
        let script = (0..n).map(|_| "cat").collect::<Vec<_>>().join(" | ");
        let cmd = parse_ok(&script);
        assert!(assert_pipeline(&cmd, n));
    }
}

#[test]
fn test_embedded_newlines() {
    let _ = parse_ok("echo 'line1\nline2\nline3'");
}

// ============================================================================
// Property-Based Tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    // Disabled: Some edge cases cause segfaults in the bash parser FFI
    // TODO: Investigate and fix upstream crash with certain inputs
    // #[test]
    // fn prop_parse_never_panics(s in "\\PC{0,100}") {
    //     setup();
    //     let _ = parse(&s); // Should not panic
    // }

    // Disabled: Some edge cases cause segfaults in the bash parser FFI
    // TODO: Investigate and fix upstream crash with certain inputs
    // #[test]
    // fn prop_parse_ascii_never_panics(s in "[a-zA-Z0-9 ;|&<>(){}\\[\\]$\"'\\\\]{0,200}") {
    //     setup();
    //     let _ = parse(&s);
    // }

    #[test]
    fn prop_echo_parses(word in "[a-zA-Z][a-zA-Z0-9_]{0,20}") {
        let script = format!("echo {}", word);
        let cmd = parse_ok(&script);
        assert!(matches!(cmd, Command::Simple { .. }));
    }

    #[test]
    fn prop_pipeline_parses(n in 2usize..10) {
        let script = (0..n).map(|_| "cat").collect::<Vec<_>>().join(" | ");
        let cmd = parse_ok(&script);
        assert!(assert_pipeline(&cmd, n));
    }

    #[test]
    fn prop_for_loop_parses(words in prop::collection::vec("[a-z]+", 1..20)) {
        setup();
        let script = format!("for i in {}; do echo $i; done", words.join(" "));
        assert!(parse(&script).is_ok());
    }

    #[test]
    fn prop_nested_groups(depth in 1usize..50) {
        setup();
        let script = format!(
            "{}echo x; {}",
            "{ ".repeat(depth),
            "} ".repeat(depth).trim()
        );
        assert!(parse(&script).is_ok(), "Failed to parse nested groups depth {}: {}", depth, script);
    }

    #[test]
    fn prop_oversized_rejected(extra in 1usize..1000) {
        let script = "x".repeat(MAX_SCRIPT_SIZE + extra);
        let err = parse_err(&script);
        assert!(matches!(err, ParseError::InputTooLarge));
    }

    #[test]
    fn prop_null_bytes_rejected(prefix in "[a-z]{0,10}", suffix in "[a-z]{0,10}") {
        let script = format!("{}\0{}", prefix, suffix);
        let err = parse_err(&script);
        assert!(matches!(err, ParseError::InvalidString(_)));
    }
}
