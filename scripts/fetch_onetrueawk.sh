#!/bin/sh
# Fetch One True AWK at the 20260426 tag. The tree is not vendored.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
PIN=5739fd79bcfc75ba7526773d0cf634521f8aca3c
URL=https://github.com/onetrueawk/awk.git
DEST="$ROOT/tests/external/onetrueawk"

case "$DEST" in
    */tests/external/onetrueawk) ;;
    *)
        echo "refusing to use unexpected destination: $DEST" >&2
        exit 1
        ;;
esac

if [ -f "$DEST/.pin" ] \
    && [ "$(cat "$DEST/.pin")" = "$PIN" ] \
    && { [ -f "$DEST/makefile" ] || [ -f "$DEST/Makefile" ]; } \
    && [ -f "$DEST/awkgram.y" ]; then
    echo "onetrueawk already at $PIN"
    exit 0
fi

rm -rf "$DEST"
mkdir -p "$DEST"
git -C "$DEST" init -q
git -C "$DEST" remote add origin "$URL"
GIT_TERMINAL_PROMPT=0 git -C "$DEST" fetch --depth 1 origin "$PIN"
git -C "$DEST" checkout -q --detach FETCH_HEAD
printf '%s\n' "$PIN" > "$DEST/.pin"
echo "fetched onetrueawk $PIN (tag 20260426)"
