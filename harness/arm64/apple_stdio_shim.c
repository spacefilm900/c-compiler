/* Test-harness support, not part of the compiler.
 *
 * The Apple arm64 execution proxy runs code compiled for
 * arm64-apple-macos on a Linux AArch64 C library under qemu-aarch64. The
 * IR keeps Apple's calling convention (the object is built for the LLVM
 * triple arm64-apple-darwin-elf), so a call from that code to a variadic
 * glibc function would pass the variable arguments the Apple way (all on
 * the stack) while glibc expects AAPCS64 registers. This file is compiled
 * the same Apple way and provides the variadic stdio functions the tests
 * use, under the names __apple_printf and so on; the harness renames the
 * calls in the test's IR so glibc's own internal uses of printf and its
 * relatives are left alone. It reads the arguments with Apple's va_list (a char pointer) and
 * formats each conversion through a fixed-argument helper compiled for
 * Linux (linux_fmt_helpers.c). It is built by Clang, not by the compiler
 * under test.
 */
#include <stdarg.h>

typedef unsigned long size_t;
typedef struct _IO_FILE FILE;
extern FILE *stdout;
extern FILE *stderr;
size_t fwrite(const void *, size_t, size_t, FILE *);
double nan(const char *);
void *malloc(size_t);
void free(void *);

int __lx_fmt_ll(char *buf, size_t n, const char *spec, long long v);
int __lx_fmt_d(char *buf, size_t n, const char *spec, double v);
int __lx_fmt_p(char *buf, size_t n, const char *spec, const void *v);

struct sink {
    char *out;
    size_t cap;
    size_t len;
};

static void put(struct sink *s, const char *text, size_t n) {
    for (size_t i = 0; i < n; i++) {
        if (s->len + 1 < s->cap) {
            s->out[s->len] = text[i];
        }
        s->len++;
    }
    if (s->cap) {
        s->out[s->len < s->cap ? s->len : s->cap - 1] = 0;
    }
}

static size_t text_len(const char *p) {
    size_t n = 0;
    while (p[n]) n++;
    return n;
}

static int format_one(char *buf, size_t n, const char *spec, int kind, long long ival,
                      double dval, const void *pval) {
    if (kind == 1) return __lx_fmt_d(buf, n, spec, dval);
    if (kind == 2) return __lx_fmt_p(buf, n, spec, pval);
    return __lx_fmt_ll(buf, n, spec, ival);
}

static int engine(char *out, size_t cap, const char *fmt, va_list ap) {
    struct sink s = {out, cap, 0};
    char tmp[4096];
    if (cap) out[0] = 0;
    while (*fmt) {
        if (*fmt != '%') {
            const char *start = fmt;
            while (*fmt && *fmt != '%') fmt++;
            put(&s, start, (size_t)(fmt - start));
            continue;
        }
        if (fmt[1] == '%') {
            put(&s, "%", 1);
            fmt += 2;
            continue;
        }
        /* Build the conversion spec with '*' replaced by numbers. */
        char spec[64];
        size_t k = 0;
        spec[k++] = *fmt++;
        while (*fmt && (*fmt == '-' || *fmt == '+' || *fmt == ' ' || *fmt == '#' || *fmt == '0' || *fmt == '\'')) {
            spec[k++] = *fmt++;
        }
        for (int part = 0; part < 2; part++) {
            if (part == 1) {
                if (*fmt != '.') break;
                spec[k++] = *fmt++;
            }
            if (*fmt == '*') {
                int v = va_arg(ap, int);
                char num[16];
                if (part == 1 && v < 0) {
                    /* A negative precision is taken as if omitted. */
                    k--;
                    fmt++;
                    continue;
                }
                int m = __lx_fmt_ll(num, sizeof num, "%lld", v);
                for (int i = 0; i < m && k < 40; i++) spec[k++] = num[i];
                fmt++;
            } else {
                while (*fmt >= '0' && *fmt <= '9' && k < 40) spec[k++] = *fmt++;
            }
        }
        int len = 0; /* 1 h, 2 hh, 3 l, 4 ll, 5 L, 6 j/z/t */
        if (*fmt == 'h') { fmt++; len = 1; if (*fmt == 'h') { fmt++; len = 2; } }
        else if (*fmt == 'l') { fmt++; len = 3; if (*fmt == 'l') { fmt++; len = 4; } }
        else if (*fmt == 'L' || *fmt == 'q') { fmt++; len = 5; }
        else if (*fmt == 'j' || *fmt == 'z' || *fmt == 't') { fmt++; len = 6; }
        char conv = *fmt ? *fmt++ : 0;
        int n = 0;
        int kind = 0; /* 0: __lx_fmt_ll, 1: __lx_fmt_d, 2: __lx_fmt_p */
        long long ival = 0;
        double dval = 0;
        const void *pval = 0;
        switch (conv) {
        case 'd': case 'i': {
            long long v;
            if (len >= 3) v = va_arg(ap, long long);
            else v = va_arg(ap, int);
            if (len == 1) v = (short)v;
            if (len == 2) v = (signed char)v;
            spec[k++] = 'l'; spec[k++] = 'l'; spec[k++] = conv; spec[k] = 0;
            ival = v;
            break;
        }
        case 'u': case 'o': case 'x': case 'X': {
            unsigned long long v;
            if (len >= 3) v = va_arg(ap, unsigned long long);
            else v = va_arg(ap, unsigned int);
            if (len == 1) v = (unsigned short)v;
            if (len == 2) v = (unsigned char)v;
            spec[k++] = 'l'; spec[k++] = 'l'; spec[k++] = conv; spec[k] = 0;
            ival = (long long)v;
            break;
        }
        case 'c': {
            /* int, or wint_t for %lc; both are read as int. */
            int v = va_arg(ap, int);
            if (len == 3) spec[k++] = 'l';
            spec[k++] = 'c'; spec[k] = 0;
            ival = v;
            break;
        }
        case 'f': case 'F': case 'e': case 'E': case 'g': case 'G': case 'a': case 'A': {
            /* long double is double on Apple arm64. */
            double v = va_arg(ap, double);
            spec[k++] = conv; spec[k] = 0;
            kind = 1;
            dval = v;
            break;
        }
        case 's': case 'p': {
            const void *v = va_arg(ap, const void *);
            if (conv == 's' && len == 3) spec[k++] = 'l'; /* %ls: wchar_t * */
            spec[k++] = conv; spec[k] = 0;
            kind = 2;
            pval = v;
            break;
        }
        case 'n': {
            void *p = va_arg(ap, void *);
            switch (len) {
            case 1: *(short *)p = (short)s.len; break;
            case 2: *(signed char *)p = (signed char)s.len; break;
            case 3: case 4: case 6: *(long long *)p = (long long)s.len; break;
            default: *(int *)p = (int)s.len; break;
            }
            continue;
        }
        default:
            spec[k++] = conv; spec[k] = 0;
            put(&s, spec, k);
            continue;
        }
        n = format_one(tmp, sizeof tmp, spec, kind, ival, dval, pval);
        if (n < 0) return -1;
        if ((size_t)n < sizeof tmp) {
            put(&s, tmp, (size_t)n);
        } else {
            /* A conversion longer than tmp: format it again in full. */
            char *wide = malloc((size_t)n + 1);
            if (!wide) return -1;
            format_one(wide, (size_t)n + 1, spec, kind, ival, dval, pval);
            put(&s, wide, (size_t)n);
            free(wide);
        }
    }
    return (int)s.len;
}

int __apple_vsnprintf(char *out, size_t cap, const char *fmt, va_list ap) {
    return engine(out, cap, fmt, ap);
}

int __apple_vsprintf(char *out, const char *fmt, va_list ap) {
    return engine(out, (size_t)1 << 30, fmt, ap);
}

static char big[1 << 16];

int __apple_vfprintf(FILE *stream, const char *fmt, va_list ap) {
    va_list again;
    va_copy(again, ap);
    int n = engine(big, sizeof big, fmt, ap);
    char *text = big;
    if (n >= 0 && (size_t)n >= sizeof big) {
        /* Longer than the buffer: format again into one that fits. */
        text = malloc((size_t)n + 1);
        if (!text) {
            n = -1;
        } else if (engine(text, (size_t)n + 1, fmt, again) < 0) {
            /* The second pass failed too: nothing usable to write. */
            free(text);
            text = big;
            n = -1;
        }
    }
    va_end(again);
    /* Byte-counted, so a %c of 0 is written too. */
    if (n > 0 && fwrite(text, 1, (size_t)n, stream) != (size_t)n) n = -1;
    if (text != big) free(text);
    return n;
}

int __apple_vprintf(const char *fmt, va_list ap) {
    return __apple_vfprintf(stdout, fmt, ap);
}

int __apple_printf(const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int n = __apple_vfprintf(stdout, fmt, ap);
    va_end(ap);
    return n;
}

int __apple_fprintf(FILE *stream, const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int n = __apple_vfprintf(stream, fmt, ap);
    va_end(ap);
    return n;
}

int __apple_sprintf(char *out, const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int n = __apple_vsprintf(out, fmt, ap);
    va_end(ap);
    return n;
}

int __apple_snprintf(char *out, size_t cap, const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int n = __apple_vsnprintf(out, cap, fmt, ap);
    va_end(ap);
    return n;
}

/* long double is double on Apple arm64; glibc's nanl returns binary128. */
double __apple_nanl(const char *tag) {
    return nan(tag);
}

/* Silence the unused-function warning for text_len on some compilers. */
size_t __apple_shim_text_len(const char *p) {
    return text_len(p);
}
