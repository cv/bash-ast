//! Bash initialization for parsing
//!
//! This module handles initializing the bash parser state required
//! before parsing can occur.

use std::sync::Once;

static INIT: Once = Once::new();

// External globals from bash - we declare them here since bindgen
// may not export them correctly
extern "C" {
    static mut interactive: i32;
    static mut interactive_shell: i32;
    static mut login_shell: i32;
    static mut posixly_correct: i32;
    static mut shell_initialized: i32;
    static mut startup_state: i32;
    static mut parsing_command: i32;
}

/// Initialize bash internals for parsing
///
/// This must be called once before any parsing operations.
/// It is safe to call multiple times - subsequent calls are no-ops.
///
/// # Safety
///
/// This function modifies global state in the bash library.
/// It should be called from the main thread before any multi-threaded
/// parsing attempts.
pub fn init() {
    INIT.call_once(|| unsafe {
        init_bash_globals();
    });
}

/// Initialize the global variables bash needs for parsing
///
/// # Safety
///
/// Modifies global C state. Must only be called once.
unsafe fn init_bash_globals() {
    // These globals control bash's behavior
    // We set them to disable interactive features and enable
    // pure parsing mode

    // Disable interactive mode
    interactive = 0;
    interactive_shell = 0;

    // Not a login shell
    login_shell = 0;

    // Disable POSIX mode (allows more bash extensions)
    posixly_correct = 0;

    // Mark as initialized
    shell_initialized = 1;

    // Set startup state to indicate we're ready
    startup_state = 0;

    // Not currently parsing a command
    parsing_command = 0;
}
