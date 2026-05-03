#!/usr/bin/env bash
set -euxo pipefail

mkdir -p review-artifacts/agent-probes
git status --short > review-artifacts/git-status.txt
git log --oneline -n 20 > review-artifacts/recent-commits.txt
cargo metadata --no-deps --format-version 1 > review-artifacts/cargo-metadata.json || true

git fetch origin "$BASE_REF" --depth=1 || true
if git rev-parse --verify "origin/$BASE_REF" >/dev/null 2>&1; then
  git diff --stat "origin/$BASE_REF...HEAD" > review-artifacts/base-diff.stat || true
  git diff --find-renames "origin/$BASE_REF...HEAD" > review-artifacts/base-diff.patch || true
else
  : > review-artifacts/base-diff.stat
  : > review-artifacts/base-diff.patch
fi

if [ -n "$PR_NUMBER" ]; then
  gh pr view "$PR_NUMBER" \
    --json number,title,author,body,baseRefName,headRefName,headRefOid,url,files,comments \
    > review-artifacts/pr-context.json || echo '{}' > review-artifacts/pr-context.json
  gh pr diff "$PR_NUMBER" > review-artifacts/pr.diff || true
else
  echo '{}' > review-artifacts/pr-context.json
  : > review-artifacts/pr.diff
fi
