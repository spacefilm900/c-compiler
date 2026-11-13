#!/bin/sh
# Fetch doomgeneric at a pinned commit. The tree is a test input only: it
# is not vendored, not patched, and none of it is copied into the compiler.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
PIN=e257a09527062bfb986b6e5e0780839b6cbd5b71
URL=https://github.com/ozkl/doomgeneric.git
DEST="$ROOT/tests/external/doomgeneric"

case "$DEST" in
    */tests/external/doomgeneric) ;;
    *)
        echo "refusing to use unexpected destination: $DEST" >&2
        exit 1
        ;;
esac

if [ -f "$DEST/.pin" ] \
    && [ "$(cat "$DEST/.pin")" = "$PIN" ] \
    && [ -f "$DEST/doomgeneric/d_main.c" ]; then
    echo "doomgeneric already at $PIN"
    exit 0
fi

rm -rf "$DEST"
mkdir -p "$DEST"
git -C "$DEST" init -q
git -C "$DEST" remote add origin "$URL"
GIT_TERMINAL_PROMPT=0 git -C "$DEST" fetch --depth 1 origin "$PIN"
git -C "$DEST" checkout -q --detach FETCH_HEAD
printf '%s\n' "$PIN" > "$DEST/.pin"
echo "fetched doomgeneric $PIN"
