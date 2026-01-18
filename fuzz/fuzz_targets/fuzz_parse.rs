#![no_main]

use libfuzzer_sys::fuzz_target;
use std::sync::Once;

static INIT: Once = Once::new();

fuzz_target!(|data: &[u8]| {
    // Initialize bash once (thread-safe via Once)
    INIT.call_once(|| {
        bash_ast::init();
    });

    // Try to parse the fuzzed input as a string
    if let Ok(script) = std::str::from_utf8(data) {
        // We don't care about the result - we're looking for crashes/panics
        let _ = bash_ast::parse(script);
    }
});
