#!/usr/bin/env bash
# M4: mutation testing via cargo-mutants (reads .cargo/mutants.toml).
# Usage:
#   ./scripts/mutants.sh            # full run on examine_globs
#   ./scripts/mutants.sh list       # list mutants only
#   ./scripts/mutants.sh in-diff    # mutants in uncommitted / PR-style diff
#   ./scripts/mutants.sh metrics    # only src/metrics.rs
#   ./scripts/mutants.sh alert
#   ./scripts/mutants.sh prometheus
#   ./scripts/mutants.sh container
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MODE="${1:-run}"

if ! command -v cargo-mutants >/dev/null 2>&1 && ! cargo mutants --version >/dev/null 2>&1; then
  echo "error: cargo-mutants not found. Install with:" >&2
  echo "  cargo install cargo-mutants --locked" >&2
  exit 127
fi

# CI / scripts: mutate in place to avoid tree copy overhead.
COMMON=(--in-place -v)

case "$MODE" in
  run|"")
    echo "==> cargo mutants (config: .cargo/mutants.toml)"
    cargo mutants "${COMMON[@]}"
    ;;
  list)
    cargo mutants --list
    ;;
  in-diff)
    echo "==> cargo mutants --in-diff (git diff vs HEAD)"
    cargo mutants "${COMMON[@]}" --in-diff=HEAD
    ;;
  metrics)
    cargo mutants "${COMMON[@]}" -f 'src/metrics.rs'
    ;;
  alert)
    cargo mutants "${COMMON[@]}" -f 'src/alert.rs'
    ;;
  prometheus)
    cargo mutants "${COMMON[@]}" -f 'src/export/prometheus.rs'
    ;;
  container)
    cargo mutants "${COMMON[@]}" -f 'src/collector/container.rs'
    ;;
  *)
    echo "usage: $0 [run|list|in-diff|metrics|alert|prometheus|container]" >&2
    exit 2
    ;;
esac
