# Installing bash-ast via Homebrew

## Installation

```bash
brew tap cv/taps
brew install bash-ast
```

Or install in one command:

```bash
brew install cv/taps/bash-ast
```

## Install HEAD (latest development version)

```bash
brew install --HEAD cv/taps/bash-ast
```

## Pre-built bottles

bash-ast provides pre-built bottles (binary packages) for:

- **macOS ARM64** (Apple Silicon): Sonoma, Sequoia
- **macOS x86_64** (Intel): Ventura
- **Linux x86_64**: Ubuntu/Debian-based systems

When you run `brew install bash-ast`, Homebrew will automatically download the pre-built bottle for your platform if available, making installation much faster (no compilation needed).

If no bottle is available for your platform, Homebrew will build from source (requires Rust and LLVM).

## How it works

When a new release is tagged (e.g., `v0.2.8`), the GitHub Actions workflow automatically:

1. Builds binaries for Linux and macOS (x86_64 and ARM64)
2. Builds Homebrew bottles for supported platforms
3. Creates a GitHub release with binaries and bottles
4. Updates the formula in [cv/homebrew-taps](https://github.com/cv/homebrew-taps) with bottle checksums

## Setup (for maintainers)

### Required secret

The release workflow needs a Personal Access Token to push to the homebrew-taps repo:

1. Go to GitHub → Settings → Developer settings → Personal access tokens → Fine-grained tokens
2. Create a new token with:
   - Repository access: Select `cv/homebrew-taps`
   - Permissions: Contents (Read and write)
3. Add the token as a secret in the bash-ast repo:
   - Go to bash-ast repo → Settings → Secrets and variables → Actions
   - Create a new secret named `HOMEBREW_TAP_TOKEN` with the token value

### Creating a release

```bash
# Tag the release
git tag -a v0.2.9 -m "Release v0.2.9"
git push origin v0.2.9
```

The workflow will automatically:
- Build and test on all platforms
- Build Homebrew bottles for macOS (ARM64 + x86_64) and Linux
- Create a GitHub release with binaries and bottles
- Update the Homebrew formula in cv/homebrew-taps with bottle SHA256 checksums

### Manual formula update (if needed)

If you need to manually update the formula:

```bash
# Get the commit SHA for the tag
git rev-parse v0.2.9

# Update Formula/bash-ast.rb with the tag and SHA
# Then copy to homebrew-taps repo
```

### Force rebuild from source

If you want to force building from source instead of using a bottle:

```bash
brew install --build-from-source cv/taps/bash-ast
```
