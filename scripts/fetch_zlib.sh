#!/bin/sh
# Fetch the zlib 1.3.1 source release into tests/external/zlib/ (gitignored,
# not vendored). The tarball is checked against the SHA-256 published with
# the release (https://zlib.net/ and the GitHub release v1.3.1). zlib is
# under the zlib license (see zlib.h). It is a test input only: nothing from
# it is copied into the compiler and the tree is not patched.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
VERSION=1.3.1
SHA256=9a93b2b7dfdac77ceba5a558a580e74667dd6fede4585b91eefb60f03b72df23
DEST="$ROOT/tests/external/zlib"

case "$DEST" in
    */tests/external/zlib) ;;
    *)
        echo "refusing to use unexpected destination: $DEST" >&2
        exit 1
        ;;
esac

PIN="$VERSION $SHA256"
if [ -f "$DEST/.pin" ] && [ "$(cat "$DEST/.pin")" = "$PIN" ] && [ -f "$DEST/zlib-$VERSION/zlib.h" ]; then
    echo "zlib $VERSION already present"
    exit 0
fi

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d ' ' -f 1
    else
        shasum -a 256 "$1" | cut -d ' ' -f 1
    fi
}

# zlib.net keeps only the newest release at its top level; the GitHub
# release asset and the zlib.net fossils directory keep this one.
rm -rf "$DEST"
mkdir -p "$DEST"
ok=0
for attempt in 1 2 3 4; do
    for url in "https://github.com/madler/zlib/releases/download/v$VERSION/zlib-$VERSION.tar.gz" \
               "https://zlib.net/fossils/zlib-$VERSION.tar.gz"; do
        if curl -fsSL -m 120 -o "$DEST/zlib.tar.gz" "$url"; then
            ok=1
            break 2
        fi
    done
    sleep "$attempt"
done
if [ "$ok" != 1 ]; then
    echo "could not download zlib-$VERSION.tar.gz" >&2
    exit 1
fi
if [ "$(sha256_of "$DEST/zlib.tar.gz")" != "$SHA256" ]; then
    echo "SHA-256 mismatch for zlib-$VERSION.tar.gz" >&2
    rm -rf "$DEST"
    exit 1
fi
tar -xzf "$DEST/zlib.tar.gz" -C "$DEST"
rm -f "$DEST/zlib.tar.gz"
printf '%s\n' "$PIN" > "$DEST/.pin"
echo "fetched zlib $VERSION"
