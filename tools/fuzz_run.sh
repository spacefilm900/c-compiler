#!/bin/sh
# Run one cargo-fuzz target for a fixed time, continuing past crashes so a
# long run collects every distinct failure in fuzz/artifacts/<target>/.
#
# Usage: tools/fuzz_run.sh TARGET SECONDS [extra libFuzzer flags]
# Env: HCC_FUZZ_VERIFY=1 (frontend only) runs llvm-as on every accepted
# module. Needs a nightly toolchain and cargo-fuzz (docs/FUZZING.md).
#
# Leak detection is off: record types are reference-counted and can point
# at themselves, so a compilation keeps them until the process exits. That
# is not a bug in a compiler process, but libFuzzer would stop on it.
set -eu
target=$1
seconds=$2
shift 2
cd "$(dirname "$0")/.."
mkdir -p "fuzz/corpus/$target" "fuzz/artifacts/$target"
ASAN_OPTIONS="${ASAN_OPTIONS:-detect_leaks=0}"
export ASAN_OPTIONS
exec cargo +nightly fuzz run -O "$target" "fuzz/corpus/$target" -- \
    -dict=fuzz/c.dict -timeout=10 -rss_limit_mb=4096 -max_len=65536 \
    -detect_leaks=0 -fork=1 -ignore_crashes=1 -ignore_timeouts=1 \
    -ignore_ooms=1 -max_total_time="$seconds" -print_final_stats=1 \
    -artifact_prefix="fuzz/artifacts/$target/" "$@"
