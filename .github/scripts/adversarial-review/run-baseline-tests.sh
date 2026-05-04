#!/usr/bin/env bash
set +e

mkdir -p review-artifacts
cargo test --verbose -- --test-threads=1 2>&1 | tee review-artifacts/baseline-tests.log
status=${PIPESTATUS[0]}
echo "$status" > review-artifacts/baseline-test-status.txt
exit "$status"
