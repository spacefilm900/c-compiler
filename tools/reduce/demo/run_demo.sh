#!/usr/bin/env bash
# End-to-end reducer demo on two real bugs that pull request 46 fixed.
#
# Builds the compiler at commit 0cab101 (before the fixes) in a temporary
# git worktree, unless OLD_COMPILER names one already built, then reduces:
#   bitfield_mix.c  with --mode miscompile (bug b: `+=` of a double on a
#                   bit-field overwrote its neighbour)
#   wide_address.c  with --mode bad-ir     (bug a: an address converted to
#                   __int128 in a static initializer gave invalid IR)
# Each result is checked against the current compiler (build/compiler),
# which must no longer show the bug. Results go to $OUT (default
# /tmp/reduce-demo). Needs clang, and git and cargo when building.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
OUT="${OUT:-/tmp/reduce-demo}"
mkdir -p "$OUT"
need() {
    for tool in "$@"; do
        command -v "$tool" > /dev/null || { echo "FAIL: $tool is not installed" >&2; exit 1; }
    done
}
need clang python3 cmp grep
if [ -z "${OLD_COMPILER:-}" ]; then
    need git cargo
    tree="$OUT/src-0cab101"
    if [ ! -d "$tree" ]; then
        git -C "$ROOT" worktree add --detach "$tree" 0cab101
    fi
    (cd "$tree" && cargo build --release --quiet)
    OLD_COMPILER="$tree/target/release/compiler"
fi
NEW_COMPILER="${NEW_COMPILER:-$ROOT/build/compiler}"
for compiler in "$OLD_COMPILER" "$NEW_COMPILER"; do
    if [ ! -x "$compiler" ]; then
        echo "FAIL: $compiler is not an executable (run make)" >&2
        exit 1
    fi
done
REDUCE="python3 $ROOT/tools/reduce/reduce.py"

$REDUCE "$ROOT/tools/reduce/demo/bitfield_mix.c" --mode miscompile \
    --compiler "$OLD_COMPILER" -o "$OUT/bitfield_mix.reduced.c"
$REDUCE "$ROOT/tools/reduce/demo/wide_address.c" --mode bad-ir \
    --compiler "$OLD_COMPILER" -o "$OUT/wide_address.reduced.c"

for name in bitfield_mix wide_address; do
    echo "== $name.reduced.c"
    cat "$OUT/$name.reduced.c"
done

# The current compiler must not reproduce either bug. Each step must
# succeed for the expected reason; a missing tool or a crash fails the demo.
clang -w "$OUT/bitfield_mix.reduced.c" -o "$OUT/bitfield.ref"
"$OUT/bitfield.ref" > "$OUT/bitfield.ref.out"
"$NEW_COMPILER" "$OUT/bitfield_mix.reduced.c" -o "$OUT/bitfield.ll"
clang -w "$OUT/bitfield.ll" -o "$OUT/bitfield.new"
"$OUT/bitfield.new" > "$OUT/bitfield.new.out"
if ! cmp -s "$OUT/bitfield.ref.out" "$OUT/bitfield.new.out"; then
    echo "FAIL: the current compiler still miscompiles bitfield_mix.reduced.c" >&2
    exit 1
fi
status=0
"$NEW_COMPILER" "$OUT/wide_address.reduced.c" -o "$OUT/check.ll" 2> "$OUT/wide.err" || status=$?
if [ "$status" != 1 ] || ! grep -q "wider than a pointer" "$OUT/wide.err"; then
    echo "FAIL: expected the 'wider than a pointer' error (exit 1), got exit $status:" >&2
    cat "$OUT/wide.err" >&2
    exit 1
fi
echo "demo ok: both reduced bugs are fixed in $NEW_COMPILER"
