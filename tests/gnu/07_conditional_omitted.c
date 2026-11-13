/* GNU C: the conditional with the middle operand omitted (GCC manual,
   "Conditionals with Omitted Operands"). `x ?: y` is `x ? x : y`, except
   that `x` is evaluated once. */

int printf(const char *, ...);

static int calls;
static int side(int v) { calls++; return v; }

int main(void)
{
    int a = side(5) ?: 9;
    int b = side(0) ?: 9;
    const char *name = 0;
    const char *shown = name ?: "default";
    const char *kept = "kept" ?: "other";
    long c = 0 ?: 7L;
    double d = 0.0 ?: 2.5;
    unsigned char u = 0;
    int e = u ?: -1;
    int x = 3;
    int *p = 0;
    int *q = p ?: &x;
    int chained = 0 ?: 0 ?: 4;
    printf("%d %d %d %s %s %ld %.1f %d %d %d\n", a, b, calls, shown, kept, c, d, e, *q, chained);
    printf("%zu\n", sizeof(u ?: 1L));
    return 0;
}
