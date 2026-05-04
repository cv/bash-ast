#!/usr/bin/env bash
set -euxo pipefail

gh pr checkout "$REQUESTED_PR"
git submodule update --init --recursive
echo "HEAD_SHA=$(git rev-parse HEAD)" >> "$GITHUB_ENV"
echo "BASE_REF=$(gh pr view "$REQUESTED_PR" --json baseRefName --jq .baseRefName)" >> "$GITHUB_ENV"
