//! Matrix-style semantic roundtrip coverage for `to_bash`.
//!
//! These deliberately stress feature interactions instead of isolated syntax.

mod common;

use common::assert_semantic_roundtrip;

#[test]
fn test_binary_operator_heredoc_matrix() {
    let cases = [
        "cat <<L && echo rhs\nleft\nL",
        "echo lhs && cat <<R\nright\nR",
        "cat <<L || echo rhs\nleft\nL",
        "echo lhs || cat <<R\nright\nR",
        "cat <<L | grep l\nleft\nL",
        "grep l <<<left | cat <<R\nright\nR",
        "cat <<L & wait\nleft\nL",
        "echo lhs & cat <<R\nright\nR",
        "cat <<L; echo rhs\nleft\nL",
        "echo lhs; cat <<R\nright\nR",
        "cat <<A && cat <<B\nleft\nA\nright\nB",
        "cat <<A | cat <<B\nleft\nA\nright\nB",
    ];

    for script in cases {
        assert_semantic_roundtrip(script);
    }
}

#[test]
fn test_compound_command_heredoc_matrix() {
    let cases = [
        "if cat <<EOF; then echo yes; fi\nhello\nEOF",
        "if true; then cat <<EOF; else echo no; fi\nhello\nEOF",
        "if false; then echo no; else cat <<EOF; fi\nhello\nEOF",
        "while cat <<EOF; do echo body; done\nhello\nEOF",
        "while true; do cat <<EOF; done\nhello\nEOF",
        "until cat <<EOF; do echo body; done\nhello\nEOF",
        "for i in a; do cat <<EOF; done\nhello\nEOF",
        "for ((i=0; i<1; i++)); do cat <<EOF; done\nhello\nEOF",
        "select opt in a; do cat <<EOF; break; done\nhello\nEOF",
        "{ cat <<EOF; }\nhello\nEOF",
        "{ cat <<EOF; } >out\nhello\nEOF",
        "(cat <<EOF)\nhello\nEOF",
        "foo() { cat <<EOF; }\nhello\nEOF",
        "case x in a) cat <<EOF;; esac\nhello\nEOF",
        "coproc cat <<EOF\nhello\nEOF",
    ];

    for script in cases {
        assert_semantic_roundtrip(script);
    }
}

#[test]
fn test_background_body_matrix() {
    let cases = [
        "for i in a; do sleep 1 & done",
        "for i in a; do echo one; sleep 1 & done",
        "while true; do sleep 1 & done",
        "until false; do sleep 1 & done",
        "if true; then sleep 1 & fi",
        "if true; then echo one; sleep 1 & fi",
        "if false; then echo no; else sleep 1 & fi",
        "select opt in a; do sleep 1 & done",
        "{ sleep 1 & }",
        "{ echo one; sleep 1 & }",
        "for ((i=0; i<1; i++)); do sleep 1 & done",
        "case x in a) sleep 1 &;; esac",
    ];

    for script in cases {
        assert_semantic_roundtrip(script);
    }
}

#[test]
fn test_multiple_heredoc_and_outer_redirect_matrix() {
    let cases = [
        "cat <<A; cat <<B\nleft\nA\nright\nB",
        "cat <<A && cat <<B\nleft\nA\nright\nB",
        "{ cat <<A; cat <<B; }\nleft\nA\nright\nB",
        "{ cat <<A; cat <<B; } >out\nleft\nA\nright\nB",
        "while cat <<A; do cat <<B; done\nleft\nA\nright\nB",
    ];

    for script in cases {
        assert_semantic_roundtrip(script);
    }
}
