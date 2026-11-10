#!/bin/sh
# Fetch the private Shakespeare volume tests at the pinned main commit.
# There is no release tag. ./fetch.sh downloads Gutenberg #100; it checks
# the pinned sha256 and does not change cases (this repo has no cases/).
# A failed fetch is an error. CI skips this script only when
# AWK_SUITE_READ_TOKEN is unset.
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
PIN=46701e477fe48a7e0592b8022c77ac29c4114582
DEST="$ROOT/tests/external/awk-volume-tests"
URL=https://github.com/spacefilm900/awk-volume-tests.git

case "$DEST" in
    */tests/external/awk-volume-tests) ;;
    *)
        echo "refusing to use unexpected destination: $DEST" >&2
        exit 1
        ;;
esac

if [ -d "$DEST/.git" ] && [ -f "$DEST/run.sh" ]; then
    got=$(git -C "$DEST" rev-parse HEAD)
    if [ "$got" = "$PIN" ] && [ -f "$DEST/.fetched" ]; then
        echo "awk-volume-tests already at $PIN"
        exit 0
    fi
fi

rm -rf "$DEST"
mkdir -p "$DEST"
git -C "$DEST" init -q
git -C "$DEST" remote add origin "$URL"
echo "fetching awk-volume-tests $PIN"
if ! GIT_TERMINAL_PROMPT=0 git -C "$DEST" -c credential.helper='!gh auth git-credential' fetch --depth 1 origin "$PIN"; then
    echo "could not fetch $URL." >&2
    echo "GH_TOKEN must be able to read that private repository. In Actions that is the AWK_SUITE_READ_TOKEN secret." >&2
    exit 1
fi
git -C "$DEST" checkout -q --detach FETCH_HEAD
got=$(git -C "$DEST" rev-parse HEAD)
if [ "$got" != "$PIN" ]; then
    echo "checkout resolved to $got, expected $PIN" >&2
    exit 1
fi

if [ ! -x "$DEST/fetch.sh" ]; then
    echo "awk-volume-tests has no fetch.sh" >&2
    exit 1
fi
(cd "$DEST" && ./fetch.sh)
touch "$DEST/.fetched"
echo "fetched awk-volume-tests $PIN"
