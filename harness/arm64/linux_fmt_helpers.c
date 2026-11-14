/* Test-harness support, not part of the compiler.
 *
 * Fixed-argument formatting helpers for the Apple arm64 execution proxy
 * (see apple_stdio_shim.c). This file is compiled by Clang for
 * aarch64-linux-gnu, so it calls glibc with glibc's own calling
 * convention. It uses __snprintf_chk, glibc's public checked snprintf,
 * because the shim replaces snprintf itself.
 */
#include <stddef.h>

int __snprintf_chk(char *s, size_t maxlen, int flag, size_t slen, const char *format, ...);

int __lx_fmt_ll(char *buf, size_t n, const char *spec, long long v) {
    return __snprintf_chk(buf, n, 0, n, spec, v);
}

int __lx_fmt_d(char *buf, size_t n, const char *spec, double v) {
    return __snprintf_chk(buf, n, 0, n, spec, v);
}

int __lx_fmt_p(char *buf, size_t n, const char *spec, const void *v) {
    return __snprintf_chk(buf, n, 0, n, spec, v);
}
