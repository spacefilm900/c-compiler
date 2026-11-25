/* N1570 6.5.2 postfix operators */
#include "check.h"
#include <stdarg.h>
#include <stddef.h>

struct pt { int x, y; };
union un { int i; float f; };
static int sum(int n, ...) {
    va_list ap; va_start(ap, n);
    int s = 0;
    for (int i = 0; i < n; i++) s += va_arg(ap, int);
    va_end(ap);
    return s;
}
static double promoted(int n, ...) {
    va_list ap; va_start(ap, n);
    double d = va_arg(ap, double);     /* a float argument is promoted to double */
    int c = va_arg(ap, int);           /* a char argument is promoted to int */
    va_end(ap);
    return d + c + n;
}
static struct pt mk(int x) { struct pt p = { x, x + 1 }; return p; }
static int byval(struct pt p) { p.x = 100; return p.y; }
static int narrow(char c) { return c; }

int main(void) {
    int a[4] = { 10, 20, 30, 40 };
    CHECK(a[1] == 20 && 1[a] == 20 && *(a + 2) == 30);
    int m[2][3] = { { 1, 2, 3 }, { 4, 5, 6 } };
    CHECK(m[1][2] == 6 && *(*(m + 1) + 1) == 5);
    CHECK(sum(3, 1, 2, 3) == 6);
    CHECK(promoted(1, 1.5f, (char)2) == 4.5);
    CHECK(narrow(300 & 0x7f) == 300 % 128);
    struct pt p = { 1, 2 };
    CHECK(byval(p) == 2 && p.x == 1);         /* arguments are passed by value */
    CHECK(mk(5).y == 6);
    struct pt *pp = &p;
    pp->y = 9;
    CHECK(p.y == 9 && (&p)->x == 1);
    union un u;
    u.i = 0;
    u.f = 1.0f;
    CHECK(u.f == 1.0f);
    int i = 5;
    CHECK(i++ == 5 && i == 6 && i-- == 6 && i == 5);
    double d = 1.5;
    d++;
    CHECK(d == 2.5);
    int *ip = a;
    ip++;
    CHECK(*ip == 20);
    /* 6.5.2.5: compound literals */
    CHECK(((struct pt){ .y = 3 }).x == 0);
    int *cl = (int []){ 1, 2, 3 };
    size_t clsize = sizeof (int []){ 1, 2, 3 };
    CHECK(cl[2] == 3 && clsize == 3 * sizeof(int));
    const char *s = (const char []){ "abc" };
    CHECK(s[1] == 'b');
    /* 6.5.2.2p10: the function designator and arguments are evaluated before the call */
    int (*fp)(int, ...) = sum;
    CHECK(fp(2, 4, 5) == 9 && (*fp)(1, 7) == 7);
    return DONE();
}
