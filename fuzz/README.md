# Fuzz Testing for bash-ast

This directory contains fuzz testing infrastructure using [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz).

## Requirements

- **Nightly Rust** (cargo-fuzz uses unstable features)
- **cargo-fuzz**: `cargo install cargo-fuzz`

### Installing Nightly via rustup

```bash
# Install rustup if not present
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install nightly toolchain
rustup install nightly
```

## Running the Fuzzer

```bash
# Run the parser fuzzer (runs indefinitely until Ctrl+C or crash)
cargo +nightly fuzz run fuzz_parse

# Run for a limited time (e.g., 60 seconds)
cargo +nightly fuzz run fuzz_parse -- -max_total_time=60

# Run with multiple jobs (careful: bash parser uses global state)
# Single-threaded is recommended for this target
cargo +nightly fuzz run fuzz_parse -- -jobs=1 -workers=1
```

## Corpus

The `corpus/fuzz_parse/` directory contains seed inputs that help the fuzzer
start with valid bash syntax. These are tracked in git.

To add new seeds:
```bash
echo 'your bash script' > fuzz/corpus/fuzz_parse/descriptive_name
git add fuzz/corpus/fuzz_parse/descriptive_name
```

## Crashes and Artifacts

When the fuzzer finds a crash:
- Crash inputs are saved to `artifacts/fuzz_parse/`
- Reproduce with: `cargo +nightly fuzz run fuzz_parse artifacts/fuzz_parse/crash-<hash>`

## Coverage

To see coverage information:
```bash
cargo +nightly fuzz coverage fuzz_parse
```

## Targets

| Target | Description |
|--------|-------------|
| `fuzz_parse` | Fuzzes `bash_ast::parse()` with arbitrary string input |

## Alternatives

### macOS / Stable Rust

Coverage-guided fuzzing (cargo-fuzz, honggfuzz) requires nightly Rust and has
limited macOS support. For local development, use **proptest** instead:

```bash
cargo test prop_  # Run property-based tests
```

Property-based tests are defined in `tests/integration.rs` and run automatically
with `cargo test`. They generate random inputs to find edge cases.

### CI Environment

For comprehensive fuzzing, run cargo-fuzz in a Linux CI environment with nightly Rust.
