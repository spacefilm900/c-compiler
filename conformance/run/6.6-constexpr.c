/* N1570 6.6 constant expressions */
#include "check.h"
#include <stddef.h>

struct s { int a; char b[4]; double c; };
static int g[10];
static int *pa = &g[3] + 2;                    /* address constant */
static char *pb = (char *)&g + sizeof(int);
static int (*fpa)(void);
static const char *str = "lit" + 1;
static double dc = 1.0 / 3 + (int)2.5;          /* arithmetic constant expression */
static int ice = (int)3.9 + sizeof(struct s) * 0 + _Alignof(double) * 0;
static size_t off = offsetof(struct s, b[2]);
static int arr[(int)2.5 + 1];                  /* a cast of a floating constant is allowed in an ICE */
enum { E1 = 1 << 4, E2 = E1 * 2 + (3 > 2), E3 = sizeof(int[E1]) / sizeof(int) };
struct bf { unsigned w : (int)2.0 + 1; };
static int *np = 0;
static void *np2 = (void *)0;
static long cast_addr = 0;
static int cond = 1 ? 2 : (0 / 1);
static struct s sval = { .a = 1, .b = "abc", .c = 2.5 };
static int *const cp = g;
static int *mem = &sval.a;
static int *fcl = (int [2]){ 7, 8 };            /* a file-scope compound literal is static */

int main(void) {
    CHECK(*pa == 0 && pa == g + 5 && pb == (char *)(g + 1) && fpa == 0);
    CHECK(str[0] == 'i' && dc > 2.33 && dc < 2.34 && ice == 3 && off == offsetof(struct s, b) + 2);
    CHECK(sizeof arr == 3 * sizeof(int) && E2 == 33 && E3 == 16);
    CHECK(np == 0 && np2 == 0 && cast_addr == 0 && cond == 2 && sval.b[2] == 'c' && cp == g && mem == &sval.a && fcl[1] == 8);
    switch (5) { case 2 + 3: break; default: CHECK(0); }
    struct bf b = { 7 };
    CHECK(b.w == 7);
    static const int k = 3;
    (void)k;
    _Static_assert(sizeof(char) == 1 && (1 ? 1 : 0) && 0x7fffffff + 0u > 0, "ice");
    return DONE();
}
