#!/bin/sh
# Fetch the SQLite amalgamation (sqlite3.c, sqlite3.h, shell.c, sqlite3ext.h)
# from sqlite.org into tests/external/sqlite/ (gitignored, not vendored).
#
# The pin is SQLite 3.54.1 (2026-10-16), the current release on
# https://sqlite.org/download.html when this script was written. The zip is
# checked against both the SHA3-256 sum published on that page and a
# SHA-256 sum recorded here. SQLite is public domain (the blessing is in
# sqlite3.h). It is a test input only: nothing from it is copied into the
# compiler and the tree is not patched.
#
# Downloads are retried, trying both sqlite.org and www.sqlite.org.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
VERSION=3.54.1
YEAR=2026
AMALGAMATION=sqlite-amalgamation-3540100
# SHA3-256 published on https://sqlite.org/download.html for this zip.
ZIP_SHA3_256=92281c86e2957f9b9305890f9ac04204caa7b677cc868dc326d942689df28e30
# SHA-256 of the same zip, recorded here so the fetch can be checked
# with the usual sha256sum tool.
ZIP_SHA256=0a9c8e34352dad46006aa82836d5383bff9dc6bc9ce5a60553c05aa30e6ad1f9
DEST="$ROOT/tests/external/sqlite"

case "$DEST" in
    */tests/external/sqlite) ;;
    *)
        echo "refusing to use unexpected destination: $DEST" >&2
        exit 1
        ;;
esac

PIN="$VERSION $ZIP_SHA256 $ZIP_SHA3_256"
if [ -f "$DEST/.pin" ] && [ "$(cat "$DEST/.pin")" = "$PIN" ] \
    && [ -f "$DEST/$AMALGAMATION/sqlite3.c" ] && [ -f "$DEST/$AMALGAMATION/shell.c" ]; then
    echo "SQLite $VERSION amalgamation already present"
    exit 0
fi

sha256_of() {
    sha256sum "$1" | cut -d ' ' -f 1
}

sha3_256_of() {
    # openssl dgst -sha3-256 prints "SHA3-256(file)= <hex>"
    openssl dgst -sha3-256 "$1" | awk '{print $NF}'
}

download() { # path-on-sqlite.org output
    for attempt in 1 2 3 4 5 6; do
        for host in www.sqlite.org sqlite.org; do
            if curl -fsSL -m 120 -o "$2" "https://$host/$1"; then
                return 0
            fi
        done
        sleep "$attempt"
    done
    echo "could not download https://www.sqlite.org/$1" >&2
    return 1
}

rm -rf "$DEST"
mkdir -p "$DEST"
download "$YEAR/$AMALGAMATION.zip" "$DEST/amalgamation.zip"
if [ "$(sha3_256_of "$DEST/amalgamation.zip")" != "$ZIP_SHA3_256" ]; then
    echo "SHA3-256 mismatch for $AMALGAMATION.zip" >&2
    rm -rf "$DEST"
    exit 1
fi
if [ "$(sha256_of "$DEST/amalgamation.zip")" != "$ZIP_SHA256" ]; then
    echo "SHA-256 mismatch for $AMALGAMATION.zip" >&2
    rm -rf "$DEST"
    exit 1
fi
unzip -q "$DEST/amalgamation.zip" -d "$DEST"
rm -f "$DEST/amalgamation.zip"
if [ ! -f "$DEST/$AMALGAMATION/sqlite3.c" ] || [ ! -f "$DEST/$AMALGAMATION/shell.c" ]; then
    echo "zip did not contain $AMALGAMATION/sqlite3.c and shell.c" >&2
    rm -rf "$DEST"
    exit 1
fi
printf '%s\n' "$PIN" > "$DEST/.pin"
echo "fetched SQLite $VERSION amalgamation"
