//! Tests for redirects on compound commands and negation
//!
//! These test that redirects attached to compound commands (while, for, if, etc.)
//! are properly captured and round-tripped, and that negation is handled correctly.

use bash_ast::{init, parse, to_bash, Command, Redirect};

fn setup() {
    init();
}

fn parse_ok(script: &str) -> Command {
    setup();
    parse(script).unwrap_or_else(|e| panic!("Failed to parse {:?}: {}", script, e))
}

/// Helper to check if a command has redirects
fn get_redirects(cmd: &Command) -> Option<&Vec<Redirect>> {
    match cmd {
        Command::While { redirects, .. } => Some(redirects),
        Command::Until { redirects, .. } => Some(redirects),
        Command::For { redirects, .. } => Some(redirects),
        Command::If { redirects, .. } => Some(redirects),
        Command::Case { redirects, .. } => Some(redirects),
        Command::Select { redirects, .. } => Some(redirects),
        Command::Group { redirects, .. } => Some(redirects),
        Command::Subshell { redirects, .. } => Some(redirects),
        Command::Simple { redirects, .. } => Some(redirects),
        _ => None,
    }
}

#[test]
fn test_while_with_input_redirect() {
    let cmd = parse_ok("while read line; do echo $line; done < input.txt");
    let redirects = get_redirects(&cmd).expect("While should have redirects field");
    assert_eq!(
        redirects.len(),
        1,
        "Expected 1 redirect, got {:?}",
        redirects
    );
}

#[test]
fn test_while_with_output_redirect() {
    let cmd = parse_ok("while true; do echo hello; done > output.txt");
    let redirects = get_redirects(&cmd).expect("While should have redirects field");
    assert_eq!(redirects.len(), 1, "Expected 1 redirect");
}

#[test]
fn test_for_with_redirect() {
    let cmd = parse_ok("for i in a b c; do echo $i; done > output.txt");
    let redirects = get_redirects(&cmd).expect("For should have redirects field");
    assert_eq!(redirects.len(), 1, "Expected 1 redirect");
}

#[test]
fn test_until_with_redirect() {
    let cmd = parse_ok("until false; do echo waiting; done 2>&1");
    let redirects = get_redirects(&cmd).expect("Until should have redirects field");
    assert_eq!(redirects.len(), 1, "Expected 1 redirect");
}

#[test]
fn test_if_with_redirect() {
    let cmd = parse_ok("if true; then echo yes; fi > output.txt");
    let redirects = get_redirects(&cmd).expect("If should have redirects field");
    assert_eq!(redirects.len(), 1, "Expected 1 redirect");
}

#[test]
fn test_case_with_redirect() {
    let cmd = parse_ok("case $x in a) echo a;; esac > output.txt");
    let redirects = get_redirects(&cmd).expect("Case should have redirects field");
    assert_eq!(redirects.len(), 1, "Expected 1 redirect");
}

#[test]
fn test_group_with_redirect() {
    let cmd = parse_ok("{ echo hello; } > file.txt");
    let redirects = get_redirects(&cmd).expect("Group should have redirects field");
    assert_eq!(redirects.len(), 1, "Expected 1 redirect");
}

#[test]
fn test_subshell_with_redirect() {
    let cmd = parse_ok("(echo hello) > file.txt");
    let redirects = get_redirects(&cmd).expect("Subshell should have redirects field");
    assert_eq!(redirects.len(), 1, "Expected 1 redirect");
}

#[test]
fn test_compound_with_multiple_redirects() {
    let cmd = parse_ok("while read line; do echo $line; done < input.txt > output.txt 2>&1");
    let redirects = get_redirects(&cmd).expect("While should have redirects field");
    assert_eq!(redirects.len(), 3, "Expected 3 redirects");
}

// Round-trip tests
#[test]
fn test_while_redirect_roundtrip() {
    setup();
    let original = "while read line; do echo $line; done < input.txt";
    let ast = parse(original).unwrap();
    let regenerated = to_bash(&ast);
    assert!(
        regenerated.contains("< input.txt") || regenerated.contains("<input.txt"),
        "Regenerated script should contain redirect: {}",
        regenerated
    );
}

#[test]
fn test_for_redirect_roundtrip() {
    setup();
    let original = "for i in a b c; do echo $i; done > output.txt";
    let ast = parse(original).unwrap();
    let regenerated = to_bash(&ast);
    assert!(
        regenerated.contains("> output.txt") || regenerated.contains(">output.txt"),
        "Regenerated script should contain redirect: {}",
        regenerated
    );
}

#[test]
fn test_group_redirect_roundtrip() {
    setup();
    let original = "{ echo hello; } > file.txt 2>&1";
    let ast = parse(original).unwrap();
    let regenerated = to_bash(&ast);
    assert!(
        regenerated.contains("> file.txt") || regenerated.contains(">file.txt"),
        "Regenerated script should contain redirect: {}",
        regenerated
    );
}

// ============================================================================
// Negation tests
// ============================================================================

#[test]
fn test_negated_simple_command() {
    let cmd = parse_ok("! cmd");
    if let Command::Pipeline {
        negated, commands, ..
    } = cmd
    {
        assert!(negated, "Pipeline should be negated");
        assert_eq!(commands.len(), 1, "Should have one command");
    } else {
        panic!("Expected Pipeline command for negated simple command");
    }
}

#[test]
fn test_negated_simple_in_list() {
    let cmd = parse_ok("! cmd1 && cmd2");
    if let Command::List { left, .. } = cmd {
        if let Command::Pipeline { negated, .. } = left.as_ref() {
            assert!(negated, "Left side should be negated");
        } else {
            panic!("Expected Pipeline for left side of list");
        }
    } else {
        panic!("Expected List command");
    }
}

#[test]
fn test_negated_roundtrip() {
    setup();
    let original = "! grep -q pattern file && echo not found";
    let ast = parse(original).unwrap();
    let regenerated = to_bash(&ast);
    assert!(
        regenerated.starts_with("! "),
        "Regenerated should start with '! ': {}",
        regenerated
    );
}

#[test]
fn test_negated_pipeline_roundtrip() {
    setup();
    let original = "! cmd1 | cmd2 | cmd3";
    let ast = parse(original).unwrap();
    let regenerated = to_bash(&ast);
    assert!(
        regenerated.starts_with("! "),
        "Regenerated should start with '! ': {}",
        regenerated
    );
}
