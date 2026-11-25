/* N1570 6.7.9 initialization, 6.7.10 static assertions */
#include "check.h"
#include <string.h>

struct pt { int x, y; };
struct nest { int a; struct pt p; int arr[3]; };
union u { int i; char c[4]; };
static struct nest zero;                         /* static: zero-initialised */
static double dz; static int *pz;
static int sa[] = { 1, 2, [5] = 6 };             /* size from the largest index */
static char str4[4] = "abcd";                    /* no room for the null: allowed */
static char strn[] = "ab";
static struct pt pts[] = { [1].y = 9, { 1, 2 } }; /* designator then positional */
static union u un = { 0x01020304 };              /* the first named member */
static union u un2 = { .c = { 9 } };

int main(void) {
    CHECK(zero.a == 0 && zero.p.y == 0 && zero.arr[2] == 0 && dz == 0.0 && pz == 0);
    CHECK(sizeof sa == 6 * sizeof(int) && sa[2] == 0 && sa[5] == 6);
    CHECK(memcmp(str4, "abcd", 4) == 0 && sizeof strn == 3 && strn[2] == 0);
    CHECK(sizeof pts == 3 * sizeof(struct pt) && pts[1].y == 9 && pts[2].x == 1 && pts[2].y == 2 && pts[0].x == 0);
    CHECK(un.i == 0x01020304 && un2.c[0] == 9);
    struct nest n = { 1, 2, 3, 4, 5, 6 };        /* brace elision */
    CHECK(n.a == 1 && n.p.x == 2 && n.p.y == 3 && n.arr[0] == 4 && n.arr[2] == 6);
    struct nest d = { .p.y = 7, .arr[1] = 8, 9 };   /* positional continues after the designator */
    CHECK(d.a == 0 && d.p.y == 7 && d.arr[1] == 8 && d.arr[2] == 9);
    struct nest o = { .a = 1, .a = 2 };           /* a later initializer overrides */
    CHECK(o.a == 2);
    int m[2][3] = { { 1 }, { 2, 3 } };
    CHECK(m[0][0] == 1 && m[0][1] == 0 && m[1][1] == 3);
    int flat[2][2] = { 1, 2, 3 };
    CHECK(flat[1][0] == 3 && flat[1][1] == 0);
    int scalar = { 5 };                           /* a scalar may be braced */
    CHECK(scalar == 5);
    char cs[] = { "xy" };
    CHECK(sizeof cs == 3);
    int v = 3;
    int auto_expr[3] = { v, v * 2, v + v + v };   /* automatic: any expressions */
    CHECK(auto_expr[2] == 9);
    struct pt p1 = { 1, 2 }, p2 = p1;            /* a struct from a compatible expression */
    CHECK(p2.y == 2);
    int idx[] = { [2] = 1, [0] = 3, 4 };
    CHECK(sizeof idx == 3 * sizeof(int) && idx[1] == 4);
    _Static_assert(sizeof(struct pt) >= 2 * sizeof(int), "struct pt");
    return DONE();
}
_Static_assert(1, "file scope");
struct with_assert { int a; _Static_assert(1, "member"); };
