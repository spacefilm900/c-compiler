#!/bin/sh
# Report which C11 and common POSIX headers from the macOS SDK this
# compiler can include, each on its own (strict C11 and gnu11). Report
# only: always exits 0. Run on a Mac after `make`.
compiler="${CC_UNDER_TEST:-build/compiler}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
ok=0
bad=0
for h in assert ctype errno fenv float inttypes iso646 limits locale math \
         setjmp signal stdalign stdarg stdbool stddef stdint stdio stdlib \
         stdnoreturn string tgmath time uchar wchar wctype \
         unistd fcntl dirent sys/types sys/stat sys/time sys/wait pthread; do
    for std in c11 gnu11; do
        printf '#include <%s.h>\nint main(void) { return 0; }\n' "$h" > "$tmp/t.c"
        if err="$("$compiler" -std=$std "$tmp/t.c" -o "$tmp/t.ll" 2>&1)"; then
            ok=$((ok + 1))
        else
            bad=$((bad + 1))
            echo "FAIL <$h.h> -std=$std: $(printf '%s\n' "$err" | grep -m1 'error')"
        fi
    done
done
echo "SDK headers: $ok compiled, $bad failed"
exit 0
