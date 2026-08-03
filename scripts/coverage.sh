#!/usr/bin/env bash
# M4: line coverage via cargo-llvm-cov.
# Usage:
#   ./scripts/coverage.sh           # pure modules (default, with fail-under)
#   ./scripts/coverage.sh pure      # same
#   ./scripts/coverage.sh full      # whole crate summary (no fail-under)
#   ./scripts/coverage.sh html      # HTML report for pure modules
#   ./scripts/coverage.sh lcov      # write lcov.info for CI upload
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MODE="${1:-pure}"

if ! command -v cargo-llvm-cov >/dev/null 2>&1 && ! cargo llvm-cov --version >/dev/null 2>&1; then
  echo "error: cargo-llvm-cov not found. Install with:" >&2
  echo "  cargo install cargo-llvm-cov --locked" >&2
  echo "  rustup component add llvm-tools-preview" >&2
  exit 127
fi

# Ignore OS/Docker/UI wiring when measuring pure-logic coverage.
# Ignore OS/Docker/UI/HTTP wiring; keep pure modules (metrics, alert, prometheus, util, serve/policy).
PURE_IGNORE='(main\.rs|cli\.rs|lib\.rs|error\.rs|display/|collector/|export/mod\.rs|serve/mod\.rs|serve/config\.rs)'

# Floor for pure modules (metrics, alert, prometheus, util, serve/policy).
# Baseline ~86% lines (2026-07); keep a modest margin for flaky platform diffs.
PURE_FAIL_UNDER_LINES="${PURE_FAIL_UNDER_LINES:-80}"

case "$MODE" in
  pure)
    echo "==> llvm-cov (pure: metrics, alert, prometheus, util, serve/policy) fail-under-lines=${PURE_FAIL_UNDER_LINES}"
    cargo llvm-cov --lib --tests --summary-only \
      --ignore-filename-regex "$PURE_IGNORE" \
      --fail-under-lines "$PURE_FAIL_UNDER_LINES"
    ;;
  full)
    echo "==> llvm-cov (full crate summary)"
    cargo llvm-cov --lib --tests --summary-only
    ;;
  html)
    echo "==> llvm-cov HTML → target/llvm-cov/html (pure modules)"
    cargo llvm-cov --lib --tests --html \
      --ignore-filename-regex "$PURE_IGNORE" \
      --output-dir target/llvm-cov/html
    echo "open target/llvm-cov/html/index.html"
    ;;
  lcov)
    echo "==> llvm-cov lcov → target/llvm-cov/lcov.info (pure modules)"
    mkdir -p target/llvm-cov
    cargo llvm-cov --lib --tests --lcov \
      --ignore-filename-regex "$PURE_IGNORE" \
      --output-path target/llvm-cov/lcov.info \
      --fail-under-lines "$PURE_FAIL_UNDER_LINES"
    ;;
  *)
    echo "usage: $0 [pure|full|html|lcov]" >&2
    exit 2
    ;;
esac
