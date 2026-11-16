#!/usr/bin/env bash
# Line and function coverage with cargo-llvm-cov, in two layers:
#   1. the Rust unit tests (`cargo test`), and
#   2. the same plus an instrumented build/compiler driven by the external
#      suites named in SUITES (default: the main differential suite and WACC).
# Writes OUT/unit.lcov (unit tests) and OUT/suites.lcov (the instrumented
# compiler under the suites), then prints per-file summaries of non-test code
# with tools/quality/lcov_summary.py for unit tests alone, the suites alone,
# and both merged (a line counts as covered if either covers it).
#
# Needs: cargo-llvm-cov (see reports/quality/baseline-2026-11/README.md for
# the pinned version), the llvm-tools-preview rustup component, clang, and
# the suite trees fetched (scripts/fetch_wacc_tests.sh, fetch_gcc_torture.sh).
#
# Usage: OUT=dir SUITES="tests wacc torture" tools/quality/coverage.sh
set -euo pipefail
cd "$(dirname "$0")/../.."
OUT=${OUT:-reports/quality/latest}
SUITES=${SUITES:-"tests wacc"}
mkdir -p "$OUT"

cargo llvm-cov clean --workspace
cargo llvm-cov --no-report
cargo llvm-cov report --lcov --output-path "$OUT/unit.lcov"

# shellcheck disable=SC1090
source <(cargo llvm-cov show-env --export-prefix)
cargo build --bin compiler
BIN="$CARGO_LLVM_COV_TARGET_DIR/debug/compiler"
[ -x "$BIN" ] || BIN="$(dirname "$CARGO_LLVM_COV_TARGET_DIR")/debug/compiler"
echo "instrumented compiler: $BIN"
for s in $SUITES; do
  echo "== suite $s"
  case "$s" in
    tests)   CC_UNDER_TEST="$BIN" python3 harness/run_tests.py > "$OUT/suite-tests.log" 2>&1 || true ;;
    wacc)    CC_UNDER_TEST="$BIN" python3 harness/run_wacc.py > "$OUT/suite-wacc.log" 2>&1 || true ;;
    torture) CC_UNDER_TEST="$BIN" python3 harness/run_gcc_torture.py > "$OUT/suite-torture.log" 2>&1 || true ;;
    *) echo "unknown suite $s" >&2; exit 2 ;;
  esac
  tail -n 3 "$OUT/suite-$s.log"
done
# This report covers the instrumented binary; it does not reliably include
# the unit-test profiles too, so the two lcov files are merged below.
cargo llvm-cov report --lcov --output-path "$OUT/suites.lcov"

echo "== unit tests only"
python3 tools/quality/lcov_summary.py "$OUT/unit.lcov" --csv "$OUT/coverage-unit.csv" --top 0
echo "== suites only: $SUITES"
python3 tools/quality/lcov_summary.py "$OUT/suites.lcov" --csv "$OUT/coverage-suites.csv" --top 0
echo "== unit tests plus suites"
python3 tools/quality/lcov_summary.py "$OUT/unit.lcov" "$OUT/suites.lcov" --csv "$OUT/coverage-combined.csv" --regions "$OUT/uncovered-combined.csv"
