/* N1570 6.7 declarations, 6.7.1 storage-class specifiers, 6.7.4 function specifiers */
#include "check.h"
#include <stdlib.h>

typedef int I;
typedef int I;                     /* 6.7p3: C11 allows a typedef to be redefined to the same type */
static int s1;
extern int e1;
int e1 = 4;
_Thread_local static int tls;
static _Thread_local int tls2 = 2;
extern _Thread_local int tls3;
_Thread_local int tls3 = 3;

inline int in_def(int x) { return x + 1; }   /* an inline definition, with an external one below */
extern int in_def(int x);
static inline int sin(int x) { return x; }   /* a static inline function may use any name */
_Noreturn static void bye(int c) { exit(c); }

int main(void) {
    register int r = 5;
    auto int a = 6;
    static int st = 7;
    extern int e1;
    I i = 1;
    int a1 = 1, *a2 = &a1, a3[2] = { 1, 2 }, (*a4)(void) = 0;
    CHECK(r + a + st == 18 && e1 == 4 && i == 1 && s1 == 0 && *a2 == 1 && a3[1] == 2 && !a4);
    CHECK(tls == 0 && tls2 == 2 && tls3 == 3 && in_def(1) == 2 && sin(3) == 3);
    static _Thread_local int ltls = 9;
    CHECK(ltls == 9);
    int done = DONE();
    if (done)
        return done;
    bye(0);
}
