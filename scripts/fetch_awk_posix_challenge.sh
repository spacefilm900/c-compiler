#!/bin/sh
# Fetch the private Independent POSIX AWK Challenge at tag v0.6.
# Does not modify cases/. A failed clone is an error. CI skips this script
# only when AWK_SUITE_READ_TOKEN is unset; with that secret set, a failure
# fails the job.
#
# v0.6 is an annotated tag. The tag object is
# 69b1f5895022956c5ebfc0d6d58fd45636b6dfaa. A checkout's HEAD, including a
# shallow clone of the tag, is the commit that object points at. Compare
# HEAD to that commit. A shallow clone may not contain the tag object.
set -eu

# gh reads GH_TOKEN. Actions exposes the default token as GITHUB_TOKEN.
# The sibling private repos need AWK_SUITE_READ_TOKEN, which the workflow
# passes as GH_TOKEN. This fallback only authenticates gh when GH_TOKEN
# was left empty.
if [ -z "${GH_TOKEN:-}" ] && [ -n "${GITHUB_TOKEN:-}" ]; then
    GH_TOKEN=$GITHUB_TOKEN
    export GH_TOKEN
fi

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
# Commit of annotated tag v0.6 (v0.6^{commit}), not the tag object.
PIN=b46ea063c6d25372b6a2010ac537ca19a8fd1b1e
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
    echo "GH_TOKEN must be able to read that private repository. In Actions that is the AWK_SUITE_READ_TOKEN secret." >&2
    exit 1
fi

got=$(git -C "$DEST" rev-parse HEAD)
if [ "$got" != "$PIN" ]; then
    echo "v0.6 resolved to $got, expected $PIN" >&2
    exit 1
fi
echo "fetched awk-posix-challenge $PIN"
