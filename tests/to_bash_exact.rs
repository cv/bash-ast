//! Exact-string tests for the bash printer.
//!
//! These start from hand-built ASTs so they exercise printer branches that are
//! hard to reach from parse-first tests alone.

mod common;

use bash_ast::{to_bash, Command, ListOp, RedirectType};
use common::{
    arithmetic_for, assert_semantic_roundtrip_ast, case_clause, case_clause_with_flags, case_cmd,
    cond_and, cond_binary, cond_expr, cond_not, cond_or, cond_term, cond_unary, conditional,
    coproc, for_loop, function_def, group, group_with_redirects, heredoc, if_cmd, list,
    negated_pipeline, pipeline, redirect_fd, redirect_file, select_cmd, simple, simple_with,
    subshell, word,
};

fn assert_exact(ast: &Command, expected: &str) {
    assert_eq!(to_bash(ast), expected);
}

fn assert_exact_and_semantic(ast: &Command, expected: &str) {
    assert_exact(ast, expected);
    assert_semantic_roundtrip_ast(ast);
}

#[test]
fn test_regular_assignments_print_before_words() {
    let ast = simple_with(
        vec![word("env"), word("echo")],
        vec![],
        Some(vec!["A=1", "B=2"]),
    );
    assert_exact_and_semantic(&ast, "A=1 B=2 env echo");
}

#[test]
fn test_assignment_builtin_prints_assignments_after_words() {
    let ast = simple_with(
        vec![word("declare"), word("-r")],
        vec![],
        Some(vec!["CONST=42"]),
    );
    assert_exact_and_semantic(&ast, "declare -r CONST=42");
}

#[test]
fn test_file_redirect_matrix_exact_output() {
    let cases = [
        (
            simple_with(
                vec![word("cat")],
                vec![redirect_file(RedirectType::Input, Some(0), "input.txt")],
                None,
            ),
            "cat <input.txt",
        ),
        (
            simple_with(
                vec![word("echo"), word("hello")],
                vec![redirect_file(RedirectType::Output, Some(2), "errs.txt")],
                None,
            ),
            "echo hello 2>errs.txt",
        ),
        (
            simple_with(
                vec![word("echo")],
                vec![redirect_file(RedirectType::Append, Some(1), "log")],
                None,
            ),
            "echo >>log",
        ),
        (
            simple_with(
                vec![word("cat")],
                vec![redirect_file(RedirectType::HereString, Some(0), "$data")],
                None,
            ),
            "cat <<<$data",
        ),
        (
            simple_with(
                vec![word("exec")],
                vec![redirect_file(
                    RedirectType::InputOutput,
                    Some(3),
                    "/tmp/file",
                )],
                None,
            ),
            "exec 3<>/tmp/file",
        ),
        (
            simple_with(
                vec![word("echo")],
                vec![redirect_file(RedirectType::Clobber, Some(1), "out")],
                None,
            ),
            "echo >|out",
        ),
        (
            simple_with(
                vec![word("cmd")],
                vec![redirect_file(RedirectType::ErrAndOut, Some(1), "all.log")],
                None,
            ),
            "cmd &>all.log",
        ),
        (
            simple_with(
                vec![word("cmd")],
                vec![redirect_file(
                    RedirectType::AppendErrAndOut,
                    Some(1),
                    "all.log",
                )],
                None,
            ),
            "cmd &>>all.log",
        ),
    ];

    for (ast, expected) in cases {
        assert_exact_and_semantic(&ast, expected);
    }
}

#[test]
fn test_fd_redirect_matrix_exact_output() {
    let cases = [
        (
            simple_with(
                vec![word("cmd")],
                vec![redirect_fd(RedirectType::DupInput, Some(0), 3)],
                None,
            ),
            "cmd <&3",
        ),
        (
            simple_with(
                vec![word("cmd")],
                vec![redirect_fd(RedirectType::DupOutput, Some(2), 1)],
                None,
            ),
            "cmd 2>&1",
        ),
        (
            simple_with(
                vec![word("exec")],
                vec![redirect_fd(RedirectType::Close, Some(3), -1)],
                None,
            ),
            "exec 3>&-",
        ),
        (
            simple_with(
                vec![word("cmd")],
                vec![redirect_fd(RedirectType::MoveInput, Some(0), 5)],
                None,
            ),
            "cmd <&5-",
        ),
        (
            simple_with(
                vec![word("cmd")],
                vec![redirect_fd(RedirectType::MoveOutput, Some(2), 4)],
                None,
            ),
            "cmd 2>&4-",
        ),
    ];

    for (ast, expected) in cases {
        assert_exact_and_semantic(&ast, expected);
    }
}

#[test]
fn test_redirect_target_starting_with_angle_gets_spacing() {
    let input_proc_sub = simple_with(
        vec![word("cat")],
        vec![redirect_file(RedirectType::Input, Some(0), "<(sort data)")],
        None,
    );
    assert_exact_and_semantic(&input_proc_sub, "cat < <(sort data)");

    let weird_output = simple_with(
        vec![word("echo"), word("hi")],
        vec![redirect_file(RedirectType::Output, Some(1), ">literal")],
        None,
    );
    assert_exact(&weird_output, "echo hi > >literal");
}

#[test]
fn test_heredoc_defaults_delimiter_and_adds_trailing_newline() {
    let ast = simple_with(vec![word("cat")], vec![heredoc("hello", None)], None);
    assert_exact(&ast, "cat <<EOF\nhello\nEOF");
}

#[test]
fn test_negated_pipeline_exact_output() {
    let ast = negated_pipeline(vec![simple(&["grep", "-q", "x"]), simple(&["wc", "-l"])]);
    assert_exact_and_semantic(&ast, "! grep -q x | wc -l");
}

#[test]
fn test_background_list_with_empty_right_side_exact_output() {
    let ast = list(
        ListOp::Amp,
        simple(&["sleep", "1"]),
        simple_with(Vec::new(), Vec::new(), None),
    );
    assert_exact_and_semantic(&ast, "sleep 1 &");
}

#[test]
fn test_and_list_defers_heredoc_content_until_after_rhs() {
    let ast = list(
        ListOp::And,
        simple_with(
            vec![word("cat")],
            vec![heredoc("hello\n", Some("EOF"))],
            None,
        ),
        simple(&["echo", "done"]),
    );
    assert_exact_and_semantic(&ast, "cat <<EOF && echo done\nhello\nEOF");
}

#[test]
fn test_pipeline_defers_heredoc_content_until_after_pipeline() {
    let ast = pipeline(vec![
        simple_with(
            vec![word("cat")],
            vec![heredoc("hello\n", Some("EOF"))],
            None,
        ),
        simple(&["grep", "h"]),
    ]);
    assert_exact_and_semantic(&ast, "cat <<EOF | grep h\nhello\nEOF");
}

#[test]
fn test_group_with_background_body_omits_extra_semicolon() {
    let ast = group(list(
        ListOp::Amp,
        simple(&["sleep", "1"]),
        simple_with(Vec::new(), Vec::new(), None),
    ));
    assert_exact_and_semantic(&ast, "{ sleep 1 & }");
}

#[test]
fn test_for_loop_with_background_body_omits_extra_semicolon() {
    let ast = for_loop(
        "i",
        Some(vec!["a"]),
        list(
            ListOp::Amp,
            simple(&["sleep", "1"]),
            simple_with(Vec::new(), Vec::new(), None),
        ),
    );
    assert_exact_and_semantic(&ast, "for i in a; do sleep 1 & done");
}

#[test]
fn test_if_with_background_then_branch_omits_extra_semicolon() {
    let ast = if_cmd(
        simple(&["true"]),
        list(
            ListOp::Amp,
            simple(&["sleep", "1"]),
            simple_with(Vec::new(), Vec::new(), None),
        ),
        None,
    );
    assert_exact_and_semantic(&ast, "if true; then sleep 1 & fi");
}

#[test]
fn test_case_clause_flags_exact_output() {
    let ast = case_cmd(
        "$x",
        vec![
            case_clause_with_flags(&["a", "b"], Some(simple(&["echo", "ab"])), true, false),
            case_clause_with_flags(&["c"], Some(simple(&["echo", "c"])), false, true),
            case_clause(&["*"], Some(simple(&["echo", "rest"]))),
        ],
    );
    assert_exact_and_semantic(
        &ast,
        "case $x in a|b) echo ab;& c) echo c;;& *) echo rest;; esac",
    );
}

#[test]
fn test_case_clause_with_heredoc_exact_output() {
    let ast = case_cmd(
        "x",
        vec![case_clause(
            &["a"],
            Some(simple_with(
                vec![word("cat")],
                vec![heredoc("hello\n", Some("EOF"))],
                None,
            )),
        )],
    );
    assert_exact_and_semantic(&ast, "case x in a) cat <<EOF;; esac\nhello\nEOF");
}

#[test]
fn test_conditional_expression_exact_output() {
    let ast = conditional(cond_or(
        cond_expr(cond_and(
            cond_unary("-f", "x"),
            cond_not(cond_unary("-d", "y")),
        )),
        cond_term("z"),
    ));
    assert_exact(&ast, "[[ ( -f x && ! -d y ) || z ]]");
}

#[test]
fn test_coproc_default_name_is_elided_for_simple_body() {
    let ast = coproc(Some("COPROC"), simple(&["cat"]));
    assert_exact_and_semantic(&ast, "coproc cat");
}

#[test]
fn test_named_coproc_group_is_preserved() {
    let ast = coproc(Some("WORKER"), group(simple(&["echo", "hi"])));
    assert_exact_and_semantic(&ast, "coproc WORKER { echo hi; }");
}

#[test]
fn test_function_definition_uses_canonical_syntax() {
    let ast = function_def("foo", group(simple(&["echo", "hi"])));
    assert_exact_and_semantic(&ast, "foo() { echo hi; }");
}

#[test]
fn test_subshell_with_redirects_exact_output() {
    let ast = Command::Subshell {
        line: None,
        body: Box::new(simple(&["echo", "hi"])),
        redirects: vec![redirect_file(RedirectType::Output, Some(1), "out")],
    };
    assert_exact_and_semantic(&ast, "( echo hi ) >out");
}

#[test]
fn test_group_with_outer_heredoc_exact_output() {
    let ast = group_with_redirects(
        simple(&["echo", "hi"]),
        vec![heredoc("hello\n", Some("TAG"))],
    );
    assert_exact_and_semantic(&ast, "{ echo hi; } <<TAG\nhello\nTAG");
}

#[test]
fn test_select_with_background_body_exact_output() {
    let ast = select_cmd(
        "choice",
        Some(vec!["a", "b"]),
        list(
            ListOp::Amp,
            simple(&["sleep", "1"]),
            simple_with(Vec::new(), Vec::new(), None),
        ),
    );
    assert_exact_and_semantic(&ast, "select choice in a b; do sleep 1 & done");
}

#[test]
fn test_arithmetic_for_with_background_body_exact_output() {
    let ast = arithmetic_for(
        "i=0",
        "i<1",
        "i++",
        list(
            ListOp::Amp,
            simple(&["sleep", "1"]),
            simple_with(Vec::new(), Vec::new(), None),
        ),
    );
    assert_exact_and_semantic(&ast, "for ((i=0; i<1; i++)); do sleep 1 & done");
}

#[test]
fn test_semi_list_uses_newline_when_commands_have_distinct_lines() {
    let ast = Command::List {
        line: None,
        op: ListOp::Semi,
        left: Box::new(Command::Simple {
            line: Some(1),
            words: vec![word("echo"), word("one")],
            redirects: Vec::new(),
            assignments: None,
        }),
        right: Box::new(Command::Simple {
            line: Some(2),
            words: vec![word("echo"), word("two")],
            redirects: Vec::new(),
            assignments: None,
        }),
    };
    assert_exact_and_semantic(&ast, "echo one\necho two");
}

#[test]
fn test_semi_list_stays_inline_on_same_line() {
    let ast = Command::List {
        line: None,
        op: ListOp::Semi,
        left: Box::new(Command::Simple {
            line: Some(1),
            words: vec![word("echo"), word("one")],
            redirects: Vec::new(),
            assignments: None,
        }),
        right: Box::new(Command::Simple {
            line: Some(1),
            words: vec![word("echo"), word("two")],
            redirects: Vec::new(),
            assignments: None,
        }),
    };
    assert_exact_and_semantic(&ast, "echo one; echo two");
}

#[test]
fn test_nested_conditional_binary_exact_output() {
    let ast = conditional(cond_and(
        cond_binary("$a", "==", "$b"),
        cond_binary("$c", "!=", "$d"),
    ));
    assert_exact_and_semantic(&ast, "[[ $a == $b && $c != $d ]]");
}

#[test]
fn test_subshell_builder_exact_output() {
    let ast = subshell(simple(&["echo", "hi"]));
    assert_exact_and_semantic(&ast, "( echo hi )");
}
