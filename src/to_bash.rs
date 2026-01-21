//! Convert AST back to bash script
//!
//! This module converts a bash AST (as produced by `parse()`) back into
//! executable bash code. The output may not be formatted identically to
//! the original, but it will be semantically equivalent.

use crate::ast::{
    CaseClause, Command, ConditionalExpr, ListOp, Redirect, RedirectTarget, RedirectType, Word,
};

/// Convert a Command AST to a bash script string
///
/// # Example
///
/// ```no_run
/// use bash_ast::{parse, init, to_bash};
///
/// init();
///
/// let cmd = parse("for i in a b c; do echo $i; done").unwrap();
/// let script = to_bash(&cmd);
/// // script will be something like: for i in a b c; do echo $i; done
/// ```
#[must_use]
pub fn to_bash(cmd: &Command) -> String {
    let mut output = String::new();
    write_command(cmd, &mut output);
    output
}

/// Write a command to the output string
fn write_command(cmd: &Command, out: &mut String) {
    write_command_impl(cmd, out, true);
}

/// Write a command, optionally including heredoc content
fn write_command_impl(cmd: &Command, out: &mut String, include_heredoc_content: bool) {
    match cmd {
        Command::Simple {
            words,
            redirects,
            assignments,
            ..
        } => {
            write_simple_impl(
                words,
                redirects,
                assignments.as_deref(),
                out,
                include_heredoc_content,
            );
        }
        Command::Pipeline {
            commands, negated, ..
        } => write_pipeline(commands, *negated, out),
        Command::List {
            op, left, right, ..
        } => write_list(*op, left, right, out),
        Command::For {
            variable,
            words,
            body,
            redirects,
            ..
        } => {
            write_for(variable, words.as_deref(), body, redirects, out);
        }
        Command::While {
            test,
            body,
            redirects,
            ..
        } => write_while(test, body, redirects, out),
        Command::Until {
            test,
            body,
            redirects,
            ..
        } => write_until(test, body, redirects, out),
        Command::If {
            condition,
            then_branch,
            else_branch,
            redirects,
            ..
        } => {
            write_if(
                condition,
                then_branch,
                else_branch.as_deref(),
                redirects,
                out,
            );
        }
        Command::Case {
            word,
            clauses,
            redirects,
            ..
        } => write_case(word, clauses, redirects, out),
        Command::Select {
            variable,
            words,
            body,
            redirects,
            ..
        } => {
            write_select(variable, words.as_deref(), body, redirects, out);
        }
        Command::Group {
            body, redirects, ..
        } => write_group(body, redirects, out),
        Command::Subshell {
            body, redirects, ..
        } => write_subshell(body, redirects, out),
        Command::FunctionDef { name, body, .. } => write_function_def(name, body, out),
        Command::Arithmetic { expression, .. } => write_arithmetic(expression, out),
        Command::ArithmeticFor {
            init,
            test,
            step,
            body,
            ..
        } => {
            write_arith_for(init, test, step, body, out);
        }
        Command::Conditional { expr, .. } => write_conditional(expr, out),
        Command::Coproc { name, body, .. } => write_coproc(name.as_deref(), body, out),
    }
}

/// Write a simple command (cmd arg1 arg2 ...)
fn write_simple_impl(
    words: &[Word],
    redirects: &[Redirect],
    assignments: Option<&[String]>,
    out: &mut String,
    include_heredoc_content: bool,
) {
    // Check if the command is a builtin that takes assignments as arguments
    // (like local, export, declare, readonly, typeset)
    let is_assignment_builtin = words.first().is_some_and(|w| {
        matches!(
            w.word.as_str(),
            "local" | "export" | "declare" | "readonly" | "typeset"
        )
    });

    if is_assignment_builtin {
        // For assignment builtins: cmd assignments...
        // Write command word first
        if let Some(word) = words.first() {
            out.push_str(&word.word);
        }

        // Write remaining words
        for word in words.iter().skip(1) {
            out.push(' ');
            out.push_str(&word.word);
        }

        // Write assignments after
        if let Some(assigns) = assignments {
            for assign in assigns {
                out.push(' ');
                out.push_str(assign);
            }
        }
    } else {
        // For regular commands: assignments cmd args...
        // Write assignments first (VAR=value cmd)
        if let Some(assigns) = assignments {
            for (i, assign) in assigns.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                out.push_str(assign);
            }
            if !words.is_empty() {
                out.push(' ');
            }
        }

        // Write command words
        for (i, word) in words.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            out.push_str(&word.word);
        }
    }

    // Write redirects
    write_redirects_impl(redirects, out, include_heredoc_content);
}

/// Write multiple redirects
/// Heredocs are handled specially - their content comes after all other redirects
fn write_redirects(redirects: &[Redirect], out: &mut String) {
    write_redirects_impl(redirects, out, true);
}

/// Write redirects, optionally including heredoc content
fn write_redirects_impl(redirects: &[Redirect], out: &mut String, include_heredoc_content: bool) {
    // First pass: write non-heredoc redirects
    for redirect in redirects {
        if redirect.direction != RedirectType::HereDoc {
            out.push(' ');
            write_redirect(redirect, out);
        }
    }
    // Second pass: write heredoc markers (<<EOF)
    for redirect in redirects {
        if redirect.direction == RedirectType::HereDoc {
            out.push(' ');
            write_heredoc_marker(redirect, out);
        }
    }
    // Third pass: write heredoc content (after a newline) if requested
    if include_heredoc_content {
        for redirect in redirects {
            if redirect.direction == RedirectType::HereDoc {
                out.push('\n');
                write_heredoc_content(redirect, out);
            }
        }
    }
}

/// Write just the heredoc marker (<<EOF)
fn write_heredoc_marker(redirect: &Redirect, out: &mut String) {
    if let Some(fd) = redirect.source_fd {
        if fd != 0 {
            out.push_str(&fd.to_string());
        }
    }
    out.push_str("<<");
    let eof = redirect.here_doc_eof.as_deref().unwrap_or("EOF");
    out.push_str(eof);
}

/// Write heredoc content and closing delimiter
fn write_heredoc_content(redirect: &Redirect, out: &mut String) {
    let eof = redirect.here_doc_eof.as_deref().unwrap_or("EOF");
    if let RedirectTarget::File(content) = &redirect.target {
        out.push_str(content);
        if !content.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(eof);
    }
}

/// Write a redirect
fn write_redirect(redirect: &Redirect, out: &mut String) {
    // Handle special cases that have their own format
    match redirect.direction {
        RedirectType::HereDoc => {
            // Heredocs are handled by write_redirects
            unreachable!("Heredocs should be handled by write_redirects");
        }
        RedirectType::Close => {
            // N>&- or N<&- format
            if let Some(fd) = redirect.source_fd {
                out.push_str(&fd.to_string());
            }
            out.push_str(">&-");
            return;
        }
        _ => {}
    }

    // Get the default fd for this redirect type
    let default_fd = match redirect.direction {
        RedirectType::Input
        | RedirectType::HereString
        | RedirectType::DupInput
        | RedirectType::MoveInput
        | RedirectType::InputOutput => 0,
        RedirectType::Output
        | RedirectType::Append
        | RedirectType::Clobber
        | RedirectType::DupOutput
        | RedirectType::MoveOutput => 1,
        RedirectType::ErrAndOut | RedirectType::AppendErrAndOut => -1, // No default, uses &>
        RedirectType::HereDoc | RedirectType::Close => unreachable!(), // Handled above
    };

    // Write source fd if it's not the default
    if let Some(fd) = redirect.source_fd {
        // &> and &>> don't take a source fd prefix
        if fd != default_fd
            && redirect.direction != RedirectType::ErrAndOut
            && redirect.direction != RedirectType::AppendErrAndOut
        {
            out.push_str(&fd.to_string());
        }
    }

    // Write the redirect operator
    match redirect.direction {
        RedirectType::Input => out.push('<'),
        RedirectType::Output => out.push('>'),
        RedirectType::Append => out.push_str(">>"),
        RedirectType::HereString => out.push_str("<<<"),
        RedirectType::InputOutput => out.push_str("<>"),
        RedirectType::Clobber => out.push_str(">|"),
        RedirectType::DupInput | RedirectType::MoveInput => out.push_str("<&"),
        RedirectType::DupOutput | RedirectType::MoveOutput => out.push_str(">&"),
        RedirectType::ErrAndOut => out.push_str("&>"),
        RedirectType::AppendErrAndOut => out.push_str("&>>"),
        RedirectType::HereDoc | RedirectType::Close => unreachable!(), // Handled above
    }

    // Write target
    match &redirect.target {
        RedirectTarget::File(filename) => {
            // Add space if target starts with < or > to avoid confusion with heredocs
            // e.g., "< <(cmd)" not "<<(cmd)"
            if filename.starts_with('<') || filename.starts_with('>') {
                out.push(' ');
            }
            out.push_str(filename);
        }
        RedirectTarget::Fd(fd) => {
            out.push_str(&fd.to_string());
            // For move operations, add the dash
            if matches!(
                redirect.direction,
                RedirectType::MoveInput | RedirectType::MoveOutput
            ) {
                out.push('-');
            }
        }
    }
}

/// Write a pipeline (cmd1 | cmd2 | cmd3)
fn write_pipeline(commands: &[Command], negated: bool, out: &mut String) {
    if negated {
        out.push_str("! ");
    }
    for (i, cmd) in commands.iter().enumerate() {
        if i > 0 {
            out.push_str(" | ");
        }
        write_command(cmd, out);
    }
}

/// Get the line number of the first token in a command
/// Get the first line number of a command (for determining where it starts).
/// For List nodes, recurses into the left child to find the actual first command.
fn get_first_line(cmd: &Command) -> Option<u32> {
    match cmd {
        Command::List { left, .. } => get_first_line(left),
        _ => cmd.line(),
    }
}

/// Get the last line number of a command (for determining where it ends).
/// For List nodes, recurses into the right child to find the actual last command.
fn get_last_line(cmd: &Command) -> Option<u32> {
    match cmd {
        Command::List { right, .. } => get_last_line(right),
        _ => cmd.line(),
    }
}

/// Check if a command has any heredoc redirects (requires newline after)
fn has_heredoc(cmd: &Command) -> bool {
    match cmd {
        Command::List { left, right, .. } => has_heredoc(left) || has_heredoc(right),
        Command::Pipeline { commands, .. } => commands.iter().any(has_heredoc),
        _ => cmd
            .redirects()
            .is_some_and(|r| r.iter().any(|r| r.direction == RedirectType::HereDoc)),
    }
}

/// Collect all heredocs from a command tree
fn collect_heredocs(cmd: &Command) -> Vec<&Redirect> {
    let mut heredocs = Vec::new();
    collect_heredocs_impl(cmd, &mut heredocs);
    heredocs
}

fn collect_heredocs_impl<'a>(cmd: &'a Command, heredocs: &mut Vec<&'a Redirect>) {
    match cmd {
        Command::List { left, right, .. } => {
            collect_heredocs_impl(left, heredocs);
            collect_heredocs_impl(right, heredocs);
        }
        Command::Pipeline { commands, .. } => {
            for c in commands {
                collect_heredocs_impl(c, heredocs);
            }
        }
        _ => {
            if let Some(redirects) = cmd.redirects() {
                for r in redirects {
                    if r.direction == RedirectType::HereDoc {
                        heredocs.push(r);
                    }
                }
            }
        }
    }
}

/// Write a list (cmd1 && cmd2, cmd1 || cmd2, etc.)
fn write_list(op: ListOp, left: &Command, right: &Command, out: &mut String) {
    // For And/Or with heredocs, we need special handling:
    // The heredoc content must come AFTER the entire command line
    let left_has_heredoc = has_heredoc(left);
    let defer_heredocs = (op == ListOp::And || op == ListOp::Or) && left_has_heredoc;

    if defer_heredocs {
        // Write left command without heredoc content
        write_command_impl(left, out, false);
    } else {
        write_command(left, out);
    }

    // Check if right side is empty (e.g., "cmd &" has empty right side)
    let right_is_empty = matches!(
        right,
        Command::Simple { words, redirects, assignments, .. }
        if words.is_empty() && redirects.is_empty() && assignments.is_none()
    );

    // For semi, use newline if:
    // 1. Commands are on different lines, OR
    // 2. Left command has a heredoc (heredoc content requires newline after delimiter)
    let use_newline = op == ListOp::Semi
        && (left_has_heredoc
            || match (get_last_line(left), get_first_line(right)) {
                (Some(l), Some(r)) => r > l,
                _ => false,
            });

    match op {
        ListOp::And => out.push_str(" && "),
        ListOp::Or => out.push_str(" || "),
        ListOp::Semi if use_newline => out.push('\n'),
        ListOp::Semi => out.push_str("; "),
        ListOp::Amp => {
            out.push_str(" &");
            if !right_is_empty {
                out.push(' ');
            }
        }
        ListOp::Newline => out.push('\n'),
    }

    if !right_is_empty {
        write_command(right, out);
    }

    // Now write deferred heredoc content
    if defer_heredocs {
        for heredoc in collect_heredocs(left) {
            out.push('\n');
            write_heredoc_content(heredoc, out);
        }
    }
}

/// Check if a command ends with a background operator (needs no semicolon after)
fn ends_with_background(cmd: &Command) -> bool {
    match cmd {
        Command::List {
            op: ListOp::Amp,
            right,
            ..
        } => {
            // Check if right is empty (pure background) or also ends with &
            matches!(
                right.as_ref(),
                Command::Simple { words, redirects, assignments, .. }
                if words.is_empty() && redirects.is_empty() && assignments.is_none()
            ) || ends_with_background(right)
        }
        _ => false,
    }
}

/// Write a for loop
fn write_for(
    variable: &str,
    words: Option<&[String]>,
    body: &Command,
    redirects: &[Redirect],
    out: &mut String,
) {
    out.push_str("for ");
    out.push_str(variable);
    if let Some(w) = words {
        out.push_str(" in");
        for word in w {
            out.push(' ');
            out.push_str(word);
        }
    }
    out.push_str("; do ");
    write_command(body, out);
    if ends_with_background(body) {
        out.push_str(" done");
    } else {
        out.push_str("; done");
    }
    write_redirects(redirects, out);
}

/// Write a while loop
fn write_while(test: &Command, body: &Command, redirects: &[Redirect], out: &mut String) {
    out.push_str("while ");
    write_command(test, out);
    out.push_str("; do ");
    write_command(body, out);
    if ends_with_background(body) {
        out.push_str(" done");
    } else {
        out.push_str("; done");
    }
    write_redirects(redirects, out);
}

/// Write an until loop
fn write_until(test: &Command, body: &Command, redirects: &[Redirect], out: &mut String) {
    out.push_str("until ");
    write_command(test, out);
    out.push_str("; do ");
    write_command(body, out);
    if ends_with_background(body) {
        out.push_str(" done");
    } else {
        out.push_str("; done");
    }
    write_redirects(redirects, out);
}

/// Write an if statement
fn write_if(
    condition: &Command,
    then_branch: &Command,
    else_branch: Option<&Command>,
    redirects: &[Redirect],
    out: &mut String,
) {
    out.push_str("if ");
    write_command(condition, out);
    out.push_str("; then ");
    write_command(then_branch, out);

    if let Some(else_cmd) = else_branch {
        // Check if it's an elif (nested if in else)
        if let Command::If {
            condition: elif_cond,
            then_branch: elif_then,
            else_branch: elif_else,
            ..
        } = else_cmd
        {
            out.push_str("; elif ");
            write_command(elif_cond, out);
            out.push_str("; then ");
            write_command(elif_then, out);
            // Recursively handle more elif/else
            if let Some(nested_else) = elif_else {
                write_else_chain(nested_else.as_ref(), out);
            }
        } else {
            out.push_str("; else ");
            write_command(else_cmd, out);
        }
    }

    out.push_str("; fi");
    write_redirects(redirects, out);
}

/// Helper to write elif/else chains
fn write_else_chain(cmd: &Command, out: &mut String) {
    if let Command::If {
        condition,
        then_branch,
        else_branch,
        ..
    } = cmd
    {
        out.push_str("; elif ");
        write_command(condition, out);
        out.push_str("; then ");
        write_command(then_branch, out);
        if let Some(nested_else) = else_branch {
            write_else_chain(nested_else.as_ref(), out);
        }
    } else {
        out.push_str("; else ");
        write_command(cmd, out);
    }
}

/// Write a case statement
fn write_case(word: &str, clauses: &[CaseClause], redirects: &[Redirect], out: &mut String) {
    out.push_str("case ");
    out.push_str(word);
    out.push_str(" in ");

    for clause in clauses {
        // Write patterns
        for (i, pattern) in clause.patterns.iter().enumerate() {
            if i > 0 {
                out.push('|');
            }
            out.push_str(pattern);
        }
        out.push_str(") ");

        // Write action
        if let Some(action) = &clause.action {
            write_command(action, out);
        }

        // Write terminator
        if let Some(flags) = &clause.flags {
            if flags.fallthrough {
                out.push_str(";&");
            } else if flags.test_next {
                out.push_str(";;&");
            } else {
                out.push_str(";;");
            }
        } else {
            out.push_str(";;");
        }
        out.push(' ');
    }

    out.push_str("esac");
    write_redirects(redirects, out);
}

/// Write a select statement
fn write_select(
    variable: &str,
    words: Option<&[String]>,
    body: &Command,
    redirects: &[Redirect],
    out: &mut String,
) {
    out.push_str("select ");
    out.push_str(variable);
    if let Some(w) = words {
        out.push_str(" in");
        for word in w {
            out.push(' ');
            out.push_str(word);
        }
    }
    out.push_str("; do ");
    write_command(body, out);
    if ends_with_background(body) {
        out.push_str(" done");
    } else {
        out.push_str("; done");
    }
    write_redirects(redirects, out);
}

/// Write a brace group
fn write_group(body: &Command, redirects: &[Redirect], out: &mut String) {
    out.push_str("{ ");
    write_command(body, out);
    out.push_str("; }");
    write_redirects(redirects, out);
}

/// Write a subshell
fn write_subshell(body: &Command, redirects: &[Redirect], out: &mut String) {
    out.push('(');
    write_command(body, out);
    out.push(')');
    write_redirects(redirects, out);
}

/// Write a function definition
fn write_function_def(name: &str, body: &Command, out: &mut String) {
    out.push_str(name);
    out.push_str("() ");
    write_command(body, out);
}

/// Write an arithmetic expression
fn write_arithmetic(expression: &str, out: &mut String) {
    out.push_str("((");
    out.push_str(expression);
    out.push_str("))");
}

/// Write an arithmetic for loop
fn write_arith_for(init: &str, test: &str, step: &str, body: &Command, out: &mut String) {
    out.push_str("for ((");
    out.push_str(init);
    out.push_str("; ");
    out.push_str(test);
    out.push_str("; ");
    out.push_str(step);
    out.push_str(")); do ");
    write_command(body, out);
    out.push_str("; done");
}

/// Write a conditional expression [[ ... ]]
fn write_conditional(expr: &ConditionalExpr, out: &mut String) {
    out.push_str("[[ ");
    write_cond_expr(expr, out);
    out.push_str(" ]]");
}

/// Write a conditional expression (internal)
fn write_cond_expr(expr: &ConditionalExpr, out: &mut String) {
    match expr {
        ConditionalExpr::Unary { op, arg } => {
            out.push_str(op);
            out.push(' ');
            out.push_str(arg);
        }
        ConditionalExpr::Binary { op, left, right } => {
            out.push_str(left);
            out.push(' ');
            out.push_str(op);
            out.push(' ');
            out.push_str(right);
        }
        ConditionalExpr::And { left, right } => {
            write_cond_expr(left, out);
            out.push_str(" && ");
            write_cond_expr(right, out);
        }
        ConditionalExpr::Or { left, right } => {
            write_cond_expr(left, out);
            out.push_str(" || ");
            write_cond_expr(right, out);
        }
        ConditionalExpr::Not { expr } => {
            out.push_str("! ");
            write_cond_expr(expr, out);
        }
        ConditionalExpr::Term { word } => {
            out.push_str(word);
        }
        ConditionalExpr::Expr { expr } => {
            out.push_str("( ");
            write_cond_expr(expr, out);
            out.push_str(" )");
        }
    }
}

/// Write a coproc
fn write_coproc(name: Option<&str>, body: &Command, out: &mut String) {
    out.push_str("coproc ");
    // Only print name if it's not the default "COPROC" or if body is a group/complex command
    // When body is a simple command and name is COPROC, bash auto-generates the name
    let is_default_name = name == Some("COPROC");
    let is_simple_body = matches!(body, Command::Simple { .. });

    if let Some(n) = name {
        // Only write the name explicitly if it's not the default or the body is a group
        if !is_default_name || !is_simple_body {
            out.push_str(n);
            out.push(' ');
        }
    }
    write_command(body, out);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{init, parse};

    fn setup() {
        init();
    }

    /// Helper to test round-trip: parse -> `to_bash` -> parse -> compare structure
    fn assert_round_trip(script: &str) {
        setup();
        let ast1 =
            parse(script).unwrap_or_else(|e| panic!("Failed to parse original: {script}: {e}"));
        let regenerated = to_bash(&ast1);
        let ast2 = parse(&regenerated).unwrap_or_else(|e| {
            panic!("Failed to parse regenerated script: {regenerated}\nOriginal: {script}: {e}")
        });

        // Compare JSON representations (ignoring line numbers)
        let json1 = serde_json::to_string(&ast1).unwrap();
        let json2 = serde_json::to_string(&ast2).unwrap();

        // Remove line numbers for comparison
        let json1_no_lines = remove_line_numbers(&json1);
        let json2_no_lines = remove_line_numbers(&json2);

        assert_eq!(
            json1_no_lines, json2_no_lines,
            "AST mismatch!\nOriginal: {script}\nRegenerated: {regenerated}\nAST1: {json1}\nAST2: {json2}"
        );
    }

    /// Remove line numbers from JSON for comparison
    fn remove_line_numbers(json: &str) -> String {
        // Simple regex-like removal of "line":N patterns
        let mut result = String::new();
        let mut chars = json.chars().peekable();

        while let Some(c) = chars.next() {
            result.push(c);
            if c == '"' {
                // Read the key
                let mut key = String::new();
                while let Some(&nc) = chars.peek() {
                    chars.next();
                    if nc == '"' {
                        result.push_str(&key);
                        result.push(nc);
                        break;
                    }
                    key.push(nc);
                }

                // Check if it's "line"
                if key == "line" {
                    // Skip :N, or :null
                    while let Some(&nc) = chars.peek() {
                        if nc == ',' || nc == '}' || nc == ']' {
                            break;
                        }
                        chars.next();
                    }
                    // Skip the trailing comma if present
                    if chars.peek() == Some(&',') {
                        chars.next();
                    }
                    // Remove the "line": we just added
                    let line_len = "\"line\"".len();
                    result.truncate(result.len() - line_len);
                }
            }
        }

        result
    }

    #[test]
    fn test_simple_command() {
        assert_round_trip("echo hello world");
    }

    #[test]
    fn test_pipeline() {
        assert_round_trip("cat file | grep pattern | sort");
    }

    #[test]
    fn test_negated_pipeline() {
        assert_round_trip("! cmd1 | cmd2 | cmd3");
    }

    #[test]
    fn test_and_list() {
        assert_round_trip("cmd1 && cmd2");
    }

    #[test]
    fn test_or_list() {
        assert_round_trip("cmd1 || cmd2");
    }

    #[test]
    fn test_semi_list() {
        assert_round_trip("cmd1; cmd2");
    }

    #[test]
    fn test_background() {
        assert_round_trip("long_running_cmd & wait");
    }

    #[test]
    fn test_for_loop() {
        assert_round_trip("for i in a b c; do echo $i; done");
    }

    #[test]
    fn test_for_loop_no_in() {
        assert_round_trip("for i; do echo $i; done");
    }

    #[test]
    fn test_while_loop() {
        assert_round_trip("while true; do echo loop; done");
    }

    #[test]
    fn test_until_loop() {
        assert_round_trip("until false; do echo done; done");
    }

    #[test]
    fn test_if_simple() {
        assert_round_trip("if true; then echo yes; fi");
    }

    #[test]
    fn test_if_else() {
        assert_round_trip("if true; then echo yes; else echo no; fi");
    }

    #[test]
    fn test_if_elif_else() {
        assert_round_trip("if test1; then cmd1; elif test2; then cmd2; else cmd3; fi");
    }

    #[test]
    fn test_case() {
        assert_round_trip("case $x in a) echo a;; b) echo b;; esac");
    }

    #[test]
    fn test_case_multiple_patterns() {
        assert_round_trip("case $x in a|b|c) echo match;; *) echo default;; esac");
    }

    #[test]
    fn test_case_fallthrough() {
        assert_round_trip("case $x in a) echo a;& b) echo b;; esac");
    }

    #[test]
    fn test_case_test_next() {
        assert_round_trip("case $x in a) echo a;;& b) echo b;; esac");
    }

    #[test]
    fn test_select() {
        assert_round_trip("select opt in a b c; do echo $opt; done");
    }

    #[test]
    fn test_group() {
        assert_round_trip("{ echo hello; echo world; }");
    }

    #[test]
    fn test_subshell() {
        assert_round_trip("(echo hello; echo world)");
    }

    #[test]
    fn test_function() {
        assert_round_trip("foo() { echo bar; }");
    }

    #[test]
    fn test_arithmetic() {
        assert_round_trip("(( x = 1 + 2 ))");
    }

    #[test]
    fn test_arith_for() {
        assert_round_trip("for ((i=0; i<10; i++)); do echo $i; done");
    }

    #[test]
    fn test_conditional_unary() {
        assert_round_trip("[[ -f file ]]");
    }

    #[test]
    fn test_conditional_binary() {
        assert_round_trip("[[ $a == $b ]]");
    }

    #[test]
    fn test_conditional_and() {
        assert_round_trip("[[ -f file && -r file ]]");
    }

    #[test]
    fn test_conditional_or() {
        assert_round_trip("[[ -f file || -d file ]]");
    }

    #[test]
    fn test_conditional_grouped() {
        assert_round_trip("[[ ( -f x && -r x ) ]]");
    }

    #[test]
    fn test_coproc_simple() {
        assert_round_trip("coproc cat");
    }

    #[test]
    fn test_coproc_named() {
        assert_round_trip("coproc MYPROC { cat; }");
    }

    #[test]
    fn test_redirect_output() {
        assert_round_trip("echo hello > file.txt");
    }

    #[test]
    fn test_redirect_input() {
        assert_round_trip("cat < file.txt");
    }

    #[test]
    fn test_redirect_append() {
        assert_round_trip("echo hello >> file.txt");
    }

    #[test]
    fn test_redirect_dup() {
        assert_round_trip("cmd 2>&1");
    }

    #[test]
    fn test_redirect_err_and_out() {
        assert_round_trip("cmd &> file");
    }

    #[test]
    fn test_assignments() {
        assert_round_trip("VAR=value cmd");
    }

    #[test]
    fn test_multiple_assignments() {
        assert_round_trip("A=1 B=2 C=3 cmd");
    }

    #[test]
    fn test_complex_nested() {
        assert_round_trip(
            "if true; then for i in 1 2 3; do while true; do echo $i; break; done; done; fi",
        );
    }

    #[test]
    fn test_heredoc() {
        setup();
        let script = "cat <<EOF\nhello world\nEOF";
        let ast = parse(script).unwrap();
        let regenerated = to_bash(&ast);
        // Just verify it contains the key parts
        assert!(regenerated.contains("cat"));
        assert!(regenerated.contains("<<EOF"));
        assert!(regenerated.contains("hello world"));
    }

    #[test]
    fn test_herestring() {
        assert_round_trip("cat <<<'hello world'");
    }

    #[test]
    fn test_redirect_close() {
        assert_round_trip("exec 3>&-");
    }

    #[test]
    fn test_redirect_input_output() {
        assert_round_trip("exec 3<>/tmp/file");
    }

    #[test]
    fn test_redirect_clobber() {
        assert_round_trip("echo hello >|file");
    }

    #[test]
    fn test_local_assignment() {
        assert_round_trip("local name=value");
    }

    #[test]
    fn test_export_assignment() {
        assert_round_trip("export PATH=/usr/bin");
    }

    #[test]
    fn test_declare_assignment() {
        assert_round_trip("declare -r CONST=42");
    }
}
