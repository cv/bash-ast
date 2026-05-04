#!/usr/bin/env bash
set +e

mkdir -p review-artifacts
cargo test -- --list 2>&1 | tee review-artifacts/baseline-test-list.txt
status=${PIPESTATUS[0]}
echo "$status" > review-artifacts/baseline-test-list-status.txt
python3 - <<'PY'
from pathlib import Path

text = Path('review-artifacts/baseline-test-list.txt').read_text(encoding='utf-8', errors='replace')
tests = sorted({line.strip()[:-len(': test')] for line in text.splitlines() if line.strip().endswith(': test')})
benches = sorted({line.strip()[:-len(': benchmark')] for line in text.splitlines() if line.strip().endswith(': benchmark')})
status = Path('review-artifacts/baseline-test-list-status.txt').read_text(encoding='utf-8').strip()
lines = [
    '# Automated tests already enumerated',
    '',
    f'- Test inventory exit code: `{status}`',
    f'- Enumerated test count: `{len(tests)}`',
    f'- Enumerated benchmark count: `{len(benches)}`',
    '',
    'The baseline workflow already ran `cargo test --verbose -- --test-threads=1` before this inventory was collected.',
    'Use this inventory to avoid duplicating existing automated coverage in adversarial probes and recommendations.',
    '',
    '## Test names',
]
max_names = 250
lines.extend(f'- `{name}`' for name in tests[:max_names])
if len(tests) > max_names:
    lines.append(f'- ... truncated {len(tests) - max_names} additional tests; see `baseline-test-list.txt` for the full list.')
if benches:
    lines.extend(['', '## Benchmark names'])
    lines.extend(f'- `{name}`' for name in benches[:50])
Path('review-artifacts/baseline-test-inventory.md').write_text('\n'.join(lines) + '\n', encoding='utf-8')
PY
exit 0
