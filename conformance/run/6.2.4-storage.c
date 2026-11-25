/* N1570 6.2.4 storage durations of objects */
#include "check.h"

static int counter(void) {
    static int n;              /* static storage: initialised once, before startup */
    return ++n;
}

static int automatic(int first) {
    int v = 5;                 /* automatic: initialised on each entry to the declaration */
    if (first)
        v = 9;
    return v;
}

int main(void) {
    CHECK(counter() == 1 && counter() == 2 && counter() == 3);
    CHECK(automatic(1) == 9 && automatic(0) == 5);
    int *p[3];
    for (int i = 0; i < 3; i++) {
        int *q = (int[]){ i };  /* 6.5.2.5p5: a compound literal in a block is automatic */
        p[i] = q;
        CHECK(*q == i);
    }
    int sum = 0;
    for (int i = 0; i < 3; i++) {
        int k = i * 2;          /* a new object each iteration, initialised each time */
        sum += k;
    }
    CHECK(sum == 6);
    static const char *const lit = "abc";   /* string literals have static storage */
    CHECK(lit[2] == 'c');
    (void)p;
    return DONE();
}
