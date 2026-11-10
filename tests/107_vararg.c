#include <stdarg.h>
#include <stdio.h>
#include <string.h>

int sum_ints(int n, ...) {
    va_list ap;
    va_list copy;
    va_start(ap, n);
    va_copy(copy, ap);
    int total = 0;
    int i;
    for (i = 0; i < n; i++) {
        total += va_arg(ap, int);
    }
    int again = 0;
    for (i = 0; i < n; i++) {
        again += va_arg(copy, int);
    }
    va_end(copy);
    va_end(ap);
    if (total != again) {
        return -1;
    }
    return total;
}

double last_double(int n, ...) {
    va_list ap;
    va_start(ap, n);
    double d = 0.0;
    int i;
    for (i = 0; i < n; i++) {
        d = va_arg(ap, double);
    }
    va_end(ap);
    return d;
}

struct Pair {
    int a;
    long b;
};

int pair_sum(int tag, ...) {
    va_list ap;
    va_start(ap, tag);
    struct Pair p = va_arg(ap, struct Pair);
    va_end(ap);
    return p.a + (int)p.b;
}

struct Big {
    long w;
    long x;
    long y;
    long z;
};

long big_sum(int tag, ...) {
    va_list ap;
    va_start(ap, tag);
    struct Big b = va_arg(ap, struct Big);
    va_end(ap);
    return b.w + b.x + b.y + b.z;
}

struct Dbl {
    double d;
};

struct Mixed {
    int i;
    double d;
};

struct Two {
    double a;
    double b;
};

double one_dbl(int tag, ...) {
    va_list ap;
    va_start(ap, tag);
    struct Dbl s = va_arg(ap, struct Dbl);
    va_end(ap);
    return s.d;
}

double mixed_dbl(int tag, ...) {
    va_list ap;
    va_start(ap, tag);
    struct Mixed s = va_arg(ap, struct Mixed);
    va_end(ap);
    return (double)s.i + s.d;
}

double two_dbl(int tag, ...) {
    va_list ap;
    va_start(ap, tag);
    struct Two s = va_arg(ap, struct Two);
    va_end(ap);
    return s.a + s.b;
}

int forward(char *buf, unsigned long n, const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int wrote = vsnprintf(buf, n, fmt, ap);
    va_end(ap);
    return wrote;
}

int main(void) {
    if (sum_ints(4, 1, 2, 3, 4) != 10) {
        return 1;
    }
    double d = last_double(2, 1.5, 2.25);
    unsigned long long bits = 0;
    memcpy(&bits, &d, sizeof bits);
    if (bits != 0x4002000000000000ULL) {
        return 2;
    }
    struct Pair p;
    p.a = 3;
    p.b = 4;
    if (pair_sum(0, p) != 7) {
        return 3;
    }
    struct Big b;
    b.w = 1;
    b.x = 2;
    b.y = 3;
    b.z = 4;
    if (big_sum(0, b) != 10) {
        return 4;
    }
    char buf[32];
    int n = forward(buf, sizeof buf, "n=%d %s", 7, "ok");
    if (n != 6 || strcmp(buf, "n=7 ok") != 0) {
        return 5;
    }
    struct Dbl one;
    one.d = 1.5;
    if (one_dbl(0, one) != 1.5) {
        return 6;
    }
    struct Mixed mixed;
    mixed.i = 3;
    mixed.d = 0.5;
    if (mixed_dbl(0, mixed) != 3.5) {
        return 7;
    }
    struct Two two;
    two.a = 1.25;
    two.b = 2.25;
    if (two_dbl(0, two) != 3.5) {
        return 8;
    }
    return 0;
}
