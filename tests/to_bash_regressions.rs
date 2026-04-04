//! Focused regression tests for `to_bash` semantic printing.
//!
//! These cover bugs where the printer used to emit syntactically invalid bash
//! when heredocs interacted with lists, pipelines, or compound commands.

use bash_ast::{init, parse, to_bash};

fn setup() {
    init();
}

fn assert_semantic_roundtrip(script: &str) {
    setup();

    let original =
        parse(script).unwrap_or_else(|e| panic!("failed to parse original {script:?}: {e}"));
    let regenerated = to_bash(&original);
    let reparsed = parse(&regenerated).unwrap_or_else(|e| {
        panic!(
            "failed to parse regenerated script\noriginal: {script}\nregenerated:\n{regenerated}\nerror: {e}"
        )
    });

    let lhs = normalize(&serde_json::to_value(&original).unwrap());
    let rhs = normalize(&serde_json::to_value(&reparsed).unwrap());
    assert_eq!(
        lhs, rhs,
        "semantic mismatch\noriginal: {script}\nregenerated:\n{regenerated}"
    );
}

fn normalize(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut normalized = serde_json::Map::new();
            for (key, value) in map {
                if key == "line" {
                    continue;
                }
                normalized.insert(key.clone(), normalize(value));
            }
            serde_json::Value::Object(normalized)
        }
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.iter().map(normalize).collect())
        }
        _ => value.clone(),
    }
}

#[test]
fn test_pipeline_with_heredoc_roundtrip() {
    assert_semantic_roundtrip("cat <<EOF | grep h\nhello\nEOF");
}

#[test]
fn test_and_list_with_heredoc_roundtrip() {
    assert_semantic_roundtrip("cat <<EOF && echo hi\nhello\nEOF");
}

#[test]
fn test_background_list_with_heredoc_roundtrip() {
    assert_semantic_roundtrip("cat <<EOF & wait\nhello\nEOF");
}

#[test]
fn test_if_condition_with_heredoc_roundtrip() {
    assert_semantic_roundtrip("if cat <<EOF; then echo yes; fi\nhello\nEOF");
}

#[test]
fn test_while_condition_with_heredoc_roundtrip() {
    assert_semantic_roundtrip("while cat <<EOF; do echo body; done\na\nEOF");
}

#[test]
fn test_group_body_with_heredoc_roundtrip() {
    assert_semantic_roundtrip("{ cat <<EOF; }\nhello\nEOF");
}

#[test]
fn test_case_clause_with_heredoc_roundtrip() {
    assert_semantic_roundtrip("case x in a) cat <<EOF;; esac\nhello\nEOF");
}

#[test]
fn test_for_body_ending_in_background_roundtrip() {
    assert_semantic_roundtrip("for i in a; do sleep 1 & done");
}

#[test]
fn test_group_body_ending_in_background_roundtrip() {
    assert_semantic_roundtrip("{ sleep 1 & }");
}

#[test]
fn test_if_then_branch_ending_in_background_roundtrip() {
    assert_semantic_roundtrip("if true; then sleep 1 & fi");
}

#[test]
fn test_arithmetic_for_body_ending_in_background_roundtrip() {
    assert_semantic_roundtrip("for ((i=0; i<1; i++)); do sleep 1 & done");
}
