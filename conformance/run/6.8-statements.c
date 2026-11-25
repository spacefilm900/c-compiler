/* N1570 6.8 statements and blocks */
#include "check.h"

static int classify(int v) {
    switch (v) {
    case 1:
    case 2:
        return 12;
    case 3: {
        int k = 30;
        return k;
    }
    default:
        if (v < 0)
            break;
        return 99;
    case 10:
        ;
    }
    return -1;
}
static int duff(int n) {
    int count = 0, i = (n + 3) / 4;
    switch (n % 4) {
    case 0: do { count++;
    case 3: count++;
    case 2: count++;
    case 1: count++;
            } while (--i > 0);
    }
    return count;
}
static void nothing(void) { return; }
static int vla_loop(int n) {
    int total = 0;
    for (int i = 1; i <= n; i++) {
        int v[i];                    /* a new VLA each iteration */
        v[i - 1] = i;
        total += v[i - 1] + (int)sizeof v / (int)sizeof v[0];
    }
    return total;
}

int main(void) {
    CHECK(classify(1) == 12 && classify(2) == 12 && classify(3) == 30 && classify(5) == 99 && classify(-2) == -1 && classify(10) == -1);
    for (int n = 1; n < 10; n++)
        CHECK(duff(n) == n);
    switch ((unsigned char)300) { case 44: break; default: CHECK(0); }
    switch (3L) { case 3: break; default: CHECK(0); }
    int i = 0;
    while (i < 5) i++;
    CHECK(i == 5);
    do i--; while (i > 2);
    CHECK(i == 2);
    for (;;) { if (++i > 6) break; }
    CHECK(i == 7);
    int s = 0;
    for (int a = 0; a < 5; a++) { if (a % 2) continue; s += a; }
    CHECK(s == 6);
    int k = 0;
    do { k++; if (k < 3) continue; } while (k < 3);   /* continue goes to the controlling expression */
    CHECK(k == 3);
    int g = 0;
again:
    if (++g < 3)
        goto again;
    CHECK(g == 3);
    {
        goto inner;
        CHECK(0);
    inner:;
    }
    if (0) CHECK(0); else if (1) g = 4; else CHECK(0);
    CHECK(g == 4);
    if (1) if (0) CHECK(0); else g = 5;               /* else binds to the nearest if */
    CHECK(g == 5);
    nothing();
    CHECK(vla_loop(3) == 1 + 1 + 2 + 2 + 3 + 3);
    ;                                                  /* null statement */
    { }
    L1: L2: g++;
    CHECK(g == 6);
    return DONE();
}
