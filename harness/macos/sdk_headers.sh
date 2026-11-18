#!/bin/sh
# Report which C11 and common POSIX headers from the macOS SDK this
# compiler can include, each on its own (strict C11 and gnu11), and the
# Darwin headers SQLite's amalgamation and shell reach on macOS. Report
# only: always exits 0. Run on a Mac after `make`.
compiler="${CC_UNDER_TEST:-build/compiler}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
ok=0
bad=0
for h in assert ctype errno fenv float inttypes iso646 limits locale math \
         setjmp signal stdalign stdarg stdbool stddef stdint stdio stdlib \
         stdnoreturn string tgmath time uchar wchar wctype \
         unistd fcntl dirent sys/types sys/stat sys/time sys/wait pthread \
         TargetConditionals dlfcn libkern/OSAtomic malloc/malloc pwd semaphore \
         sys/file sys/ioctl sys/mman sys/mount sys/param sys/resource \
         sys/statvfs sys/sysctl sys/uio utime mach/mach mach/mach_time \
         copyfile os/lock; do
    for std in c11 gnu11; do
        printf '#include <%s.h>\nint main(void) { return 0; }\n' "$h" > "$tmp/t.c"
        if err="$("$compiler" -std=$std "$tmp/t.c" -o "$tmp/t.ll" 2>&1)"; then
            ok=$((ok + 1))
        else
            bad=$((bad + 1))
            first="$(printf '%s\n' "$err" | grep -m1 'error')"
            echo "FAIL <$h.h> -std=$std: $first"
            # Show the lines around the error once per header.
            if [ "$std" = c11 ]; then
                file="${first%%:*}"
                line="$(printf '%s\n' "$first" | cut -d: -f2)"
                case "$line" in
                    ''|*[!0-9]*) ;;
                    *) [ -f "$file" ] && sed -n "$((line > 4 ? line - 4 : 1)),$((line + 2))p" "$file" | sed 's/^/    | /' ;;
                esac
            fi
        fi
    done
done
echo "SDK headers: $ok compiled, $bad failed"
exit 0
