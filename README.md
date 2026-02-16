# bash-ast

Parse bash scripts to JSON AST using GNU Bash's actual parser, and convert AST back to bash.

## Overview

`bash-ast` is a Rust tool that uses FFI bindings to GNU Bash's parser to convert bash scripts into JSON AST output, and can also convert AST back to executable bash code. This provides 100% compatibility with bash syntax since it uses bash's own parser.

## Features

- **100% bash compatibility**: Uses the actual GNU Bash parser via FFI
- **JSON output**: Serializes the AST to JSON for easy consumption
- **Round-trip support**: Convert AST back to bash with `--to-bash`
- **Server mode**: Low-latency Unix socket server for editor/tool integration
- **All bash constructs**: Supports all 16 bash command types including:
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

### Homebrew (macOS)

```bash
# Install from tap
brew tap cv/taps
brew install bash-ast

# Or install HEAD version directly
brew install --HEAD https://raw.githubusercontent.com/cv/bash-ast/main/Formula/bash-ast.rb
```

### Debian / Ubuntu

Download the `.deb` package from the [releases page](https://github.com/cv/bash-ast/releases):

```bash
# Download (replace VERSION with actual version)
curl -LO https://github.com/cv/bash-ast/releases/download/vVERSION/bash-ast_VERSION-1_amd64.deb

# Install
sudo dpkg -i bash-ast_VERSION-1_amd64.deb
```

### RedHat / Fedora / CentOS

Download the `.rpm` package from the [releases page](https://github.com/cv/bash-ast/releases):

```bash
# Download (replace VERSION with actual version)
curl -LO https://github.com/cv/bash-ast/releases/download/vVERSION/bash-ast-VERSION-1.x86_64.rpm

# Install
sudo rpm -i bash-ast-VERSION-1.x86_64.rpm

# Or with dnf (Fedora)
sudo dnf install ./bash-ast-VERSION-1.x86_64.rpm
```

### From source

#### Prerequisites

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
# Parse a script file to JSON AST
./target/release/bash-ast < script.sh

# Parse inline
echo 'for i in a b c; do echo $i; done' | ./target/release/bash-ast

# Pretty print with jq
./target/release/bash-ast < script.sh | jq .

# Convert JSON AST back to bash
./target/release/bash-ast script.sh | ./target/release/bash-ast --to-bash

# Round-trip: parse and regenerate
echo 'for i in a b c; do echo $i; done' | ./target/release/bash-ast | ./target/release/bash-ast -b
```

### Server Mode

For low-latency integration with editors and tools, run as a Unix socket server:

```bash
# Start server (default: $XDG_RUNTIME_DIR/bash-ast.sock or /tmp/bash-ast.sock)
bash-ast --server

# Or specify a custom socket path
bash-ast --server /tmp/my-parser.sock
```

Send newline-delimited JSON requests:

```bash
# Parse bash to AST
echo '{"method":"parse","script":"echo hello"}' | nc -U /tmp/bash-ast.sock
# → {"result":{"type":"simple","words":[{"word":"echo"},{"word":"hello"}],...}}

# Convert AST back to bash
echo '{"method":"to_bash","ast":{"type":"simple","words":[{"word":"echo"}],"redirects":[]}}' | nc -U /tmp/bash-ast.sock
# → {"result":"echo"}

# Other methods: schema, ping, shutdown
```

### Library

```rust
use bash_ast::{init, parse, to_bash, Command};

fn main() {
    // Initialize bash (call once at startup)
    init();

    // Parse a script
    let cmd = parse("echo hello world").unwrap();

    // Work with the AST
    if let Command::Simple { words, .. } = &cmd {
        for word in words {
            println!("Word: {}", word.word);
        }
    }

    // Convert AST back to bash
    let script = to_bash(&cmd);
    println!("Regenerated: {}", script);

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
          { "word": "$i", "flags": 1 }
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

Note: The `flags` field on words indicates special expansion handling (e.g., `flags: 1` means the word contains a variable expansion).

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
│   stdin (JSON)   ──► serde parse  ──► AST ──► bash stdout  │
│                                                             │
│   - bindgen-generated FFI bindings to bash                  │
│   - Calls parse_string_to_command() via FFI                 │
│   - Walks C AST, converts to Rust types                     │
│   - Serializes to JSON with serde                           │
│   - Regenerates bash from AST with to_bash()                │
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
