/* GNU C: comparing pointers to distinct types. C11 6.5.8p2 and 6.5.9p2
   make these constraint violations; GCC and Clang accept them in GNU
   mode with a warning (-Wcompare-distinct-pointer-types) and compare the
   addresses. Shapes from Csmith seeds rejected in the 13 November 2026
   campaign. */

int printf(const char *, ...);

static int g[4];
static unsigned char uc;
struct S { int a; } s;

int main(void)
{
    int *i = &g[1];
    unsigned int *u = (unsigned int *)&g[1];
    unsigned int *u2 = (unsigned int *)&g[2];
    int **pi = &i;
    const int **cpi = (const int **)&i;
    volatile unsigned char *volatile *volatile *a = 0;
    unsigned char ***const b = 0;
    unsigned char *pu = &uc;
    unsigned char **ppu = &pu;
    volatile unsigned char **vppu = (volatile unsigned char **)ppu;
    struct S *ps = &s;
    const struct S **cpps = (const struct S **)&ps;
    struct S **pps = &ps;
    double *d = (double *)&g[0];

    printf("%d %d\n", i == u, i != u);
    printf("%d %d\n", i == u2, u2 != i);
    printf("%d %d %d %d\n", i < u2, u2 > i, i <= u, u >= i);
    printf("%d %d\n", pi == cpi, cpi != pi);
    printf("%d\n", a == b);
    printf("%d %d\n", ppu == vppu, vppu != ppu);
    printf("%d\n", pps == cpps);
    printf("%d %d\n", d == i, d < i);
    return 0;
}
