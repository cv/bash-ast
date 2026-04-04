//! Property-based tests for AST-driven printer roundtrips.
//!
//! These intentionally target a canonical subset of the AST model:
//! simple commands plus shallow pipelines, lists, and groups built from them.
//! More complex constructs are covered by the exact and matrix suites.

#![allow(clippy::redundant_clone)]

mod common;

use bash_ast::{Command, ListOp, Redirect, RedirectTarget, RedirectType, Word};
use common::{assert_semantic_roundtrip_ast, group, list, negated_pipeline, pipeline};
use proptest::prelude::*;

fn arb_identifier() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z][a-z0-9_]{0,5}").unwrap()
}

fn arb_word_text() -> impl Strategy<Value = String> {
    prop_oneof![
        arb_identifier(),
        prop::string::string_regex("[0-9]{1,3}").unwrap(),
        prop::string::string_regex("\\$[a-z][a-z0-9_]{0,4}").unwrap(),
        prop::string::string_regex("'[A-Za-z0-9_ !?-]{1,6}'").unwrap(),
    ]
}

fn word_flags(text: &str) -> u32 {
    if text.contains('$') {
        1
    } else if text.starts_with('\'') || text.starts_with('"') {
        2
    } else {
        0
    }
}

fn arb_word() -> impl Strategy<Value = Word> {
    arb_word_text().prop_map(|text| Word {
        flags: word_flags(&text),
        word: text,
    })
}

fn arb_assignment() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("A=1".to_string()),
        Just("B=two".to_string()),
        Just("C='three'".to_string()),
        Just("D=$x".to_string()),
    ]
}

fn arb_redirect() -> impl Strategy<Value = Redirect> {
    let file_target = prop_oneof![
        Just("input.txt".to_string()),
        Just("out.log".to_string()),
        Just("$tmp".to_string()),
    ];
    let heredoc_content = prop_oneof![
        Just("alpha\n".to_string()),
        Just("beta\ngamma\n".to_string()),
    ];

    prop_oneof![
        file_target.clone().prop_map(|target| Redirect {
            direction: RedirectType::Input,
            source_fd: Some(0),
            target: RedirectTarget::File(target),
            here_doc_eof: None,
        }),
        file_target.clone().prop_map(|target| Redirect {
            direction: RedirectType::Output,
            source_fd: Some(1),
            target: RedirectTarget::File(target),
            here_doc_eof: None,
        }),
        file_target.clone().prop_map(|target| Redirect {
            direction: RedirectType::Append,
            source_fd: Some(1),
            target: RedirectTarget::File(target),
            here_doc_eof: None,
        }),
        file_target.clone().prop_map(|target| Redirect {
            direction: RedirectType::ErrAndOut,
            source_fd: Some(1),
            target: RedirectTarget::File(target),
            here_doc_eof: None,
        }),
        Just(Redirect {
            direction: RedirectType::DupOutput,
            source_fd: Some(2),
            target: RedirectTarget::Fd(1),
            here_doc_eof: None,
        }),
        Just(Redirect {
            direction: RedirectType::Close,
            source_fd: Some(3),
            target: RedirectTarget::Fd(-1),
            here_doc_eof: None,
        }),
        heredoc_content.prop_map(|content| Redirect {
            direction: RedirectType::HereDoc,
            source_fd: Some(0),
            target: RedirectTarget::File(content),
            here_doc_eof: Some("EOF".to_string()),
        }),
    ]
}

fn arb_simple() -> impl Strategy<Value = Command> {
    (
        prop::collection::vec(arb_word(), 1..=3),
        prop::collection::vec(arb_assignment(), 0..=1),
        prop::collection::vec(arb_redirect(), 0..=2).prop_filter(
            "at most one heredoc",
            |redirects| {
                redirects
                    .iter()
                    .filter(|redirect| redirect.direction == RedirectType::HereDoc)
                    .count()
                    <= 1
            },
        ),
    )
        .prop_map(|(words, assignments, redirects)| Command::Simple {
            line: None,
            words,
            redirects,
            assignments: (!assignments.is_empty()).then_some(assignments),
        })
}

fn arb_shallow_command() -> impl Strategy<Value = Command> {
    let simple = arb_simple().boxed();
    let simple_pair = (simple.clone(), simple.clone());
    let empty_right = Just(Command::Simple {
        line: None,
        words: Vec::new(),
        redirects: Vec::new(),
        assignments: None,
    });

    prop_oneof![
        simple.clone(),
        (prop::collection::vec(arb_simple(), 2..=3), any::<bool>()).prop_map(
            |(commands, negated)| {
                if negated {
                    negated_pipeline(commands)
                } else {
                    pipeline(commands)
                }
            }
        ),
        (
            prop_oneof![Just(ListOp::And), Just(ListOp::Or), Just(ListOp::Semi)],
            simple.clone(),
            simple.clone(),
        )
            .prop_map(|(op, left, right)| list(op, left, right)),
        (
            Just(ListOp::Amp),
            simple.clone(),
            prop_oneof![simple, empty_right],
        )
            .prop_map(|(op, left, right)| list(op, left, right)),
        simple_pair
            .clone()
            .prop_map(|(left, right)| group(list(ListOp::Semi, left, right))),
        simple_pair.prop_map(|(left, right)| group(list(ListOp::And, left, right))),
        prop::collection::vec(arb_simple(), 2..=3).prop_map(|commands| group(pipeline(commands))),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_simple_ast_to_bash_roundtrips(cmd in arb_simple()) {
        assert_semantic_roundtrip_ast(&cmd);
    }

    #[test]
    fn prop_shallow_structured_ast_to_bash_roundtrips(cmd in arb_shallow_command()) {
        assert_semantic_roundtrip_ast(&cmd);
    }
}
