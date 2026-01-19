//! Helper functions for C to Rust AST conversion

use crate::ast::{Redirect, RedirectTarget, RedirectType, Word};
use crate::ffi;
use std::ffi::{c_char, CStr};

use super::MAX_LIST_LENGTH;

/// Convert a C string pointer to a Rust String
///
/// Returns an empty string if the pointer is null.
pub(super) unsafe fn cstr_to_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        CStr::from_ptr(ptr).to_string_lossy().into_owned()
    }
}

/// Convert a `WORD_LIST` linked list to a Vec of Words
pub(super) unsafe fn convert_word_list(list: *mut ffi::WORD_LIST) -> Vec<Word> {
    let mut words = Vec::new();
    let mut current = list;
    let mut count = 0;

    while !current.is_null() {
        count += 1;
        if count > MAX_LIST_LENGTH {
            break; // Prevent infinite loop from cyclic list
        }

        let word_desc = (*current).word;
        if !word_desc.is_null() {
            words.push(Word {
                word: cstr_to_string((*word_desc).word),
                flags: (*word_desc).flags as u32,
            });
        }
        current = (*current).next;
    }

    words
}

/// Convert a `WORD_LIST` to a Vec of Strings (word text only)
pub(super) unsafe fn convert_word_list_to_strings(
    list: *mut ffi::WORD_LIST,
) -> Option<Vec<String>> {
    if list.is_null() {
        return None;
    }

    let words = convert_word_list(list);
    if words.is_empty() {
        None
    } else {
        Some(words.into_iter().map(|w| w.word).collect())
    }
}

/// Convert a REDIRECT linked list to a Vec of Redirects
pub(super) unsafe fn convert_redirects(redirects: *mut ffi::REDIRECT) -> Vec<Redirect> {
    let mut result = Vec::new();
    let mut current = redirects;
    let mut count = 0;

    while !current.is_null() {
        count += 1;
        if count > MAX_LIST_LENGTH {
            break; // Prevent infinite loop from cyclic list
        }

        let redir = &*current;

        #[allow(clippy::match_same_arms)] // Explicit output match + default fallback
        let direction = match redir.instruction {
            ffi::r_instruction_r_output_direction => RedirectType::Output,
            ffi::r_instruction_r_input_direction | ffi::r_instruction_r_inputa_direction => {
                RedirectType::Input
            }
            ffi::r_instruction_r_appending_to => RedirectType::Append,
            ffi::r_instruction_r_reading_until | ffi::r_instruction_r_deblank_reading_until => {
                RedirectType::HereDoc
            }
            ffi::r_instruction_r_reading_string => RedirectType::HereString,
            ffi::r_instruction_r_duplicating_input
            | ffi::r_instruction_r_duplicating_input_word => RedirectType::DupInput,
            ffi::r_instruction_r_duplicating_output
            | ffi::r_instruction_r_duplicating_output_word => RedirectType::DupOutput,
            ffi::r_instruction_r_close_this => RedirectType::Close,
            ffi::r_instruction_r_err_and_out => RedirectType::ErrAndOut,
            ffi::r_instruction_r_input_output => RedirectType::InputOutput,
            ffi::r_instruction_r_output_force => RedirectType::Clobber,
            ffi::r_instruction_r_move_input | ffi::r_instruction_r_move_input_word => {
                RedirectType::MoveInput
            }
            ffi::r_instruction_r_move_output | ffi::r_instruction_r_move_output_word => {
                RedirectType::MoveOutput
            }
            ffi::r_instruction_r_append_err_and_out => RedirectType::AppendErrAndOut,
            _ => RedirectType::Output,
        };

        // Get source fd from redirector
        let source_fd = {
            let fd = redir.redirector.dest;
            if fd >= 0 {
                Some(fd)
            } else {
                None
            }
        };

        // Get target
        let target = {
            // For dup/close operations, the target is a fd number
            // For file operations, it's a filename
            match redir.instruction {
                ffi::r_instruction_r_duplicating_input
                | ffi::r_instruction_r_duplicating_output
                | ffi::r_instruction_r_move_input
                | ffi::r_instruction_r_move_output => RedirectTarget::Fd(redir.redirectee.dest),
                ffi::r_instruction_r_close_this => RedirectTarget::Fd(-1),
                _ => {
                    // File-based redirect
                    let filename = redir.redirectee.filename;
                    if filename.is_null() {
                        RedirectTarget::File(String::new())
                    } else {
                        RedirectTarget::File(cstr_to_string((*filename).word))
                    }
                }
            }
        };

        // Here-doc delimiter
        let here_doc_eof = if redir.here_doc_eof.is_null() {
            None
        } else {
            Some(cstr_to_string(redir.here_doc_eof))
        };

        result.push(Redirect {
            direction,
            source_fd,
            target,
            here_doc_eof,
        });

        current = redir.next;
    }

    result
}
