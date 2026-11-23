/* Long double constant folding (x87 80-bit on x86-64 Linux, binary128 on
   AArch64 Linux, double elsewhere): conversions to and from
   float, double and every integer type, comparisons, and hexadecimal
   literals at the subnormal end. Every value is a static initializer, so
   the compiler must fold it; the bytes are printed and compared with Clang. */
int printf(const char *, ...);

static void bytes(const char *name, const void *p, int n) {
    const unsigned char *b = p;
    printf("%s:", name);
    for (int i = n - 1; i >= 0; i--)
        printf("%02x", b[i]);
    printf("\n");
}
#define LD(x) bytes(#x, &x, sizeof x)
#define D(x) bytes(#x, &x, sizeof x)
#define F(x) bytes(#x, &x, sizeof x)

/* long double -> double: rounding, ties to even, overflow, underflow. */
static double d1 = (double)(1.0L / 3.0L);
static double d2 = 1.0L + 0x1p-53L;                 /* tie: rounds to even (down) */
static double d3 = 1.0L + 0x3p-53L;                 /* tie: rounds to even (up) */
static double d4 = 1.0L + 0x1p-53L + 0x1p-60L;      /* above tie: up */
static double d5 = 0x1p1024L;                       /* overflow to infinity */
static double d6 = -0x1.fffffffffffffffep1023L;     /* rounds up to -infinity */
static double d7 = 0x1p-1074L;                      /* smallest subnormal */
static double d8 = 0x1p-1075L;                      /* tie with 0: to even (0) */
static double d9 = 0x1.8p-1075L;                    /* above tie: smallest subnormal */
static double d10 = 0x1.fffffffffffffp-1023L;       /* subnormal with rounding */
static double d11 = 0x1p-1080L;                     /* underflows to +0 */
static double d12 = -0x1p-1080L;                    /* underflows to -0 */
static double d13 = 0x1.fffffffffffff8p-1022L;      /* rounds up to the smallest normal */
static double d14 = -0.0L;
static double d15 = 0x1.fffffffffffff7ffp1023L;     /* largest finite, below tie */
static double d16 = 0x1.fffffffffffff8p1023L;       /* tie at the top: infinity */
static double d17 = 123456789.123456789L;
static double d18 = -0x1.23456789abcdef12p-1030L;
static double d19 = __builtin_infl();
static double d20 = -__builtin_infl();
/* long double -> float. */
static float f1 = 1.0L / 3.0L;
static float f2 = 1.0L + 0x1p-24L;                  /* tie to even */
static float f3 = 1.0L + 0x3p-24L;
static float f4 = 0x1p128L;
static float f5 = 0x1p-149L;
static float f6 = 0x1p-150L;
static float f7 = 0x1.000002p-150L;
static float f8 = 0x1.fffffep127L;
static float f9 = 0x1.ffffffp127L;                   /* tie at the top: infinity */
static float f10 = -0x1.234567p-140L;
static float f11 = -0x1p-200L;
static float f12 = __builtin_infl();

/* long double -> integers, truncating toward zero. WIDE(v) is v where long
   double has a 64-bit or wider significand and 0 where it is double (the
   value would not fit there); the unselected operand is not evaluated. */
#define WIDE(v) (sizeof(long double) > sizeof(double) ? (v) : 0)
static _Bool b1 = 0.5L;
static _Bool b2 = -0.0L;
static _Bool b3 = 0x1p-16445L;
static signed char sc1 = -128.9L;
static unsigned char uc1 = 255.99L;
static short s1 = -32768.5L;
static unsigned short us1 = 65535.75L;
static int i1 = -2147483648.75L;
static int i2 = 2147483647.999L;
static unsigned u1 = 4294967295.5L;
static long long l1 = -9223372036854775808.0L;
static long long l2 = WIDE(9223372036854775807.0L);
static long long l3 = -0.999L;
static unsigned long long ul1 = WIDE(18446744073709551615.0L);
static unsigned long long ul2 = WIDE(0x1.fffffffffffffffep63L);
static long long ll1 = -1234567890123456789.9L;
static unsigned long long ull1 = 9223372036854775808.0L;
static unsigned long long ull2 = 12345678901234567890.5L;
static unsigned long long ull3 = 0.75L;

/* integers -> long double: all 64 bits fit, so these are exact. */
static long double x1 = -9223372036854775807LL - 1;
static long double x2 = 18446744073709551615ULL;
static long double x3 = 9223372036854775807LL;
static long double x4 = (unsigned char)200;
static long double x5 = (_Bool)1;
static long double x6 = -1;
static long double x7 = 0u;
static long double x8 = (short)-32768;
static long double x9 = 4294967295u;
static long double x10 = 1ULL << 63;

/* float and double -> long double. */
static long double y1 = 0x1p-1074;
static long double y2 = 0x1.fffffffffffffp-1023;
static long double y3 = 0x1p-149f;
static long double y4 = -0x1.7fffffp-130f;
static long double y5 = 1e308;
static long double y6 = -0.0f;
static long double y7 = __builtin_inf();
static long double y8 = 0.1f;

/* Hexadecimal long double literals at the subnormal end. */
static long double h1 = 0x1p-16445L;                 /* smallest subnormal */
static long double h2 = 0x1.8p-16445L;               /* tie: to even (2 units) */
static long double h3 = 0x1p-16446L;                 /* tie with 0: to even (0) */
static long double h4 = 0x1.0000000000000001p-16446L; /* above tie: 1 unit */
static long double h5 = 0x3p-16446L;                 /* 1.5 units: to 2 */
static long double h6 = 0x1.fffffffffffffffep-16383L; /* largest subnormal */
static long double h7 = 0x1.ffffffffffffffffp-16383L; /* rounds to smallest normal */
static long double h8 = 0x1p-16500L;                 /* underflows to 0 */
static long double h9 = 0x1.23456789abcdef0123p-16400L;
static long double h10 = -0x5p-16447L;               /* 1.25 units: down to 1 */
static long double h11 = 0x1.ffffffffffffffffp16383L; /* overflows to infinity */

/* Comparisons of long double constants, including signed zeros,
   infinities, subnormals and NaN (every comparison with NaN is false,
   except !=). */
#define NANL __builtin_nanl("")
#define INFL __builtin_infl()
static int c[] = {
    1.0L < 2.0L, 2.0L < 1.0L, 1.0L <= 1.0L, 1.0L >= 2.0L, 2.0L > 1.0L, 1.0L == 1.0L, 1.0L != 1.0L,
    -0.0L == 0.0L, -0.0L < 0.0L, -0.0L <= 0.0L, 0.0L >= -0.0L, 0.0L != -0.0L,
    -1.0L < 1.0L, -2.0L < -1.0L, -1.0L > -2.0L, 1.0L > -1.0L, -2.0L >= -2.0L,
    INFL > 0x1.fffffffffffffffep16383L, -INFL < -0x1.fffffffffffffffep16383L, INFL == INFL, -INFL == INFL,
    0x1p-16445L > 0.0L, 0x1p-16445L < 0x1p-16444L, -0x1p-16445L < 0.0L, 0x1p-16445L == 0x2p-16446L,
    NANL == NANL, NANL != NANL, NANL < 1.0L, NANL > 1.0L, NANL <= 1.0L, NANL >= 1.0L, 1.0L < NANL,
    NANL == INFL, INFL != NANL, 1.0L + 0x1p-63L > 1.0L, 1.0L + 0x1p-64L == 1.0L,
    1e4000L > 1e3999L, -1e-4000L > -1e-3999L, 3.0L / 7.0L == 0.428571428571428571428L,
    (long double)0.1 == 0.1L, (long double)0.1f < 0.1L, 0.5L == 0.5f, 0.1L > 0.1,
};

/* Arithmetic special cases: infinities, zeros, NaN, and exact results. */
static long double a1 = INFL * 0.0L;
static long double a2 = -INFL * 2.0L;
static long double a3 = -0.0L * 5.0L;
static long double a4 = 0x1p-16445L * 0x1p-10L;
static long double a5 = 0x1.fffffffffffffffep16383L * 2.0L;
static long double a6 = 3.0L * 0x1.5555555555555556p-2L;
static long double a7 = INFL / INFL;
static long double a8 = 0.0L / 0.0L;
static long double a9 = -1.0L / 0.0L;
static long double a10 = 1.0L / -INFL;
static long double a11 = 0.0L / -3.0L;
static long double a12 = 1.0L / 3.0L;
static long double a13 = 2.0L / 3.0L;
static long double a14 = 0x1p-16445L / 3.0L;
static long double a15 = 1.0L - 1.0L;
static long double a16 = -1.0L - -1.0L;
static long double a17 = INFL - INFL;
static long double a18 = 1.0L - 0x1p-64L;
static long double a19 = 0x1p-16382L - 0x1p-16445L;
static long double a20 = NANL * 1.0L;
static long double a21 = 7.0L / NANL;
static long double a22 = 1e99999L;
static long double a23 = 1e-99999L;
static long double a24 = __builtin_nansl("");
static long double a25 = -__builtin_nanl("0x1234");
static long double a26 = 0x1.0000000000000002p0L * 0x1.fffffffffffffffcp-1L;
static long double a27 = 10.0L / 0x1p-16445L;

/* A NaN operand propagates: the first NaN, quieted, with its sign and
   payload. Widening keeps the payload too. An invalid operation gives the
   default (positive) NaN in every format. */
static long double n1 = -__builtin_nanl("5") * 2.0L;
static long double n2 = 2.0L / -__builtin_nanl("5");
static long double n3 = __builtin_nanl("1") - -__builtin_nanl("2");
static long double n4 = __builtin_nansl("3") + 1.0L;
static long double n5 = 1.0L - -__builtin_nanl("6");
static long double n6 = -__builtin_nans("9");
static long double n7 = __builtin_nanf("0x12345");
static double n8 = -__builtin_nanl("0x123456789abcdef");
static double n9 = __builtin_inf() * 0.0;
static double n10 = __builtin_inf() - __builtin_inf();
static float n11 = 0.0f / 0.0f;
static double n12 = -(0.0 / 0.0);
static double n13 = 1.0 - -__builtin_nan("7");
static double n14 = __builtin_nan("1") + -__builtin_nan("2");

/* __int128 to and from long double: rounding to 64 significant bits. */
static long double q1 = ((__int128)1 << 100) | 12345;
static long double q2 = ~(unsigned __int128)0;
static long double q3 = -((__int128)1 << 126) - 1;
static long double q4 = ((unsigned __int128)0xffffffffffffffffULL << 1) | 1; /* tie: to even */
static long double q5 = ((unsigned __int128)0xfffffffffffffffdULL << 2) | 2; /* tie: to even (up) */
static __int128 r1 = 0x1p100L;
static __int128 r2 = -0x1.fffffffffffffffep126L;
static unsigned __int128 r3 = WIDE(0x1.fffffffffffffffep127L);
static __int128 r4 = -1.5L;

int main(void) {
    LD(a1); LD(a2); LD(a3); LD(a4); LD(a5); LD(a6); LD(a7); LD(a8); LD(a9); LD(a10);
    LD(a11); LD(a12); LD(a13); LD(a14); LD(a15); LD(a16); LD(a17); LD(a18); LD(a19); LD(a20);
    LD(a21); LD(a22); LD(a23); LD(a24); LD(a25); LD(a26); LD(a27);
    LD(n1); LD(n2); LD(n3); LD(n4); LD(n5); LD(n6); LD(n7); D(n8); D(n9); D(n10); F(n11); D(n12); D(n13); D(n14);
    LD(q1); LD(q2); LD(q3); LD(q4); LD(q5);
    bytes("r1", &r1, 16); bytes("r2", &r2, 16); bytes("r3", &r3, 16); bytes("r4", &r4, 16);
    D(d1); D(d2); D(d3); D(d4); D(d5); D(d6); D(d7); D(d8); D(d9); D(d10);
    D(d11); D(d12); D(d13); D(d14); D(d15); D(d16); D(d17); D(d18); D(d19); D(d20);
    F(f1); F(f2); F(f3); F(f4); F(f5); F(f6); F(f7); F(f8); F(f9); F(f10); F(f11); F(f12);
    printf("%d %d %d %d %u %d %u %d %d %u\n", b1, b2, b3, sc1, uc1, s1, us1, i1, i2, u1);
    printf("%lld %lld %lld %llu %llu %lld %llu %llu %llu\n", l1, l2, l3, ul1, ul2, ll1, ull1, ull2, ull3);
    LD(x1); LD(x2); LD(x3); LD(x4); LD(x5); LD(x6); LD(x7); LD(x8); LD(x9); LD(x10);
    LD(y1); LD(y2); LD(y3); LD(y4); LD(y5); LD(y6); LD(y7); LD(y8);
    LD(h1); LD(h2); LD(h3); LD(h4); LD(h5); LD(h6); LD(h7); LD(h8); LD(h9); LD(h10); LD(h11);
    for (unsigned i = 0; i < sizeof c / sizeof c[0]; i++)
        printf("%d", c[i]);
    printf("\n");
    return 0;
}
