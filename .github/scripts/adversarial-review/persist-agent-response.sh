#!/usr/bin/env bash
set -euo pipefail

mkdir -p review-artifacts
python3 - <<'PY'
import json
import os
from pathlib import Path

use_retry = os.environ.get('INITIAL_VALID') != 'true'
response = os.environ.get('RETRY_RESPONSE' if use_retry else 'INITIAL_RESPONSE', '')
success = os.environ.get('RETRY_SUCCESS' if use_retry else 'INITIAL_SUCCESS', '')
share_url = os.environ.get('RETRY_SHARE_URL' if use_retry else 'INITIAL_SHARE_URL', '')
Path('review-artifacts/agent-response.md').write_text(response, encoding='utf-8')
if use_retry:
    Path('review-artifacts/agent-response-retry.md').write_text(os.environ.get('RETRY_RESPONSE', ''), encoding='utf-8')
Path('review-artifacts/agent-action-metadata.json').write_text(
    json.dumps({
        'selected_attempt': 'retry' if use_retry else 'initial',
        'initial_valid': os.environ.get('INITIAL_VALID', ''),
        'success': success,
        'share_url': share_url,
    }, indent=2) + '\n',
    encoding='utf-8',
)
PY
