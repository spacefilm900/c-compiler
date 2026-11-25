/* N1570 6.4.1 keywords, 6.4.2 identifiers, 6.4.3 universal character names */
#include "check.h"

static inline int k(register int v) { auto int w = v; return w; }
_Noreturn void never(void);
_Static_assert(1, "static assertion");
_Thread_local int tl = 3;
_Alignas(16) char buf[16];

int \u00e9t\u00E9 = 5;                        /* 6.4.3: UCNs in identifiers */
int _Under_score1, under_score_2, x\U000000c0;

int main(void) {
    volatile const unsigned long long int a = 1;
    signed short int b = 2;
    float f = 1; double d = 2; long double ld = 3;
    _Bool bo = 1;
    _Atomic int at = 1;
    int *restrict rp = 0;
    typedef struct { union { int i; } u; enum { Q } e; } S;
    S s = { { 4 }, Q };
    int r = 0;
    for (int i = 0; i < 3; i++) { if (i == 1) continue; else r += i; }
    do { r++; } while (0);
    while (r < 10) { r++; if (r == 8) break; }
    switch (r) { case 8: r = 1; break; default: r = 0; }
    goto end;
end:
    CHECK(r == 1 && k(2) == 2 && tl == 3 && _Alignof(char[16]) == 1 && (long)(void *)buf % 16 == 0);
    CHECK(\u00e9t\u00e9 == 5 && \u00e9t\U000000E9 == 5);
    CHECK(sizeof(a) == sizeof(long long) && b == 2 && f + d + ld == 6 && bo && at == 1 && !rp && s.u.i == 4 && s.e == Q);
    (void)x\u00c0;
    return DONE();
}
