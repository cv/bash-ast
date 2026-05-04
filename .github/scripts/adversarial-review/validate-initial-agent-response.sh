#!/usr/bin/env bash
set -euo pipefail

mkdir -p review-artifacts
python3 - <<'PY' >> "$GITHUB_OUTPUT"
import importlib.util
import os
from pathlib import Path

Path('review-artifacts/agent-response-initial.md').write_text(os.environ.get('AGENT_RESPONSE', ''), encoding='utf-8')
spec = importlib.util.spec_from_file_location('review_summary', '.github/scripts/render-adversarial-review-summary.py')
review_summary = importlib.util.module_from_spec(spec)
spec.loader.exec_module(review_summary)
review, warning = review_summary.extract_json_blob(os.environ.get('AGENT_RESPONSE', ''))
errors = []
if review is None:
    errors.append(warning or 'missing structured review result')
else:
    errors.extend(review_summary.validate_review(review))
if errors:
    print('valid=false')
    print(f"reason={' ; '.join(errors)}")
else:
    print('valid=true')
    print('reason=valid structured review result')
PY
