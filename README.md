# bash-ast

Parse bash scripts to JSON AST using GNU Bash's actual parser.

## Overview

`bash-ast` is a Rust tool that uses FFI bindings to GNU Bash's parser to convert bash scripts into JSON AST output. This provides 100% compatibility with bash syntax since it uses bash's own parser.

## Features

- **100% bash compatibility**: Uses the actual GNU Bash parser via FFI
- **JSON output**: Serializes the AST to JSON for easy consumption
- **All bash constructs**: Supports all 15 bash command types including:
  - Simple commands (`cmd arg1 arg2`)
  - Pipelines (`cmd1 | cmd2`)
  - Lists (`cmd1 && cmd2`, `cmd1 || cmd2`, `cmd1 ; cmd2`, `cmd1 &`)
  - For loops (`for var in list; do ...; done`)
  - While/Until loops
  - If statements
  - Case statements
  - Select statements
  - Group commands (`{ ...; }`)
  - Subshells (`( ... )`)
  - Function definitions
  - Arithmetic evaluation (`(( expr ))`)
  - C-style for loops (`for ((i=0; i<n; i++))`)
  - Conditional expressions (`[[ expr ]]`)
  - Coprocesses

## Installation

### Prerequisites

- Rust 1.70 or later
- LLVM/Clang (for bindgen)
- A C compiler (gcc or clang)
- ncurses development libraries

**Installing Rust:**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

**On macOS:**
```bash
# Install Xcode command line tools (includes clang)
xcode-select --install

# Install LLVM for bindgen
brew install llvm

# Set LLVM paths for bindgen
export LLVM_CONFIG_PATH="$(brew --prefix llvm)/bin/llvm-config"
```

**On Ubuntu/Debian:**
```bash
sudo apt-get install llvm-dev libclang-dev clang libncurses-dev build-essential
```

### Building

```bash
# Clone the repository
git clone <repository-url>
cd bash-ast

# Initialize the bash submodule
git submodule update --init

# Build the project
cargo build --release
```

## Usage

### CLI

```bash
# Parse a script file
./target/release/bash-ast < script.sh

# Parse inline
echo 'for i in a b c; do echo $i; done' | ./target/release/bash-ast

# Pretty print with jq
./target/release/bash-ast < script.sh | jq .
```

### Library

```rust
use bash_ast::{init, parse, Command};

fn main() {
    // Initialize bash (call once at startup)
    init();

    // Parse a script
    let cmd = parse("echo hello world").unwrap();

    // Work with the AST
    if let Command::Simple { words, .. } = cmd {
        for word in words {
            println!("Word: {}", word.word);
        }
    }

    // Or get JSON directly
    let json = bash_ast::parse_to_json("echo hello", true).unwrap();
    println!("{}", json);
}
```

## JSON Output Example

Input:
```bash
for i in a b c; do
    echo $i
done | grep a
```

Output:
```json
{
  "type": "pipeline",
  "line": 1,
  "commands": [
    {
      "type": "for",
      "line": 1,
      "variable": "i",
      "words": ["a", "b", "c"],
      "body": {
        "type": "simple",
        "line": 2,
        "words": [
          { "word": "echo" },
          { "word": "$i" }
        ],
        "redirects": []
      }
    },
    {
      "type": "simple",
      "line": 3,
      "words": [
        { "word": "grep" },
        { "word": "a" }
      ],
      "redirects": []
    }
  ]
}
```

## Error Handling

Syntax errors are handled gracefully and return `ParseError::SyntaxError`:

```bash
# Invalid syntax returns an error (no crash)
$ echo 'if then fi' | bash-ast
Error: Syntax error in script
```

For detailed error information (line numbers, tokens), use `bash -n` for pre-validation:

```bash
$ bash -n script.sh 2>&1
script.sh: line 1: syntax error near unexpected token `then'
```

## Thread Safety

**bash-ast is not thread-safe.** The underlying bash parser uses global state, so all parsing must be done from a single thread.

Tests are automatically configured to run single-threaded via `.cargo/config.toml`.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    bash-ast (Rust, GPL v3)                  │
│                                                             │
│   stdin (script) ──► FFI to bash ──► AST ──► JSON stdout   │
│                                                             │
│   - bindgen-generated FFI bindings to bash                  │
│   - Calls parse_string_to_command() via FFI                 │
│   - Walks C AST, converts to Rust types                     │
│   - Serializes to JSON with serde                           │
└─────────────────────────────────────────────────────────────┘
```

## License

This project is licensed under the GNU General Public License v3.0 (GPL-3.0) due to its linkage with GNU Bash.

See the [LICENSE](LICENSE) file for details.

## Development

### Testing

```bash
# Run all tests
cargo test

# Run property-based tests only
cargo test prop_

# Run with Makefile
make test        # Run all tests
make lint        # Run clippy and fmt check
make ci          # Full CI pipeline (lint + test)
```

### Coverage (Linux CI)

Coverage requires rustup-installed Rust:

```bash
rustup component add llvm-tools-preview
cargo llvm-cov --html --output-dir coverage
```

### Fuzzing (Linux CI)

Fuzz testing requires nightly Rust:

```bash
rustup install nightly
cargo +nightly fuzz run fuzz_parse -- -max_total_time=60
```

See [fuzz/README.md](fuzz/README.md) for details.

### Benchmarking

Run benchmarks with criterion:

```bash
cargo bench
# Or via Makefile
make bench
```

Results are saved to `target/criterion/report/index.html` with detailed HTML reports.

## Contributing

Contributions are welcome! Please ensure any contributions are compatible with the GPL-3.0 license.

## Acknowledgments

- GNU Bash project for the parser
- The Rust community for bindgen and serde
