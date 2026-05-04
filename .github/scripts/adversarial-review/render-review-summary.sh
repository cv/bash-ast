#!/usr/bin/env bash
set -euxo pipefail

python3 .github/scripts/render-adversarial-review-summary.py \
  --response review-artifacts/agent-response.md \
  --build-log review-artifacts/build.log \
  --baseline-log review-artifacts/baseline-tests.log \
  --build-status review-artifacts/build-status.txt \
  --baseline-status review-artifacts/baseline-test-status.txt \
  --output review-artifacts/adversarial-review-summary.md
cat review-artifacts/adversarial-review-summary.md >> "$GITHUB_STEP_SUMMARY"
