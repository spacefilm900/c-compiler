/* GNU C: typeof and __typeof__ (GCC manual, "Referring to a Type with
   typeof"). The operand is an expression or a type name; an expression
   operand is not evaluated. Strict C11 has no typeof keyword. */

int printf(const char *, ...);

struct point { int x, y; };
static int calls;
static int bump(void) { return ++calls; }
static long lfun(int v) { return v * 3L; }

int main(void)
{
    int i = 5;
    typeof(i) j = 7;
    __typeof__(int *) pi = &i;
    typeof(*pi) k = *pi + 1;
    typeof(unsigned char) uc = 300;
    typeof(1.5f) f = 2.25f;
    typeof(i + 1L) wide = 1L << 40;
    char buf[10];
    typeof(buf) other;
    struct point pt = {1, 2};
    typeof(pt) copy = pt;
    typeof(&pt) pp = &copy;
    typeof(lfun) *fp = lfun;
    typeof(bump()) unevaluated = 0;
    const int ci = 4;
    typeof(ci) cj = 9;
    typeof(typeof(char) *) cp = buf;
    typeof(int[3]) arr = {1, 2, 3};
    __typeof__(pt.x) px = pt.y;

    { __typeof__(i) _t = i; i = j; j = _t; }
    buf[0] = 'a';
    other[0] = 'b';
    cp[1] = 0;
    printf("%d %d %d %u %.2f %ld\n", i, j, k, uc, f, wide);
    printf("%zu %zu %zu %zu %zu\n", sizeof(other), sizeof(j), sizeof(uc), sizeof(wide), sizeof(arr));
    printf("%d %d %ld %d %d %d\n", pp->x, copy.y, fp(4), unevaluated, calls, cj);
    printf("%d %d %d\n", ({ typeof(3) _a = 3; typeof(8) _b = 8; _a > _b ? _a : _b; }),
           ({ typeof(-1) _a = -1; typeof(-7) _b = -7; _a > _b ? _a : _b; }),
           ({ typeof(i) _a = i; typeof(j) _b = j; _a > _b ? _a : _b; }));
    printf("%.1f %d %c\n", (double)({ typeof(1.5) _a = 1.5; typeof(0.5) _b = 0.5; _a > _b ? _a : _b; }), px, buf[0]);
    printf("%d\n", _Generic((typeof(ci))0, int: 1, default: 0));
    printf("%d\n", _Generic(&cj, const int *: 1, default: 0));
    return arr[2] - 3;
}
