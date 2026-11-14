#!/bin/sh
# Fetch the Freedoom 0.13.0 release and extract freedoom1.wad and
# freedoom2.wad.
#
# Freedoom is BSD-licensed (COPYING.txt in the release), so it may be
# redistributed, but it is still fetched rather than committed. The
# release zip and both extracted WADs are checked against pinned SHA-256
# sums. Each WAD ships four recorded demos (DEMO1 to DEMO4) in the vanilla
# 1.9 demo format; the DOOM harness plays every one of them.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
VERSION=0.13.0
URL="https://github.com/freedoom/freedoom/releases/download/v$VERSION/freedoom-$VERSION.zip"
ZIP_SHA256=3f9b264f3e3ce503b4fb7f6bdcb1f419d93c7b546f4df3e874dd878db9688f59
WAD_SHA256=7323bcc168c5a45ff10749b339960e98314740a734c30d4b9f3337001f9e703d
WAD2_SHA256=a8772e088847032510d97ba2312406a6998f21cbab44d4ff10696faa9c0ecd4b
DEST="$ROOT/tests/external/freedoom"

case "$DEST" in
    */tests/external/freedoom) ;;
    *)
        echo "refusing to use unexpected destination: $DEST" >&2
        exit 1
        ;;
esac

# Linux has sha256sum; macOS has shasum.
sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d ' ' -f 1
    else
        shasum -a 256 "$1" | cut -d ' ' -f 1
    fi
}

if [ -f "$DEST/freedoom1.wad" ] \
    && [ "$(sha256_of "$DEST/freedoom1.wad")" = "$WAD_SHA256" ] \
    && [ -f "$DEST/freedoom2.wad" ] \
    && [ "$(sha256_of "$DEST/freedoom2.wad")" = "$WAD2_SHA256" ]; then
    echo "freedoom1.wad and freedoom2.wad $VERSION already present"
    exit 0
fi

rm -rf "$DEST"
mkdir -p "$DEST"
curl -fsSL --retry 3 -o "$DEST/freedoom.zip" "$URL"
if [ "$(sha256_of "$DEST/freedoom.zip")" != "$ZIP_SHA256" ]; then
    echo "SHA-256 mismatch for $URL" >&2
    rm -rf "$DEST"
    exit 1
fi
unzip -q -j -o "$DEST/freedoom.zip" \
    "freedoom-$VERSION/freedoom1.wad" "freedoom-$VERSION/freedoom2.wad" \
    "freedoom-$VERSION/COPYING.txt" -d "$DEST"
rm -f "$DEST/freedoom.zip"
if [ "$(sha256_of "$DEST/freedoom1.wad")" != "$WAD_SHA256" ]; then
    echo "SHA-256 mismatch for freedoom1.wad" >&2
    rm -rf "$DEST"
    exit 1
fi
if [ "$(sha256_of "$DEST/freedoom2.wad")" != "$WAD2_SHA256" ]; then
    echo "SHA-256 mismatch for freedoom2.wad" >&2
    rm -rf "$DEST"
    exit 1
fi
echo "fetched freedoom1.wad and freedoom2.wad $VERSION"
