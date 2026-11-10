#include <stdarg.h>

int sum_ints(int n, ...) {
    va_list ap;
    va_start(ap, n);
    int total = 0;
    int i;
    for (i = 0; i < n; i++) {
        total += va_arg(ap, int);
    }
    va_end(ap);
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
    int b;
};

int pair_sum(int tag, ...) {
    va_list ap;
    va_start(ap, tag);
    struct Pair p = va_arg(ap, struct Pair);
    va_end(ap);
    return p.a + p.b;
}

int pick_ptr(int tag, ...) {
    va_list ap;
    va_start(ap, tag);
    int *p = va_arg(ap, int *);
    int v = va_arg(ap, int);
    va_end(ap);
    return *p + v;
}
