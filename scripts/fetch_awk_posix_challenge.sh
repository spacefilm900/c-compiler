#!/bin/sh
# Fetch the private Independent POSIX AWK Challenge at tag v0.6.
# Does not modify cases/. A failed clone is an error here; CI turns that
# into a skip because the default GITHUB_TOKEN often cannot read it.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
PIN=69b1f5895022956c5ebfc0d6d58fd45636b6dfaa
DEST="$ROOT/tests/external/awk-posix-challenge"
REPO=spacefilm900/awk-posix-challenge

case "$DEST" in
    */tests/external/awk-posix-challenge) ;;
    *)
        echo "refusing to use unexpected destination: $DEST" >&2
        exit 1
        ;;
esac

if [ -d "$DEST/.git" ] && [ -f "$DEST/tools/runner.py" ]; then
    got=$(git -C "$DEST" rev-parse HEAD)
    if [ "$got" = "$PIN" ]; then
        echo "awk-posix-challenge already at $PIN"
        exit 0
    fi
fi

rm -rf "$DEST"
echo "cloning $REPO at v0.6 ($PIN)"
if ! gh repo clone "$REPO" "$DEST" -- --depth 1 --branch v0.6; then
    echo "could not clone $REPO." >&2
    echo "The repository is private. The default Actions GITHUB_TOKEN for spacefilm900/c-compiler often cannot clone other private repositories." >&2
    exit 1
fi

got=$(git -C "$DEST" rev-parse HEAD)
if [ "$got" != "$PIN" ]; then
    echo "v0.6 resolved to $got, expected $PIN" >&2
    exit 1
fi
echo "fetched awk-posix-challenge $PIN"
