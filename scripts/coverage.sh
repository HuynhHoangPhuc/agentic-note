#!/usr/bin/env bash
set -euo pipefail

cargo llvm-cov --workspace --html --output-dir target/llvm-cov

report_path="$(pwd)/target/llvm-cov/index.html"
if command -v open >/dev/null 2>&1; then
  open "$report_path"
else
  echo "Coverage report generated at $report_path"
fi
