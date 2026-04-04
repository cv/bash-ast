#![allow(dead_code)]

use bash_ast::{
    init, parse, to_bash, CaseClause, CaseClauseFlags, Command, ConditionalExpr, ListOp, Redirect,
    RedirectTarget, RedirectType, Word,
};

pub fn setup() {
    init();
}

pub fn parse_ok(script: &str) -> Command {
    setup();
    parse(script).unwrap_or_else(|e| panic!("Failed to parse {script:?}: {e}"))
}

pub fn semantic_roundtrip(script: &str) -> Result<(Command, String, Command), String> {
    setup();

    let original = parse(script).map_err(|e| format!("failed to parse original: {e}"))?;
    let regenerated = to_bash(&original);
    let reparsed = parse(&regenerated)
        .map_err(|e| format!("failed to parse regenerated script:\n{regenerated}\nerror: {e}"))?;

    Ok((original, regenerated, reparsed))
}

pub fn assert_semantic_roundtrip(script: &str) {
    let (original, regenerated, reparsed) = semantic_roundtrip(script).unwrap_or_else(|e| {
        panic!("semantic roundtrip failed\noriginal:\n{script}\n{e}");
    });

    assert_eq!(
        normalize_command(&original),
        normalize_command(&reparsed),
        "semantic mismatch\noriginal:\n{script}\nregenerated:\n{regenerated}"
    );
}

pub fn assert_semantic_roundtrip_ast(ast: &Command) {
    setup();

    let regenerated = to_bash(ast);
    let reparsed = parse(&regenerated).unwrap_or_else(|e| {
        panic!(
            "failed to parse regenerated bash\nregenerated:\n{regenerated}\nerror: {e}\nast: {ast:?}"
        )
    });

    assert_eq!(
        normalize_command(ast),
        normalize_command(&reparsed),
        "semantic mismatch for hand-built AST\nregenerated:\n{regenerated}\nast: {ast:?}"
    );
}

pub fn normalize_command(command: &Command) -> serde_json::Value {
    let mut value = serde_json::to_value(command).expect("command should serialize");
    normalize_for_comparison(&mut value);
    value
}

pub fn normalize_json_for_comparison(json: &str) -> String {
    let mut value: serde_json::Value = serde_json::from_str(json).expect("valid json");
    normalize_for_comparison(&mut value);
    serde_json::to_string(&value).expect("json serialization cannot fail")
}

pub fn normalize_for_comparison(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            map.remove("line");

            if let Some(serde_json::Value::Array(redirects)) = map.get_mut("redirects") {
                redirects.sort_by_key(|redirect| serde_json::to_string(redirect).unwrap());
            }

            for value in map.values_mut() {
                normalize_for_comparison(value);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                normalize_for_comparison(value);
            }
        }
        _ => {}
    }
}

pub fn word(text: &str) -> Word {
    Word {
        word: text.to_string(),
        flags: 0,
    }
}

pub fn word_with_flags(text: &str, flags: u32) -> Word {
    Word {
        word: text.to_string(),
        flags,
    }
}

pub fn simple(words: &[&str]) -> Command {
    Command::Simple {
        line: None,
        words: words.iter().map(|&text| word(text)).collect(),
        redirects: Vec::new(),
        assignments: None,
    }
}

pub fn simple_with(
    words: Vec<Word>,
    redirects: Vec<Redirect>,
    assignments: Option<Vec<&str>>,
) -> Command {
    Command::Simple {
        line: None,
        words,
        redirects,
        assignments: assignments.map(|items| items.into_iter().map(str::to_string).collect()),
    }
}

pub const fn pipeline(commands: Vec<Command>) -> Command {
    Command::Pipeline {
        line: None,
        commands,
        negated: false,
    }
}

pub const fn negated_pipeline(commands: Vec<Command>) -> Command {
    Command::Pipeline {
        line: None,
        commands,
        negated: true,
    }
}

pub fn list(op: ListOp, left: Command, right: Command) -> Command {
    Command::List {
        line: None,
        op,
        left: Box::new(left),
        right: Box::new(right),
    }
}

pub fn group(body: Command) -> Command {
    Command::Group {
        line: None,
        body: Box::new(body),
        redirects: Vec::new(),
    }
}

pub fn group_with_redirects(body: Command, redirects: Vec<Redirect>) -> Command {
    Command::Group {
        line: None,
        body: Box::new(body),
        redirects,
    }
}

pub fn subshell(body: Command) -> Command {
    Command::Subshell {
        line: None,
        body: Box::new(body),
        redirects: Vec::new(),
    }
}

pub fn for_loop(variable: &str, words: Option<Vec<&str>>, body: Command) -> Command {
    Command::For {
        line: None,
        variable: variable.to_string(),
        words: words.map(|items| items.into_iter().map(str::to_string).collect()),
        body: Box::new(body),
        redirects: Vec::new(),
    }
}

pub fn while_loop(test: Command, body: Command) -> Command {
    Command::While {
        line: None,
        test: Box::new(test),
        body: Box::new(body),
        redirects: Vec::new(),
    }
}

pub fn until_loop(test: Command, body: Command) -> Command {
    Command::Until {
        line: None,
        test: Box::new(test),
        body: Box::new(body),
        redirects: Vec::new(),
    }
}

pub fn if_cmd(condition: Command, then_branch: Command, else_branch: Option<Command>) -> Command {
    Command::If {
        line: None,
        condition: Box::new(condition),
        then_branch: Box::new(then_branch),
        else_branch: else_branch.map(Box::new),
        redirects: Vec::new(),
    }
}

pub fn case_clause(patterns: &[&str], action: Option<Command>) -> CaseClause {
    CaseClause {
        patterns: patterns
            .iter()
            .map(std::string::ToString::to_string)
            .collect(),
        action: action.map(Box::new),
        flags: None,
    }
}

pub fn case_clause_with_flags(
    patterns: &[&str],
    action: Option<Command>,
    fallthrough: bool,
    test_next: bool,
) -> CaseClause {
    CaseClause {
        patterns: patterns
            .iter()
            .map(std::string::ToString::to_string)
            .collect(),
        action: action.map(Box::new),
        flags: Some(CaseClauseFlags {
            fallthrough,
            test_next,
        }),
    }
}

pub fn case_cmd(word: &str, clauses: Vec<CaseClause>) -> Command {
    Command::Case {
        line: None,
        word: word.to_string(),
        clauses,
        redirects: Vec::new(),
    }
}

pub fn select_cmd(variable: &str, words: Option<Vec<&str>>, body: Command) -> Command {
    Command::Select {
        line: None,
        variable: variable.to_string(),
        words: words.map(|items| items.into_iter().map(str::to_string).collect()),
        body: Box::new(body),
        redirects: Vec::new(),
    }
}

pub fn function_def(name: &str, body: Command) -> Command {
    Command::FunctionDef {
        line: None,
        name: name.to_string(),
        body: Box::new(body),
        source_file: None,
    }
}

pub fn arithmetic(expression: &str) -> Command {
    Command::Arithmetic {
        line: None,
        expression: expression.to_string(),
    }
}

pub fn arithmetic_for(init: &str, test: &str, step: &str, body: Command) -> Command {
    Command::ArithmeticFor {
        line: None,
        init: init.to_string(),
        test: test.to_string(),
        step: step.to_string(),
        body: Box::new(body),
    }
}

pub const fn conditional(expr: ConditionalExpr) -> Command {
    Command::Conditional { line: None, expr }
}

pub fn cond_unary(op: &str, arg: &str) -> ConditionalExpr {
    ConditionalExpr::Unary {
        op: op.to_string(),
        arg: arg.to_string(),
    }
}

pub fn cond_binary(left: &str, op: &str, right: &str) -> ConditionalExpr {
    ConditionalExpr::Binary {
        left: left.to_string(),
        op: op.to_string(),
        right: right.to_string(),
    }
}

pub fn cond_and(left: ConditionalExpr, right: ConditionalExpr) -> ConditionalExpr {
    ConditionalExpr::And {
        left: Box::new(left),
        right: Box::new(right),
    }
}

pub fn cond_or(left: ConditionalExpr, right: ConditionalExpr) -> ConditionalExpr {
    ConditionalExpr::Or {
        left: Box::new(left),
        right: Box::new(right),
    }
}

pub fn cond_not(expr: ConditionalExpr) -> ConditionalExpr {
    ConditionalExpr::Not {
        expr: Box::new(expr),
    }
}

pub fn cond_term(word: &str) -> ConditionalExpr {
    ConditionalExpr::Term {
        word: word.to_string(),
    }
}

pub fn cond_expr(expr: ConditionalExpr) -> ConditionalExpr {
    ConditionalExpr::Expr {
        expr: Box::new(expr),
    }
}

pub fn coproc(name: Option<&str>, body: Command) -> Command {
    Command::Coproc {
        line: None,
        name: name.map(str::to_string),
        body: Box::new(body),
    }
}

pub fn redirect_file(direction: RedirectType, source_fd: Option<i32>, target: &str) -> Redirect {
    Redirect {
        direction,
        source_fd,
        target: RedirectTarget::File(target.to_string()),
        here_doc_eof: None,
    }
}

pub const fn redirect_fd(direction: RedirectType, source_fd: Option<i32>, target: i32) -> Redirect {
    Redirect {
        direction,
        source_fd,
        target: RedirectTarget::Fd(target),
        here_doc_eof: None,
    }
}

pub fn heredoc(content: &str, eof: Option<&str>) -> Redirect {
    Redirect {
        direction: RedirectType::HereDoc,
        source_fd: Some(0),
        target: RedirectTarget::File(content.to_string()),
        here_doc_eof: eof.map(str::to_string),
    }
}
