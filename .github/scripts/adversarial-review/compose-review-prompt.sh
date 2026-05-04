#!/usr/bin/env bash
set -euo pipefail

delimiter="REVIEW_PROMPT_$(date +%s)_$$"
{
  cat .github/prompts/adversarial-review.md
  echo
  echo "## Workflow-provided context"
  echo
  echo "- Repository: $GITHUB_REPOSITORY"
  echo "- Event: $GITHUB_EVENT_NAME"
  echo "- PR number: ${PR_NUMBER:-none}"
  echo "- Base ref: ${BASE_REF:-unknown}"
  echo "- Head SHA: ${HEAD_SHA:-unknown}"
  echo "- Build exit code: $(cat review-artifacts/build-status.txt 2>/dev/null || echo unknown)"
  echo "- Baseline test exit code: $(cat review-artifacts/baseline-test-status.txt 2>/dev/null || echo unknown)"
  echo "- Baseline test inventory exit code: $(cat review-artifacts/baseline-test-list-status.txt 2>/dev/null || echo unknown)"
  echo
  echo "Artifacts are available under ./review-artifacts/. Keep any additional probe artifacts under ./review-artifacts/agent-probes/."
  if [ -f review-artifacts/baseline-test-inventory.md ]; then
    echo
    echo "## Automated tests already run/enumerated"
    cat review-artifacts/baseline-test-inventory.md
  fi
} > review-artifacts/review-prompt.md
{
  echo "prompt<<$delimiter"
  cat review-artifacts/review-prompt.md
  echo "$delimiter"
} >> "$GITHUB_OUTPUT"
