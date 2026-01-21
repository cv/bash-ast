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
    match cmd {
        Command::Simple {
            words,
            redirects,
            assignments,
            ..
        } => {
            write_simple(words, redirects, assignments.as_deref(), out);
        }
        Command::Pipeline {
            commands, negated, ..
        } => {
            write_pipeline(commands, *negated, out);
        }
        Command::List { op, left, right, .. } => {
            write_list(op, left, right, out);
        }
        Command::For {
            variable,
            words,
            body,
            ..
        } => {
            write_for(variable, words.as_deref(), body, out);
        }
        Command::While { test, body, .. } => {
            write_while(test, body, out);
        }
        Command::Until { test, body, .. } => {
            write_until(test, body, out);
        }
        Command::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            write_if(condition, then_branch, else_branch.as_ref(), out);
        }
        Command::Case { word, clauses, .. } => {
            write_case(word, clauses, out);
        }
        Command::Select {
            variable,
            words,
            body,
            ..
        } => {
            write_select(variable, words.as_deref(), body, out);
        }
        Command::Group { body, .. } => {
            write_group(body, out);
        }
        Command::Subshell { body, .. } => {
            write_subshell(body, out);
        }
        Command::FunctionDef { name, body, .. } => {
            write_function_def(name, body, out);
        }
        Command::Arithmetic { expression, .. } => {
            write_arithmetic(expression, out);
        }
        Command::ArithmeticFor {
            init,
            test,
            step,
            body,
            ..
        } => {
            write_arith_for(init, test, step, body, out);
        }
        Command::Conditional { expr, .. } => {
            write_conditional(expr, out);
        }
        Command::Coproc { name, body, .. } => {
            write_coproc(name.as_deref(), body, out);
        }
    }
}

/// Write a simple command (cmd arg1 arg2 ...)
fn write_simple(
    words: &[Word],
    redirects: &[Redirect],
    assignments: Option<&[String]>,
    out: &mut String,
) {
    // Write assignments first
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

    // Write redirects
    for redirect in redirects {
        out.push(' ');
        write_redirect(redirect, out);
    }
}

/// Write a redirect
fn write_redirect(redirect: &Redirect, out: &mut String) {
    // Handle special cases that have their own format
    match redirect.direction {
        RedirectType::HereDoc => {
            // <<EOF\ncontent\nEOF format
            if let Some(fd) = redirect.source_fd {
                if fd != 0 {
                    out.push_str(&fd.to_string());
                }
            }
            out.push_str("<<");
            let eof = redirect.here_doc_eof.as_deref().unwrap_or("EOF");
            out.push_str(eof);
            if let RedirectTarget::File(content) = &redirect.target {
                out.push('\n');
                out.push_str(content);
                if !content.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(eof);
            }
            return;
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
        RedirectType::DupInput => out.push_str("<&"),
        RedirectType::DupOutput => out.push_str(">&"),
        RedirectType::ErrAndOut => out.push_str("&>"),
        RedirectType::AppendErrAndOut => out.push_str("&>>"),
        RedirectType::MoveInput => out.push_str("<&"),
        RedirectType::MoveOutput => out.push_str(">&"),
        RedirectType::HereDoc | RedirectType::Close => unreachable!(), // Handled above
    }

    // Write target
    match &redirect.target {
        RedirectTarget::File(filename) => {
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

/// Write a list (cmd1 && cmd2, cmd1 || cmd2, etc.)
fn write_list(op: &ListOp, left: &Command, right: &Command, out: &mut String) {
    write_command(left, out);

    match op {
        ListOp::And => out.push_str(" && "),
        ListOp::Or => out.push_str(" || "),
        ListOp::Semi => out.push_str("; "),
        ListOp::Amp => out.push_str(" & "),
        ListOp::Newline => out.push('\n'),
    }

    write_command(right, out);
}

/// Write a for loop
fn write_for(variable: &str, words: Option<&[String]>, body: &Command, out: &mut String) {
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
    out.push_str("; done");
}

/// Write a while loop
fn write_while(test: &Command, body: &Command, out: &mut String) {
    out.push_str("while ");
    write_command(test, out);
    out.push_str("; do ");
    write_command(body, out);
    out.push_str("; done");
}

/// Write an until loop
fn write_until(test: &Command, body: &Command, out: &mut String) {
    out.push_str("until ");
    write_command(test, out);
    out.push_str("; do ");
    write_command(body, out);
    out.push_str("; done");
}

/// Write an if statement
fn write_if(
    condition: &Command,
    then_branch: &Command,
    else_branch: Option<&Box<Command>>,
    out: &mut String,
) {
    out.push_str("if ");
    write_command(condition, out);
    out.push_str("; then ");
    write_command(then_branch, out);
    
    if let Some(else_cmd) = else_branch {
        // Check if it's an elif (nested if in else)
        if let Command::If { condition: elif_cond, then_branch: elif_then, else_branch: elif_else, .. } = else_cmd.as_ref() {
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
}

/// Helper to write elif/else chains
fn write_else_chain(cmd: &Command, out: &mut String) {
    if let Command::If { condition, then_branch, else_branch, .. } = cmd {
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
fn write_case(word: &str, clauses: &[CaseClause], out: &mut String) {
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
}

/// Write a select statement
fn write_select(variable: &str, words: Option<&[String]>, body: &Command, out: &mut String) {
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
    out.push_str("; done");
}

/// Write a brace group
fn write_group(body: &Command, out: &mut String) {
    out.push_str("{ ");
    write_command(body, out);
    out.push_str("; }");
}

/// Write a subshell
fn write_subshell(body: &Command, out: &mut String) {
    out.push('(');
    write_command(body, out);
    out.push(')');
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

    /// Helper to test round-trip: parse -> to_bash -> parse -> compare structure
    fn assert_round_trip(script: &str) {
        setup();
        let ast1 = parse(script).expect(&format!("Failed to parse original: {}", script));
        let regenerated = to_bash(&ast1);
        let ast2 = parse(&regenerated).expect(&format!(
            "Failed to parse regenerated script: {}\nOriginal: {}",
            regenerated, script
        ));
        
        // Compare JSON representations (ignoring line numbers)
        let json1 = serde_json::to_string(&ast1).unwrap();
        let json2 = serde_json::to_string(&ast2).unwrap();
        
        // Remove line numbers for comparison
        let json1_no_lines = remove_line_numbers(&json1);
        let json2_no_lines = remove_line_numbers(&json2);
        
        assert_eq!(
            json1_no_lines, json2_no_lines,
            "AST mismatch!\nOriginal: {}\nRegenerated: {}\nAST1: {}\nAST2: {}",
            script, regenerated, json1, json2
        );
    }
    
    /// Remove line numbers from JSON for comparison
    fn remove_line_numbers(json: &str) -> String {
        // Simple regex-like removal of "line":N patterns
        let mut result = String::new();
        let mut chars = json.chars().peekable();
        
        while let Some(c) = chars.next() {
            if c == '"' {
                result.push(c);
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
            } else {
                result.push(c);
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
}
