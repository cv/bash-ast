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

## How it works

When a new release is tagged (e.g., `v0.1.0`), the GitHub Actions workflow automatically:

1. Builds binaries for Linux and macOS (x86_64 and ARM64)
2. Creates a GitHub release with the binaries
3. Updates the formula in [cv/homebrew-taps](https://github.com/cv/homebrew-taps)

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
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0
```

The workflow will automatically:
- Build and test on all platforms
- Create a GitHub release with binaries
- Update the Homebrew formula in cv/homebrew-taps

### Manual formula update (if needed)

If you need to manually update the formula:

```bash
# Get the commit SHA for the tag
git rev-parse v0.1.0

# Update Formula/bash-ast.rb with the tag and SHA
# Then copy to homebrew-taps repo
```
