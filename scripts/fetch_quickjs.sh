#!/bin/sh
# Fetch the QuickJS 2026-06-04 source release from bellard.org and the
# test262 revision its Makefile names (TEST262_COMMIT) from GitHub into
# tests/external/quickjs/ (gitignored, not vendored).
#
# Both tarballs are checked against SHA-256 sums recorded here when the pin
# was chosen. QuickJS is MIT licensed (see its LICENSE file); test262 is
# BSD licensed. They are test inputs only: nothing from them is copied into
# the compiler. The QuickJS tree is not patched; test262 gets QuickJS's own
# tests/test262.patch, which is how the QuickJS Makefile prepares it.
#
# Set QUICKJS_NO_TEST262=1 to fetch only QuickJS.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
VERSION=2026-06-04
SRC_SHA256=b376e839b322978313d929fd20663b11ba58b75df5a46c126dd19ea2fa70ad2a
TEST262_COMMIT=5c8206929d81b2d3d727ca6aac56c18358c8d790
TEST262_SHA256=d2bb9b620c0697ea1052ffb67cc77ad6a3bdb9ff0c9cfba95ac7f4f64b98014a
DEST="$ROOT/tests/external/quickjs"
WANT_TEST262=1
[ "${QUICKJS_NO_TEST262:-0}" = 1 ] && WANT_TEST262=0

case "$DEST" in
    */tests/external/quickjs) ;;
    *)
        echo "refusing to use unexpected destination: $DEST" >&2
        exit 1
        ;;
esac

TREE="$DEST/quickjs-$VERSION"
PIN="$VERSION $SRC_SHA256 $TEST262_COMMIT $TEST262_SHA256"
if [ -f "$DEST/.pin" ] && [ "$(cat "$DEST/.pin")" = "$PIN" ] && [ -f "$TREE/quickjs.c" ] \
    && { [ "$WANT_TEST262" = 0 ] || [ -f "$TREE/test262/harness/assert.js" ]; }; then
    echo "QuickJS $VERSION already present"
    exit 0
fi

# Linux has sha256sum; macOS has shasum.
sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d ' ' -f 1
    else
        shasum -a 256 "$1" | cut -d ' ' -f 1
    fi
}

download() { # url output
    for attempt in 1 2 3 4 5; do
        if curl -fsSL -m 300 -o "$2" "$1"; then
            return 0
        fi
        sleep "$attempt"
    done
    echo "could not download $1" >&2
    return 1
}

check() { # file sha256 name
    if [ "$(sha256_of "$1")" != "$2" ]; then
        echo "SHA-256 mismatch for $3" >&2
        rm -rf "$DEST"
        exit 1
    fi
}

rm -rf "$DEST"
mkdir -p "$DEST"
download "https://bellard.org/quickjs/quickjs-$VERSION.tar.xz" "$DEST/quickjs.tar.xz"
check "$DEST/quickjs.tar.xz" "$SRC_SHA256" "quickjs-$VERSION.tar.xz"
tar -xJf "$DEST/quickjs.tar.xz" -C "$DEST"
rm -f "$DEST/quickjs.tar.xz"
if [ "$WANT_TEST262" = 1 ]; then
    download "https://codeload.github.com/tc39/test262/tar.gz/$TEST262_COMMIT" "$DEST/test262.tar.gz"
    check "$DEST/test262.tar.gz" "$TEST262_SHA256" "test262 $TEST262_COMMIT"
    tar -xzf "$DEST/test262.tar.gz" -C "$DEST"
    rm -f "$DEST/test262.tar.gz"
    mv "$DEST/test262-$TEST262_COMMIT" "$TREE/test262"
    (cd "$TREE/test262" && patch -s -p1 < ../tests/test262.patch)
fi
printf '%s\n' "$PIN" > "$DEST/.pin"
echo "fetched QuickJS $VERSION$( [ "$WANT_TEST262" = 1 ] && echo " and test262 $TEST262_COMMIT")"
