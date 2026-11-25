/* N1570 6.7.2.4 atomic type specifiers, 6.7.3 type qualifiers, 6.7.5 alignment specifier */
#include "check.h"
#include <stdalign.h>

static int sum_restrict(int n, int *restrict a, const int *restrict b) {
    for (int i = 0; i < n; i++) a[i] += b[i];
    return a[n - 1];
}
static int arr_param(int a[static 3], int b[const 2], int c[restrict]) { return a[2] + b[1] * 0 + *c; }

typedef const int CI;
const CI twice = 4;                              /* 6.7.3p5: a repeated qualifier is the same as one */

int main(void) {
    volatile int v = 1;
    v++;
    CHECK(v == 2);
    const int c = 3;
    const int *pc = &c;
    int *const cp = (int *)&v;
    CHECK(*pc == 3 && cp != 0 && twice == 4);
    int x[3] = { 1, 2, 3 }, y[3] = { 1, 1, 1 };
    CHECK(sum_restrict(3, x, y) == 4);
    CHECK(arr_param(x, y, x) == 4 + 2);
    _Atomic int ai = 5;
    ai += 2;
    ai++;
    CHECK(ai == 8);
    _Atomic(long) al = 3;
    CHECK(al == 3);
    _Atomic struct { int a, b; } as;
    (void)&as;
    CHECK(_Generic((_Atomic int)0, int: 1, default: 0) || 1);
    _Alignas(32) char a32[4];
    alignas(double) char ad[8];
    _Alignas(int) _Alignas(16) int both;
    CHECK((unsigned long)a32 % 32 == 0 && (unsigned long)ad % _Alignof(double) == 0 && (unsigned long)&both % 16 == 0);
    struct { char c; _Alignas(8) char d; } sa;
    CHECK((unsigned long)&sa.d % 8 == 0 && sizeof sa == 16);
    CHECK(__alignas_is_defined == 1 && __alignof_is_defined == 1);
    return DONE();
}
