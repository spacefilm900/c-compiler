#!/bin/sh
# Fetch gcc.c-torture/execute at a pinned GCC release.
# These files are test programs only. The sparse checkout is that
# directory and nothing else. Do not widen it to compiler sources.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
# releases/gcc-15.2.0
PIN=5115c7e447fc07457443df874bf57840e8316d5f
URL=https://github.com/gcc-mirror/gcc.git
DEST="$ROOT/tests/external/gcc-torture"
ONLY=gcc/testsuite/gcc.c-torture/execute

case "$DEST" in
    */tests/external/gcc-torture) ;;
    *)
        echo "refusing to use unexpected destination: $DEST" >&2
        exit 1
        ;;
esac

if [ -f "$DEST/.pin" ] \
    && [ "$(cat "$DEST/.pin")" = "$PIN" ] \
    && [ -d "$DEST/$ONLY" ]; then
    echo "gcc torture execute tests already at $PIN"
    exit 0
fi

rm -rf "$DEST"
mkdir -p "$DEST"
git -C "$DEST" init -q
git -C "$DEST" remote add origin "$URL"
# Non-cone. Cone mode would also check out every file in gcc/, which is
# the compiler. The pattern below is the execute tests and nothing else.
git -C "$DEST" sparse-checkout init --no-cone
git -C "$DEST" sparse-checkout set "$ONLY"
GIT_TERMINAL_PROMPT=0 git -C "$DEST" fetch --depth 1 --filter=blob:none origin "$PIN"
git -C "$DEST" checkout -q --detach FETCH_HEAD

if find "$DEST" -name '*.cc' -o -name '*.c' | grep -v '/gcc.c-torture/execute/' | grep -q .; then
    echo "refusing to keep a checkout that contains files outside gcc.c-torture/execute" >&2
    exit 1
fi

if [ ! -d "$DEST/$ONLY" ]; then
    echo "gcc.c-torture/execute was not checked out" >&2
    exit 1
fi

printf '%s\n' "$PIN" > "$DEST/.pin"
echo "fetched gcc.c-torture/execute from GCC 15.2.0 ($PIN)"
