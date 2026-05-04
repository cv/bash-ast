#!/usr/bin/env bash
set +e

mkdir -p review-artifacts
cargo build --verbose 2>&1 | tee review-artifacts/build.log
status=${PIPESTATUS[0]}
echo "$status" > review-artifacts/build-status.txt
exit "$status"
