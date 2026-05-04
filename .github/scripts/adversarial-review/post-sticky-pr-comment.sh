#!/usr/bin/env bash
set -euo pipefail

post_comment_input=$(jq -r '.inputs.post_comment // "false"' "$GITHUB_EVENT_PATH")
if [ "${ADVERSARIAL_REVIEW_POST_COMMENTS:-}" != "true" ] && [ "$post_comment_input" != "true" ]; then
  echo "Sticky PR comment disabled; set ADVERSARIAL_REVIEW_POST_COMMENTS=true or workflow_dispatch post_comment=true to enable."
  exit 0
fi

marker='<!-- adversarial-review:bash-ast -->'
body_file=$(mktemp)
{
  echo "$marker"
  echo "<!-- head_sha: ${HEAD_SHA:-unknown}; run_id: $GITHUB_RUN_ID; run_attempt: $GITHUB_RUN_ATTEMPT -->"
  echo
  cat review-artifacts/adversarial-review-summary.md
} > "$body_file"

comment_id=$(gh api "repos/$GITHUB_REPOSITORY/issues/$PR_NUMBER/comments" --paginate \
  --jq ".[] | select(.body | contains(\"$marker\")) | .id" | tail -n 1)

if [ -n "$comment_id" ]; then
  gh api -X PATCH "repos/$GITHUB_REPOSITORY/issues/comments/$comment_id" -F "body=@$body_file" >/dev/null
else
  gh pr comment "$PR_NUMBER" --body-file "$body_file"
fi
