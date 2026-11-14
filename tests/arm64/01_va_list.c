/* __builtin_va_list handed between functions, va_copy, function pointers to
   aggregate-returning functions, and calls without a prototype. The
   __builtin_va_list representation differs: char * on Apple arm64, a 32-byte
   structure on AAPCS64 Linux. */
int printf(const char *, ...);

typedef struct { double x, y; } V2;
typedef struct { long a, b, c; } Big;

static double vsum(int n, __builtin_va_list ap)
{
    double s = 0;
    while (n-- > 0)
        s = s * 3 + __builtin_va_arg(ap, double);
    return s;
}

static double outer(int n, ...)
{
    __builtin_va_list ap, again;
    __builtin_va_start(ap, n);
    __builtin_va_copy(again, ap);
    double a = vsum(n, ap);
    double b = vsum(n, again);
    __builtin_va_end(again);
    __builtin_va_end(ap);
    return a + b;
}

static long ints(int n, ...)
{
    __builtin_va_list ap;
    long s = 0;
    __builtin_va_start(ap, n);
    for (int i = 0; i < n; i++) {
        if (i % 2)
            s += __builtin_va_arg(ap, int) * 10;
        else
            s += __builtin_va_arg(ap, long long);
    }
    __builtin_va_end(ap);
    return s;
}

static V2 vadd(V2 a, V2 b) { V2 r = {a.x + b.x, a.y + b.y}; return r; }
static Big bump(Big b) { b.a++; b.c--; return b; }

int legacy();
double legacy_d();

int main(void)
{
    V2 (*f)(V2, V2) = vadd;
    Big (*g)(Big) = bump;
    V2 a = {1, 2}, b = {3.5, 4.5};
    Big big = {1, 2, 3};
    V2 c = f(a, b);
    Big d = g(big);
    printf("%g %g\n", outer(3, 1.0, 2.0, 3.0), outer(9, 1., 2., 3., 4., 5., 6., 7., 8., 9.));
    printf("%ld\n", ints(11, 1LL, 2, 3LL, 4, 5LL, 6, 7LL, 8, 9LL, 10, 1LL << 40));
    printf("%g %g %ld %ld %ld\n", c.x, c.y, d.a, d.b, d.c);
    printf("%d %g\n", legacy(3, (char)4, (short)5, 6.5f), legacy_d(2.5f, 7));
    return 0;
}

int legacy(int a, int b, int c, double d) { return a * 100 + b * 10 + c + (int)(d * 1000); }
double legacy_d(double x, int n) { return x * n; }
