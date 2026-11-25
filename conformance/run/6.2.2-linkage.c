/* N1570 6.2.2 linkages of identifiers, 6.9.2 tentative definitions */
#include "check.h"

static int s;          /* internal linkage */
extern int s;          /* 6.2.2p4: keeps the earlier internal linkage */
int t;                 /* tentative definitions: one object, zero-initialised */
int t;
int t = 0;
int u;
static int f(void);
int f(void) { return 9; }   /* 6.2.2p5: no storage class, same as extern: keeps internal linkage */
int g(void);
int g(void) { return 1; }

int main(void) {
    CHECK(s == 0 && t == 0 && u == 0);
    s = 3;
    {
        extern int s;
        CHECK(s == 3);
        int g(void);           /* block-scope function declaration has linkage */
        CHECK(g() == 1);
    }
    CHECK(f() == 9);
    return DONE();
}
