#!/bin/sh
# Fetch and build Csmith at a pinned commit. The tree is not vendored.
# Generated programs include runtime/csmith.h from this tree.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
PIN=30dccd73b78652c4719f36572994778a5b233a4e
URL=https://github.com/csmith-project/csmith.git
DEST="$ROOT/tests/external/csmith"

case "$DEST" in
    */tests/external/csmith) ;;
    *)
        echo "refusing to use unexpected destination: $DEST" >&2
        exit 1
        ;;
esac

if [ -f "$DEST/.pin" ] \
    && [ "$(cat "$DEST/.pin")" = "$PIN" ] \
    && [ -x "$DEST/install/bin/csmith" ] \
    && [ -f "$DEST/src/runtime/csmith.h" ]; then
    echo "csmith already at $PIN"
    exit 0
fi

rm -rf "$DEST"
mkdir -p "$DEST/src"
git -C "$DEST/src" init -q
git -C "$DEST/src" remote add origin "$URL"
GIT_TERMINAL_PROMPT=0 git -C "$DEST/src" fetch --depth 1 origin "$PIN"
git -C "$DEST/src" checkout -q --detach FETCH_HEAD

jobs=$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 2)
(
    cd "$DEST/src"
    ./configure --prefix="$DEST/install"
    make -j"$jobs"
    make install
)

printf '%s\n' "$PIN" > "$DEST/.pin"
echo "fetched and built csmith $PIN"
