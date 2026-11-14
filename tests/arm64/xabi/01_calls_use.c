int printf(const char *, ...);
/* Declarations shared for the AArch64 calling-convention pair (AAPCS64 and
   Apple arm64). Each kind of argument and result is crossed between this
   compiler and the Clang oracle by harness/run_cross.py. */
typedef struct { float a; } F1;
typedef struct { float a, b, c, d; } F4;
typedef struct { double a, b; } D2;
typedef struct { double a, b, c, d; } D4;
typedef struct { double a[3]; } DA3;
typedef struct { float a; double b; } FD;
typedef struct { char a, b, c; } C3;
typedef struct { int a, b; } I2;
typedef struct { int a, b, c; } I3;
typedef struct { long a, b; } L2;
typedef struct { long a, b, c; } L3;
typedef struct { char c; float f; } CF;
typedef struct { __int128 v; } Q;
typedef struct { F4 f; } NF4;
/* Union alternatives overlap: three float positions, an HFA. */
typedef union { float a[3]; float b[3]; } U3;

F1 mk_f1(float a);
F4 mk_f4(float a);
D2 mk_d2(double a);
D4 mk_d4(double a);
DA3 mk_da3(double a);
FD mk_fd(float a, double b);
C3 mk_c3(char a);
I2 mk_i2(int a);
I3 mk_i3(int a);
L2 mk_l2(long a);
L3 mk_l3(long a);
CF mk_cf(char c, float f);
Q mk_q(long a);
NF4 mk_nf4(float a);
U3 mk_u3(float a);
float take_u3(int pad, U3 u, double x);

double take_hfa(F1 a, F4 b, D2 c, D4 d, DA3 e, NF4 f);
double take_hfa_spill(D4 a, D4 b, D4 c, double x, F4 d);
long take_small(C3 a, I2 b, I3 c, L2 d, CF e, FD f);
long take_big(L3 a, L3 b, long x, L3 c);
long take_q(int a, Q b, __int128 c, int d);
long take_narrow(char a, signed char b, unsigned char c, short d, unsigned short e, _Bool f,
                 int g, long h, char i, short j, unsigned char k, _Bool l);
double take_floats(float a, double b, float c, double d, float e, double f, float g, double h,
                   float i, double j, float k);
long double take_ld(long double a, int b, long double c);
long sum_va(int n, ...);
double sum_va_fp(int n, ...);
long sum_va_mixed(const char *spec, ...);

int main(void)
{
    F1 f1 = mk_f1(1.5f);
    F4 f4 = mk_f4(2.0f);
    D2 d2 = mk_d2(3.0);
    D4 d4 = mk_d4(4.0);
    DA3 da3 = mk_da3(5.0);
    FD fd = mk_fd(6.5f, 7.25);
    C3 c3 = mk_c3(10);
    I2 i2 = mk_i2(11);
    I3 i3 = mk_i3(12);
    L2 l2 = mk_l2(13);
    L3 l3 = mk_l3(14);
    CF cf = mk_cf(15, 16.5f);
    Q q = mk_q(17);
    NF4 nf4 = mk_nf4(0.5f);
    printf("%g %g %g %g %g %g\n", f1.a, f4.a, f4.d, d2.b, d4.d, da3.a[2]);
    printf("%g %g %d %d %d %d %d %ld %ld %ld\n", fd.a, fd.b, c3.a, c3.c, i2.b, i3.c, cf.c, l2.b,
           l3.c, (long)(q.v >> 70));
    printf("%g %g\n", cf.f, nf4.f.d);
    printf("%g\n", take_hfa(f1, f4, d2, d4, da3, nf4));
    printf("%g\n", take_hfa_spill(d4, d4, mk_d4(1.0), 0.125, f4));
    printf("%ld\n", take_small(c3, i2, i3, l2, cf, fd));
    L3 a = mk_l3(1), b = mk_l3(2);
    printf("%ld %ld %ld\n", take_big(a, b, 3, mk_l3(4)), a.a, b.c);
    printf("%ld\n", take_q(1, q, ((__int128)9 << 64) | 8, 2));
    printf("%ld\n", take_narrow(-3, -4, 250, -300, 60000, 1, -7, 1L << 40, -9, -1000, 200, 1));
    printf("%g\n", take_floats(1.5f, 2.5, 3.5f, 4.5, 5.5f, 6.5, 7.5f, 8.5, 9.5f, 10.5, 11.5f));
    printf("%g\n", (double)take_ld(1.25L, 3, 0.5L));
    printf("%ld\n", sum_va(10, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10));
    printf("%g\n", sum_va_fp(10, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.5));
    printf("%ld\n", sum_va_mixed("icfDLqgsxld", 1, c3, f4, d4, l3, ((__int128)3 << 64) | 4,
                                 2.5L, i3, fd, 99L, 0.75));
    printf("%ld\n", sum_va_mixed("ddddddddddiiiiiiiiiiDf", 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0,
                                 8.0, 9.0, 10.0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, d4, f4));
    U3 u3 = mk_u3(1.5f);
    printf("%g %g\n", u3.a[2], take_u3(4, u3, 0.25));
    return 0;
}
