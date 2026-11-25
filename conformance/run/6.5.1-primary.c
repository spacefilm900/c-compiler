/* N1570 6.5.1 primary expressions and 6.5.1.1 generic selection */
#include "check.h"

static int seen;
static int touch(void) { return ++seen; }

int main(void) {
    int x = 1;
    CHECK((x) == 1);
    (x) = 2;                                /* a parenthesised lvalue is an lvalue */
    CHECK(x == 2);
    CHECK(_Generic(1.0f, float: 1, double: 2, default: 3) == 1);
    CHECK(_Generic(x, default: 3, int: 4) == 4);
    CHECK(_Generic(touch(), int: 1) == 1 && seen == 0);   /* the controlling expression is not evaluated */
    CHECK(_Generic((char)0, char: 1, signed char: 2, unsigned char: 3) == 1);
    CHECK(_Generic((const int)0, int: 1, default: 0));
    int a[2];
    CHECK(_Generic(a, int *: 1, default: 0));          /* array-to-pointer conversion */
    CHECK(_Generic(main, int (*)(void): 1, default: 0));
    _Generic(x, int: (void)(x = 5), default: (void)0);
    CHECK(x == 5);
    *_Generic(x, int: &x) = 6;
    CHECK(x == 6);
    return DONE();
}
