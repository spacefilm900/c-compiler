/* N1570 6.9 external definitions */
#include "check.h"

int tentative[];                     /* 6.9.2p5: completed as int[1] at the end of the unit */
int counter;
static int helper(int);
int uses_helper(void) { return helper(2); }
static int helper(int x) { return x * 3; }
int old_style(x, y) char x; float y; { return x + (int)y; }   /* parameters promoted at the call */
int (*get(void))(void) { return uses_helper; }
void unprototyped();
void unprototyped(n) int n; { counter = n; }
extern inline int ext_inline(int v) { return v + 2; }

int main(void) {
    tentative[0] = 4;
    CHECK(tentative[0] == 4);
    CHECK(uses_helper() == 6 && get()() == 6);
    CHECK(old_style(1, 2.5) == 3);
    unprototyped(8);
    CHECK(counter == 8);
    CHECK(ext_inline(1) == 3);
    return DONE();
}
