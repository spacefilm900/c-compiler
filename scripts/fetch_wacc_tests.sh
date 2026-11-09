#!/bin/sh
# Fetch the Writing a C Compiler test suite at a pinned commit.
# The tree is MIT-licensed (see tests/external/README.md) and is not vendored.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
PIN=ae12014d2dec14488f3f80d14df4b6d8e4634d7d
URL=https://github.com/nlsandler/writing-a-c-compiler-tests.git
DEST="$ROOT/tests/external/wacc"

case "$DEST" in
    */tests/external/wacc) ;;
    *)
        echo "refusing to use unexpected destination: $DEST" >&2
        exit 1
        ;;
esac

if [ -f "$DEST/.pin" ] \
    && [ "$(cat "$DEST/.pin")" = "$PIN" ] \
    && [ -f "$DEST/expected_results.json" ] \
    && [ -f "$DEST/LICENSE" ]; then
    echo "writing-a-c-compiler-tests already at $PIN"
    exit 0
fi

rm -rf "$DEST"
mkdir -p "$DEST"
git -C "$DEST" init -q
git -C "$DEST" remote add origin "$URL"
GIT_TERMINAL_PROMPT=0 git -C "$DEST" fetch --depth 1 origin "$PIN"
git -C "$DEST" checkout -q --detach FETCH_HEAD
printf '%s\n' "$PIN" > "$DEST/.pin"
echo "fetched writing-a-c-compiler-tests $PIN"
