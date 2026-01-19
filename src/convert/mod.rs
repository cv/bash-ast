//! C AST to Rust AST conversion
//!
//! This module converts bash's internal C command structures
//! to our Rust AST representation.

// FFI code necessarily uses unsafe and casts between C and Rust types
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_truncation)]

mod helpers;

use crate::ast::Command;
use helpers::{convert_redirects, convert_word_list, convert_word_list_to_strings, cstr_to_string};

// Re-export the main entry point
pub use self::convert_impl::convert_command;

/// Maximum recursion depth for AST conversion (256 levels)
///
/// This prevents stack overflow from deeply nested bash scripts.
const MAX_DEPTH: usize = 256;

/// Maximum linked list length (100,000 items)
///
/// This prevents infinite loops from malformed/cyclic linked list data.
const MAX_LIST_LENGTH: usize = 100_000;

// Constants that may not be exported by bindgen
// These values come from bash's command.h

/// `W_ASSIGNMENT` flag - word is a variable assignment
const W_ASSIGNMENT: u32 = 1 << 2;

/// `CASEPAT_FALLTHROUGH` - case clause falls through (;&)
const CASEPAT_FALLTHROUGH: i32 = 0x01;

/// `CASEPAT_TESTNEXT` - case clause tests next pattern (;;&)
const CASEPAT_TESTNEXT: i32 = 0x02;

/// `COND_AND` - conditional AND
const COND_AND: i32 = 1;

/// `COND_OR` - conditional OR
const COND_OR: i32 = 2;

/// `COND_UNARY` - unary conditional test
const COND_UNARY: i32 = 3;

/// `COND_BINARY` - binary conditional test
const COND_BINARY: i32 = 4;

/// `COND_TERM` - conditional term
const COND_TERM: i32 = 5;

/// `COND_EXPR` - conditional expression (grouped)
const COND_EXPR: i32 = 6;

/// `CMD_INVERT_RETURN` flag
const CMD_INVERT_RETURN: i32 = 0x04;

/// Convert a line number to Option, filtering out invalid values
///
/// Bash doesn't always track line numbers accurately for all command types.
/// When the line number is 0 or appears to be garbage (uninitialized memory),
/// we return None to indicate it's unknown.
///
/// We consider line numbers > 1 million to be garbage since no reasonable
/// script would have that many lines.
const fn line_or_none(line: u32) -> Option<u32> {
    // Filter out 0 (unknown) and garbage values (uninitialized memory on Linux)
    if line == 0 || line > 1_000_000 {
        None
    } else {
        Some(line)
    }
}

/// Get the effective line number, preferring the command-specific line if non-zero
///
/// Many bash command structures have their own line field that may be more
/// accurate than the parent COMMAND's line. This helper picks the best one.
const fn effective_line(cmd_line: i32, fallback: u32) -> u32 {
    if cmd_line > 0 {
        cmd_line as u32
    } else {
        fallback
    }
}

/// Flatten nested pipelines into a single vector
fn flatten_pipeline(cmd: &Command, commands: &mut Vec<Command>) {
    if let Command::Pipeline {
        commands: inner, ..
    } = cmd
    {
        for c in inner {
            flatten_pipeline(c, commands);
        }
    } else {
        commands.push(cmd.clone());
    }
}

// The actual conversion implementation
mod convert_impl {
    use super::{
        convert_redirects, convert_word_list, convert_word_list_to_strings, cstr_to_string,
        effective_line, flatten_pipeline, line_or_none, CASEPAT_FALLTHROUGH, CASEPAT_TESTNEXT,
        CMD_INVERT_RETURN, COND_AND, COND_BINARY, COND_EXPR, COND_OR, COND_TERM, COND_UNARY,
        MAX_DEPTH, MAX_LIST_LENGTH, W_ASSIGNMENT,
    };
    use crate::ast::{CaseClause, CaseClauseFlags, Command, ConditionalExpr, ListOp};
    use crate::ffi;

    /// Convert a C COMMAND pointer to a Rust Command
    ///
    /// # Safety
    ///
    /// The pointer must be valid and non-null, pointing to a valid
    /// COMMAND structure allocated by bash's parser.
    pub unsafe fn convert_command(cmd: *const ffi::COMMAND) -> Option<Command> {
        convert_command_with_depth(cmd, 0)
    }

    /// Internal function with depth tracking to prevent stack overflow
    pub(super) unsafe fn convert_command_with_depth(
        cmd: *const ffi::COMMAND,
        depth: usize,
    ) -> Option<Command> {
        if depth > MAX_DEPTH {
            return None; // Prevent stack overflow from deeply nested scripts
        }

        if cmd.is_null() {
            return None;
        }

        let cmd = &*cmd;
        let line = cmd.line as u32;

        // Check if this command is negated (for pipelines)
        let negated = (cmd.flags & CMD_INVERT_RETURN) != 0;

        match cmd.type_ {
            ffi::command_type_cm_simple => convert_simple(cmd, line),
            ffi::command_type_cm_connection => convert_connection(cmd, line, negated, depth),
            ffi::command_type_cm_for => convert_for(cmd, line, depth),
            ffi::command_type_cm_while => convert_while(cmd, line, depth),
            ffi::command_type_cm_until => convert_until(cmd, line, depth),
            ffi::command_type_cm_if => convert_if(cmd, line, depth),
            ffi::command_type_cm_case => convert_case(cmd, line, depth),
            ffi::command_type_cm_select => convert_select(cmd, line, depth),
            ffi::command_type_cm_group => convert_group(cmd, line, depth),
            ffi::command_type_cm_subshell => convert_subshell(cmd, line, depth),
            ffi::command_type_cm_function_def => convert_function_def(cmd, line, depth),
            ffi::command_type_cm_arith => convert_arith(cmd, line),
            ffi::command_type_cm_arith_for => convert_arith_for(cmd, line, depth),
            ffi::command_type_cm_cond => convert_cond(cmd, line, depth),
            ffi::command_type_cm_coproc => convert_coproc(cmd, line, depth),
            _ => None,
        }
    }

    #[allow(clippy::unnecessary_wraps)] // Consistent with other converters that may return None
    unsafe fn convert_simple(cmd: &ffi::COMMAND, line: u32) -> Option<Command> {
        let simple = &*cmd.value.Simple;
        let eff_line = effective_line(simple.line, line);
        let words = convert_word_list(simple.words);

        // Also get redirects from both the simple command and the parent command
        let mut redirects = convert_redirects(simple.redirects);
        redirects.extend(convert_redirects(cmd.redirects));

        // Separate assignments from words
        let (assignments, command_words): (Vec<_>, Vec<_>) = words
            .into_iter()
            .partition(|w| (w.flags & W_ASSIGNMENT) != 0);

        let assignments = if assignments.is_empty() {
            None
        } else {
            Some(assignments.into_iter().map(|w| w.word).collect())
        };

        Some(Command::Simple {
            line: line_or_none(eff_line),
            words: command_words,
            redirects,
            assignments,
        })
    }

    unsafe fn convert_connection(
        cmd: &ffi::COMMAND,
        _line: u32,
        negated: bool,
        depth: usize,
    ) -> Option<Command> {
        let conn = &*cmd.value.Connection;

        // Connector determines the type of connection
        // '|' for pipeline, '&&' for and, '||' for or, ';' for semi, '&' for async
        let connector = conn.connector as u8 as char;

        if connector == '|' {
            // Pipeline - collect all commands in the pipeline
            let mut commands = Vec::new();

            let first = convert_command_with_depth(conn.first, depth + 1)?;
            let second = convert_command_with_depth(conn.second, depth + 1)?;

            // Flatten nested pipelines
            flatten_pipeline(&first, &mut commands);
            flatten_pipeline(&second, &mut commands);

            Some(Command::Pipeline {
                // Don't include line for pipelines - the COMMAND.line field
                // for CONNECTION types is unreliable (uninitialized on some
                // platforms). Child commands have their own accurate line numbers.
                line: None,
                commands,
                negated,
            })
        } else {
            // List connection
            let op = match connector {
                '&' => {
                    // Check if this is '&&' or just '&'
                    // connector == '&' && next char is '&' means AND
                    // We need to check the actual connector value
                    if conn.connector == ('&' as i32) << 8 | ('&' as i32) || conn.connector == 288
                    // AND_AND token
                    {
                        ListOp::And
                    } else {
                        ListOp::Amp
                    }
                }
                '|' => ListOp::Or, // This is actually OR_OR
                ';' => ListOp::Semi,
                '\n' => ListOp::Newline,
                _ => {
                    // Check token values
                    if conn.connector == 289 {
                        // OR_OR token
                        ListOp::Or
                    } else if conn.connector == 288 {
                        // AND_AND token
                        ListOp::And
                    } else {
                        ListOp::Semi
                    }
                }
            };

            let left = convert_command_with_depth(conn.first, depth + 1)?;

            // For background commands (cmd &), the second command may be null
            let right = convert_command_with_depth(conn.second, depth + 1);

            match right {
                Some(right_cmd) => Some(Command::List {
                    // Don't include line for list commands - it's unreliable
                    // (uninitialized on some platforms) and the child commands
                    // have their own accurate line numbers
                    line: None,
                    op,
                    left: Box::new(left),
                    right: Box::new(right_cmd),
                }),
                None if op == ListOp::Amp => {
                    // Background command with no following command - wrap in a list
                    // with an empty/noop right side isn't ideal. Instead, we'll
                    // mark the left command as backgrounded by returning it as
                    // a single-element list
                    Some(Command::List {
                        line: None,
                        op: ListOp::Amp,
                        left: Box::new(left),
                        right: Box::new(Command::Simple {
                            line: None,
                            words: vec![],
                            redirects: vec![],
                            assignments: None,
                        }),
                    })
                }
                None => None, // Other cases require a second command
            }
        }
    }

    unsafe fn convert_for(cmd: &ffi::COMMAND, line: u32, depth: usize) -> Option<Command> {
        let for_cmd = &*cmd.value.For;
        let eff_line = effective_line(for_cmd.line, line);
        let variable = cstr_to_string((*for_cmd.name).word);
        let words = convert_word_list_to_strings(for_cmd.map_list);
        let body = convert_command_with_depth(for_cmd.action, depth + 1)?;

        Some(Command::For {
            line: line_or_none(eff_line),
            variable,
            words,
            body: Box::new(body),
        })
    }

    unsafe fn convert_while(cmd: &ffi::COMMAND, line: u32, depth: usize) -> Option<Command> {
        let while_cmd = &*cmd.value.While;

        let test = convert_command_with_depth(while_cmd.test, depth + 1)?;
        let body = convert_command_with_depth(while_cmd.action, depth + 1)?;

        Some(Command::While {
            line: line_or_none(line),
            test: Box::new(test),
            body: Box::new(body),
        })
    }

    unsafe fn convert_until(cmd: &ffi::COMMAND, line: u32, depth: usize) -> Option<Command> {
        // Until uses the same structure as while
        let while_cmd = &*cmd.value.While;

        let test = convert_command_with_depth(while_cmd.test, depth + 1)?;
        let body = convert_command_with_depth(while_cmd.action, depth + 1)?;

        Some(Command::Until {
            line: line_or_none(line),
            test: Box::new(test),
            body: Box::new(body),
        })
    }

    unsafe fn convert_if(cmd: &ffi::COMMAND, line: u32, depth: usize) -> Option<Command> {
        let if_cmd = &*cmd.value.If;

        let condition = convert_command_with_depth(if_cmd.test, depth + 1)?;
        let then_branch = convert_command_with_depth(if_cmd.true_case, depth + 1)?;
        let else_branch = if if_cmd.false_case.is_null() {
            None
        } else {
            Some(Box::new(convert_command_with_depth(
                if_cmd.false_case,
                depth + 1,
            )?))
        };

        Some(Command::If {
            line: line_or_none(line),
            condition: Box::new(condition),
            then_branch: Box::new(then_branch),
            else_branch,
        })
    }

    #[allow(clippy::unnecessary_wraps)] // Consistent with other converters that may return None
    unsafe fn convert_case(cmd: &ffi::COMMAND, line: u32, depth: usize) -> Option<Command> {
        let case_cmd = &*cmd.value.Case;
        let eff_line = effective_line(case_cmd.line, line);
        let word = cstr_to_string((*case_cmd.word).word);
        let clauses = convert_pattern_list(case_cmd.clauses, depth);

        Some(Command::Case {
            line: line_or_none(eff_line),
            word,
            clauses,
        })
    }

    unsafe fn convert_pattern_list(list: *mut ffi::PATTERN_LIST, depth: usize) -> Vec<CaseClause> {
        let mut clauses = Vec::new();
        let mut current = list;
        let mut count = 0;

        while !current.is_null() {
            count += 1;
            if count > MAX_LIST_LENGTH {
                break; // Prevent infinite loop from cyclic list
            }

            let pattern = &*current;

            let patterns = convert_word_list_to_strings(pattern.patterns).unwrap_or_default();
            let action = if pattern.action.is_null() {
                None
            } else {
                convert_command_with_depth(pattern.action, depth + 1).map(Box::new)
            };

            let flags = if pattern.flags != 0 {
                Some(CaseClauseFlags {
                    fallthrough: (pattern.flags & CASEPAT_FALLTHROUGH) != 0,
                    test_next: (pattern.flags & CASEPAT_TESTNEXT) != 0,
                })
            } else {
                None
            };

            clauses.push(CaseClause {
                patterns,
                action,
                flags,
            });

            current = pattern.next;
        }

        clauses
    }

    unsafe fn convert_select(cmd: &ffi::COMMAND, line: u32, depth: usize) -> Option<Command> {
        let select_cmd = &*cmd.value.Select;
        let eff_line = effective_line(select_cmd.line, line);
        let variable = cstr_to_string((*select_cmd.name).word);
        let words = convert_word_list_to_strings(select_cmd.map_list);
        let body = convert_command_with_depth(select_cmd.action, depth + 1)?;

        Some(Command::Select {
            line: line_or_none(eff_line),
            variable,
            words,
            body: Box::new(body),
        })
    }

    unsafe fn convert_group(cmd: &ffi::COMMAND, line: u32, depth: usize) -> Option<Command> {
        let group_cmd = &*cmd.value.Group;

        let body = convert_command_with_depth(group_cmd.command, depth + 1)?;

        Some(Command::Group {
            line: line_or_none(line),
            body: Box::new(body),
        })
    }

    unsafe fn convert_subshell(cmd: &ffi::COMMAND, line: u32, depth: usize) -> Option<Command> {
        let subshell_cmd = &*cmd.value.Subshell;
        let eff_line = effective_line(subshell_cmd.line, line);
        let body = convert_command_with_depth(subshell_cmd.command, depth + 1)?;

        Some(Command::Subshell {
            line: line_or_none(eff_line),
            body: Box::new(body),
        })
    }

    unsafe fn convert_function_def(cmd: &ffi::COMMAND, line: u32, depth: usize) -> Option<Command> {
        let func_def = &*cmd.value.Function_def;

        let name = cstr_to_string((*func_def.name).word);
        let body = convert_command_with_depth(func_def.command, depth + 1)?;
        let source_file = if func_def.source_file.is_null() {
            None
        } else {
            Some(cstr_to_string(func_def.source_file))
        };

        Some(Command::FunctionDef {
            line: line_or_none(line),
            name,
            body: Box::new(body),
            source_file,
        })
    }

    #[allow(clippy::unnecessary_wraps)] // Consistent with other converters that may return None
    unsafe fn convert_arith(cmd: &ffi::COMMAND, line: u32) -> Option<Command> {
        let arith_cmd = &*cmd.value.Arith;
        let eff_line = effective_line(arith_cmd.line, line);

        // The expression is in the first word of the word list
        let expression = if arith_cmd.exp.is_null() {
            String::new()
        } else {
            let word_list = convert_word_list(arith_cmd.exp);
            word_list
                .into_iter()
                .map(|w| w.word)
                .collect::<Vec<_>>()
                .join(" ")
        };

        Some(Command::Arithmetic {
            line: line_or_none(eff_line),
            expression,
        })
    }

    unsafe fn convert_arith_for(cmd: &ffi::COMMAND, line: u32, depth: usize) -> Option<Command> {
        let arith_for = &*cmd.value.ArithFor;
        let eff_line = effective_line(arith_for.line, line);
        let init = words_to_string(arith_for.init);
        let test = words_to_string(arith_for.test);
        let step = words_to_string(arith_for.step);
        let body = convert_command_with_depth(arith_for.action, depth + 1)?;

        Some(Command::ArithmeticFor {
            line: line_or_none(eff_line),
            init,
            test,
            step,
            body: Box::new(body),
        })
    }

    unsafe fn convert_cond(cmd: &ffi::COMMAND, line: u32, depth: usize) -> Option<Command> {
        let cond_cmd = &*cmd.value.Cond;
        let eff_line = effective_line(cond_cmd.line, line);
        let expr = convert_cond_com(cond_cmd, depth)?;

        Some(Command::Conditional {
            line: line_or_none(eff_line),
            expr,
        })
    }

    unsafe fn convert_cond_com(
        cond: *const ffi::COND_COM,
        depth: usize,
    ) -> Option<ConditionalExpr> {
        if depth > MAX_DEPTH {
            return None; // Prevent stack overflow from deeply nested conditionals
        }

        if cond.is_null() {
            return None;
        }

        let cond = &*cond;

        match cond.type_ {
            t if t == COND_AND => {
                let left = convert_cond_com(cond.left, depth + 1)?;
                let right = convert_cond_com(cond.right, depth + 1)?;
                Some(ConditionalExpr::And {
                    left: Box::new(left),
                    right: Box::new(right),
                })
            }
            t if t == COND_OR => {
                let left = convert_cond_com(cond.left, depth + 1)?;
                let right = convert_cond_com(cond.right, depth + 1)?;
                Some(ConditionalExpr::Or {
                    left: Box::new(left),
                    right: Box::new(right),
                })
            }
            t if t == COND_UNARY => {
                let op = if cond.op.is_null() {
                    String::new()
                } else {
                    cstr_to_string((*cond.op).word)
                };
                let arg = if cond.left.is_null() {
                    String::new()
                } else {
                    // For unary, the argument is in left->op
                    let left = &*cond.left;
                    if left.op.is_null() {
                        String::new()
                    } else {
                        cstr_to_string((*left.op).word)
                    }
                };
                Some(ConditionalExpr::Unary { op, arg })
            }
            t if t == COND_BINARY => {
                let op = if cond.op.is_null() {
                    String::new()
                } else {
                    cstr_to_string((*cond.op).word)
                };
                let left = if cond.left.is_null() {
                    String::new()
                } else {
                    let left_cond = &*cond.left;
                    if left_cond.op.is_null() {
                        String::new()
                    } else {
                        cstr_to_string((*left_cond.op).word)
                    }
                };
                let right = if cond.right.is_null() {
                    String::new()
                } else {
                    let right_cond = &*cond.right;
                    if right_cond.op.is_null() {
                        String::new()
                    } else {
                        cstr_to_string((*right_cond.op).word)
                    }
                };
                Some(ConditionalExpr::Binary { op, left, right })
            }
            t if t == COND_TERM => {
                let word = if cond.op.is_null() {
                    String::new()
                } else {
                    cstr_to_string((*cond.op).word)
                };
                Some(ConditionalExpr::Term { word })
            }
            t if t == COND_EXPR => {
                let expr = convert_cond_com(cond.left, depth + 1)?;
                Some(ConditionalExpr::Expr {
                    expr: Box::new(expr),
                })
            }
            _ => None,
        }
    }

    unsafe fn convert_coproc(cmd: &ffi::COMMAND, line: u32, depth: usize) -> Option<Command> {
        let coproc_cmd = &*cmd.value.Coproc;

        let name = if coproc_cmd.name.is_null() {
            None
        } else {
            Some(cstr_to_string(coproc_cmd.name))
        };
        let body = convert_command_with_depth(coproc_cmd.command, depth + 1)?;

        Some(Command::Coproc {
            line: line_or_none(line),
            name,
            body: Box::new(body),
        })
    }

    // Helper to convert word list to string
    unsafe fn words_to_string(list: *mut ffi::WORD_LIST) -> String {
        convert_word_list_to_strings(list)
            .map(|v| v.join(" "))
            .unwrap_or_default()
    }
}
