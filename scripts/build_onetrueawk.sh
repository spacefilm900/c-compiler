#!/bin/sh
# Build One True AWK twice from the pinned tree.
#
# bison produces awkgram.tab.c the way the upstream makefile does
# (YACC = bison -d). maketab is a host program: it is compiled with the
# system cc, not with this compiler, and it writes proctab.c.
#
# Both awk binaries are compiled at -O0. Clang is also given -std=c11.
# -D_DEFAULT_SOURCE exposes POSIX declarations such as srandom, which
# glibc hides when __STRICT_ANSI__ is set. This compiler always defines
# that macro. Clang -std=c11 defines it too, so both builds pass the
# same -D.
#
# Usage: scripts/build_onetrueawk.sh clang|ccomp DEST
# DEST is the directory that receives the awk binary.
set -eu

if [ "$#" -ne 2 ]; then
    echo "usage: $0 clang|ccomp DEST" >&2
    exit 2
fi

MODE=$1
DEST=$2
case "$MODE" in
    clang | ccomp) ;;
    *)
        echo "mode must be clang or ccomp, not $MODE" >&2
        exit 2
        ;;
esac

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
SRC="$ROOT/tests/external/onetrueawk"
COMPILER=${CC_UNDER_TEST:-$ROOT/build/hcc}
CLANG=${CLANG:-clang}

if [ ! -f "$SRC/awkgram.y" ]; then
    echo "missing $SRC. Run scripts/fetch_onetrueawk.sh first." >&2
    exit 1
fi

if [ ! -f "$SRC/awkgram.tab.c" ] || [ ! -f "$SRC/awkgram.tab.h" ]; then
    (cd "$SRC" && bison -d awkgram.y)
fi
if [ ! -f "$SRC/proctab.c" ]; then
    (cd "$SRC" && cc -g -Wall -pedantic -Wcast-qual maketab.c -o maketab)
    (cd "$SRC" && ./maketab awkgram.tab.h > proctab.c)
fi

mkdir -p "$DEST"
rm -f "$DEST"/*.o "$DEST"/*.ll "$DEST"/awk

sources="awkgram.tab.c b.c main.c parse.c proctab.c tran.c lib.c run.c lex.c"
failed=0
for src in $sources; do
    base=${src%.c}
    if [ "$MODE" = "clang" ]; then
        if ! "$CLANG" -std=c11 -O0 -D_DEFAULT_SOURCE -I "$SRC" -w -c \
            "$SRC/$src" -o "$DEST/$base.o"; then
            echo "clang failed to compile $src" >&2
            failed=1
        fi
    else
        if ! "$COMPILER" -D_DEFAULT_SOURCE -I "$SRC" "$SRC/$src" -o "$DEST/$base.ll"; then
            echo "compiler failed to compile $src" >&2
            failed=1
            continue
        fi
        if ! "$CLANG" -O0 -w -c "$DEST/$base.ll" -o "$DEST/$base.o"; then
            echo "clang rejected IR for $src" >&2
            failed=1
        fi
    fi
done

if [ "$failed" -ne 0 ]; then
    exit 1
fi

objects=""
for src in $sources; do
    base=${src%.c}
    objects="$objects $DEST/$base.o"
done
# shellcheck disable=SC2086
"$CLANG" -O0 -o "$DEST/awk" $objects -lm
echo "built $MODE awk at $DEST/awk"
