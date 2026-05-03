#!/usr/bin/env bash
set -euo pipefail

python3 - <<'PY'
import importlib.util
import sys
from pathlib import Path

module_path = Path('.github/scripts/render-adversarial-review-summary.py')
spec = importlib.util.spec_from_file_location('review_summary', module_path)
if spec is None or spec.loader is None:
    print(f'::error::Could not load review summary parser from {module_path}')
    sys.exit(1)

review_summary = importlib.util.module_from_spec(spec)
spec.loader.exec_module(review_summary)

response_path = Path('review-artifacts/agent-response.md')
response = response_path.read_text(encoding='utf-8', errors='replace') if response_path.exists() else ''
review, warning = review_summary.extract_json_blob(response)
if review is None:
    print(f'::error::Adversarial review did not produce a valid structured recommendation: {warning}')
    sys.exit(1)

validation_errors = review_summary.validate_review(review)
if validation_errors:
    for error in validation_errors:
        print(f'::error::Invalid adversarial review result: {error}')
    sys.exit(1)

recommendation = str(review.get('recommendation', '')).strip()
print(f'Adversarial review recommendation: {recommendation}')
if recommendation != 'PASS':
    print(f'::error::Adversarial review recommendation is {recommendation}; failing the check so the PR is not mergeable as-is.')
    sys.exit(1)
PY
