#!/bin/sh
# Fetch the Lua 5.4.7 source release and its official test suite from
# lua.org into tests/external/lua/ (gitignored, not vendored).
#
# Both tarballs are checked against the SHA-256 sums published on
# https://www.lua.org/ftp/ and https://www.lua.org/tests/. Lua is MIT
# licensed (the notice is in lua-5.4.7/src/lua.h and doc/readme.html). It
# is a test input only: nothing from it is copied into the compiler and the
# tree is not patched.
#
# lua.org sometimes drops TLS connections, so each download is retried on
# both host names.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
VERSION=5.4.7
SRC_SHA256=9fbf5e28ef86c69858f6d3d34eccc32e911c1a28b4120ff3e84aaa70cfbf1e30
TESTS_SHA256=8a4898ffe4c7613c8009327a0722db7a41ef861d526c77c5b46114e59ebf811e
DEST="$ROOT/tests/external/lua"

case "$DEST" in
    */tests/external/lua) ;;
    *)
        echo "refusing to use unexpected destination: $DEST" >&2
        exit 1
        ;;
esac

PIN="$VERSION $SRC_SHA256 $TESTS_SHA256"
if [ -f "$DEST/.pin" ] && [ "$(cat "$DEST/.pin")" = "$PIN" ] \
    && [ -f "$DEST/lua-$VERSION/src/lua.c" ] && [ -f "$DEST/lua-$VERSION-tests/all.lua" ]; then
    echo "Lua $VERSION and its tests already present"
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

download() { # path-on-lua.org output
    for attempt in 1 2 3 4 5 6; do
        for host in www.lua.org lua.org; do
            if curl -fsSL -m 120 -o "$2" "https://$host/$1"; then
                return 0
            fi
        done
        sleep "$attempt"
    done
    echo "could not download https://www.lua.org/$1" >&2
    return 1
}

rm -rf "$DEST"
mkdir -p "$DEST"
download "ftp/lua-$VERSION.tar.gz" "$DEST/lua.tar.gz"
download "tests/lua-$VERSION-tests.tar.gz" "$DEST/tests.tar.gz"
if [ "$(sha256_of "$DEST/lua.tar.gz")" != "$SRC_SHA256" ]; then
    echo "SHA-256 mismatch for lua-$VERSION.tar.gz" >&2
    rm -rf "$DEST"
    exit 1
fi
if [ "$(sha256_of "$DEST/tests.tar.gz")" != "$TESTS_SHA256" ]; then
    echo "SHA-256 mismatch for lua-$VERSION-tests.tar.gz" >&2
    rm -rf "$DEST"
    exit 1
fi
tar -xzf "$DEST/lua.tar.gz" -C "$DEST"
tar -xzf "$DEST/tests.tar.gz" -C "$DEST"
rm -f "$DEST/lua.tar.gz" "$DEST/tests.tar.gz"
printf '%s\n' "$PIN" > "$DEST/.pin"
echo "fetched Lua $VERSION and lua-$VERSION-tests"
