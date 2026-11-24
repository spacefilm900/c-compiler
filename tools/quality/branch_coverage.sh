#!/usr/bin/env bash
# Line, function and branch coverage of the hcc crate, per test layer.
#
# Branch coverage needs a nightly toolchain (`-Zcoverage-options=branch`),
# so this script drives rustc's instrumentation directly instead of going
# through cargo-llvm-cov (whose 0.6.x release does not find test binaries
# in the newer nightly build-directory layout). The pinned 1.85.1 toolchain
# is still what builds and ships hcc; nightly is used here only to measure.
#
#   1. `cargo +nightly test --lib` with instrumentation (the unit tests);
#   2. an instrumented debug `hcc`, driven by each suite named in SUITES,
#      each into its own profile so a suite's own reach can be reported.
#
# Every layer gets OUT/<layer>.lcov; OUT/suites.lcov merges the suites and
# OUT/all.lcov everything. tools/quality/lcov_summary.py then prints per-file
# and per-module numbers (non-test code only) and the largest uncovered runs.
#
# Needs: rustup with a nightly toolchain and its llvm-tools-preview
# component, clang, and the suite trees fetched (scripts/fetch_wacc_tests.sh,
# scripts/fetch_gcc_torture.sh). The cross suites need qemu-aarch64 and the
# AArch64 glibc (see harness/run_cross.py); "ir" needs llc.
#
# Usage: OUT=dir SUITES="tests dumps ir wacc" tools/quality/branch_coverage.sh
# Default SUITES: every in-repo suite runnable on x86_64 Linux without Wine.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD
OUT=${OUT:-reports/quality/latest-branch}
SUITES=${SUITES:-"tests dumps ir cross-aarch64 cross-apple-arm64 cross-x86-mac wacc torture challenge"}
TOOLCHAIN=${TOOLCHAIN:-nightly}
TD="$ROOT/target/cov-branch"
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd)

SYSROOT=$(rustc +"$TOOLCHAIN" --print sysroot)
HOST=$(rustc +"$TOOLCHAIN" -vV | sed -n 's/^host: //p')
BIN_DIR="$SYSROOT/lib/rustlib/$HOST/bin"
PROFDATA="$BIN_DIR/llvm-profdata"
LLVMCOV="$BIN_DIR/llvm-cov"
[ -x "$PROFDATA" ] && [ -x "$LLVMCOV" ] || {
  echo "llvm-tools missing: rustup component add llvm-tools-preview --toolchain $TOOLCHAIN" >&2; exit 2; }

export RUSTFLAGS="-C instrument-coverage -Zcoverage-options=branch --cfg coverage"
export CARGO_TARGET_DIR="$TD"
rm -rf "$TD/prof"
mkdir -p "$TD/prof"

# Unit tests. %4m merges profiles of the same binary into a small pool of
# files online, so thousands of short hcc runs do not leave thousands of files.
TEST_BIN=$(cargo +"$TOOLCHAIN" test --lib --no-run --message-format=json 2>/dev/null \
  | python3 -c 'import json,sys
for l in sys.stdin:
    try: m = json.loads(l)
    except ValueError: continue
    if m.get("reason") == "compiler-artifact" and m.get("executable") and m["target"]["kind"] == ["lib"]:
        print(m["executable"])' | tail -n 1)
[ -x "$TEST_BIN" ] || { echo "unit-test binary not found" >&2; exit 1; }
mkdir -p "$TD/prof/unit"
LLVM_PROFILE_FILE="$TD/prof/unit/u-%4m.profraw" "$TEST_BIN" --test-threads "$(nproc)" > "$OUT/unit-tests.log" 2>&1 \
  || { tail -n 20 "$OUT/unit-tests.log"; echo "unit tests failed" >&2; exit 1; }
tail -n 2 "$OUT/unit-tests.log"

cargo +"$TOOLCHAIN" build --bin hcc 2>/dev/null
HCC="$TD/debug/hcc"
[ -x "$HCC" ] || { echo "instrumented hcc not found at $HCC" >&2; exit 1; }
echo "instrumented compiler: $HCC"

export_lcov() {  # export_lcov NAME PROFILE-DIR...
  local name=$1; shift
  local raws=()
  for d in "$@"; do raws+=("$d"/*.profraw); done
  "$PROFDATA" merge -sparse "${raws[@]}" -o "$TD/prof/$name.profdata"
  "$LLVMCOV" export -format=lcov -instr-profile="$TD/prof/$name.profdata" \
    -object "$TEST_BIN" -object "$HCC" \
    -ignore-filename-regex='/rustc/|/\.cargo/registry/|/\.rustup/toolchains/' > "$OUT/$name.lcov"
}

# A suite may report failures (coverage is still valid), but it must reach
# its final summary line; otherwise the measurement is incomplete.
incomplete=""
suite_dirs=()
for s in $SUITES; do
  echo "== suite $s"
  case "$s" in
    tests)   cmd=(python3 harness/run_tests.py);       done_re='^[0-9]+/[0-9]+ passed' ;;
    dumps)   cmd=(python3 harness/run_dump_tests.py);  done_re='dump checks passed' ;;
    ir)      cmd=(python3 harness/run_arm64_ir.py);    done_re='llc -filetype=obj accepted' ;;
    cross-aarch64)     cmd=(python3 harness/run_cross.py --target aarch64-linux-gnu);   done_re='tests, [0-9]+ passed' ;;
    cross-apple-arm64) cmd=(python3 harness/run_cross.py --target arm64-apple-macos);   done_re='tests, [0-9]+ passed' ;;
    cross-x86-mac)     cmd=(python3 harness/run_cross.py --target x86_64-apple-macos);  done_re='tests, [0-9]+ passed' ;;
    wacc)    cmd=(python3 harness/run_wacc.py);        done_re='^[0-9]+ passed, ' ;;
    torture) cmd=(python3 harness/run_gcc_torture.py); done_re='^[0-9]+ passed, ' ;;
    challenge) cmd=(python3 tools/platform/challenge_native.py
                    --kit tests/adversarial/hcc-challenge-v0.7 --kit tests/adversarial/hcc-challenge-v0.8
                    --kit tests/adversarial/hcc-challenge-v0.9 --opt O0 --out "$OUT/challenge-run"
                    --known-failures /dev/null)
               done_re='' ;;
    *) echo "unknown suite $s" >&2; exit 2 ;;
  esac
  dir="$TD/prof/$s"; mkdir -p "$dir"
  LLVM_PROFILE_FILE="$dir/h-%4m.profraw" CC_UNDER_TEST="$HCC" "${cmd[@]}" > "$OUT/suite-$s.log" 2>&1 || true
  tail -n 3 "$OUT/suite-$s.log"
  if [ "$s" = challenge ]; then
    [ -f "$OUT/challenge-run/results.json" ] || incomplete="$incomplete $s"
  elif ! grep -Eq "$done_re" "$OUT/suite-$s.log" \
       || grep -Eq 'compiler not found/executable:|clang not found:|is required; got:|is required \(LLVM' "$OUT/suite-$s.log"; then
    incomplete="$incomplete $s"
  fi
  if ls "$dir"/*.profraw >/dev/null 2>&1; then
    export_lcov "$s" "$dir"; suite_dirs+=("$dir")
  else
    incomplete="$incomplete $s(no-profile)"
  fi
done
export_lcov unit "$TD/prof/unit"
[ ${#suite_dirs[@]} -gt 0 ] && export_lcov suites "${suite_dirs[@]}"
export_lcov all "$TD/prof/unit" "${suite_dirs[@]}"

S=tools/quality/lcov_summary.py
echo "== unit tests only";      python3 $S "$OUT/unit.lcov"   --csv "$OUT/coverage-unit.csv" --modules-csv "$OUT/modules-unit.csv" --top 0
[ -f "$OUT/suites.lcov" ] && { echo "== suites only: $SUITES"; python3 $S "$OUT/suites.lcov" --csv "$OUT/coverage-suites.csv" --modules-csv "$OUT/modules-suites.csv" --top 0; }
echo "== everything";           python3 $S "$OUT/all.lcov"    --csv "$OUT/coverage-all.csv" --modules-csv "$OUT/modules-all.csv" \
  --regions "$OUT/uncovered-all.csv" --files sema.rs,parser.rs,irgen.rs,pp.rs,ast.rs,lexer.rs,driver.rs
if [ -n "$incomplete" ]; then
  echo "INCOMPLETE: suites did not finish:$incomplete" >&2
  exit 1
fi
