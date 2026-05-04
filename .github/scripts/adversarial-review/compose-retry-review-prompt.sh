#!/usr/bin/env bash
set -euo pipefail

delimiter="RETRY_REVIEW_PROMPT_$(date +%s)_$$"
{
  cat review-artifacts/review-prompt.md
  echo
  echo "## Retry instruction"
  echo
  echo "The previous adversarial-review attempt did not produce a valid structured result. Reason: ${RETRY_REASON:-unknown}."
  echo "Retry the review now. You must end with exact line markers JSON_RESULT_START and JSON_RESULT_END, with one valid JSON object between them and no nested marker text inside JSON strings. The JSON must satisfy the required schema, including a non-empty tests array."
} > review-artifacts/review-prompt-retry.md
{
  echo "prompt<<$delimiter"
  cat review-artifacts/review-prompt-retry.md
  echo "$delimiter"
} >> "$GITHUB_OUTPUT"
