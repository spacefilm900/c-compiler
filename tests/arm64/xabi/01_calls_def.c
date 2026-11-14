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


F1 mk_f1(float a) { F1 r = {a}; return r; }
F4 mk_f4(float a) { F4 r = {a, a + 1, a + 2, a + 3}; return r; }
D2 mk_d2(double a) { D2 r = {a, a * 2}; return r; }
D4 mk_d4(double a) { D4 r = {a, a + 0.5, a + 1, a + 1.5}; return r; }
DA3 mk_da3(double a) { DA3 r = {{a, -a, a * 3}}; return r; }
FD mk_fd(float a, double b) { FD r = {a, b}; return r; }
C3 mk_c3(char a) { C3 r = {a, (char)(a + 1), (char)(a + 2)}; return r; }
I2 mk_i2(int a) { I2 r = {a, -a}; return r; }
I3 mk_i3(int a) { I3 r = {a, a * 2, a * 3}; return r; }
L2 mk_l2(long a) { L2 r = {a, a << 20}; return r; }
L3 mk_l3(long a) { L3 r = {a, a + 7, a * 11}; return r; }
CF mk_cf(char c, float f) { CF r = {c, f}; return r; }
Q mk_q(long a) { Q r = {(__int128)a << 70 | 5}; return r; }
U3 mk_u3(float a) { U3 r = {{a, a * 2, a * 3}}; return r; }
float take_u3(int pad, U3 u, double x) { return pad + u.b[0] + u.b[1] * 10 + u.a[2] * 100 + (float)x; }
NF4 mk_nf4(float a) { NF4 r = {{a, a * 2, a * 4, a * 8}}; return r; }

double take_hfa(F1 a, F4 b, D2 c, D4 d, DA3 e, NF4 f)
{
    return a.a + b.a + b.b * 2 + b.c * 3 + b.d * 4 + c.a + c.b * 5 + d.a + d.b + d.c + d.d * 7
        + e.a[0] + e.a[1] * 2 + e.a[2] * 9 + f.f.a + f.f.d * 3;
}

double take_hfa_spill(D4 a, D4 b, D4 c, double x, F4 d)
{
    return a.a + a.d + b.b * 2 + b.c + c.a * 3 + c.d + x * 10 + d.a + d.d * 5;
}

long take_small(C3 a, I2 b, I3 c, L2 d, CF e, FD f)
{
    return a.a + a.b * 3 + a.c * 5 + b.a * 7 + b.b + c.a + c.b + c.c * 2 + d.a + (d.b >> 20) * 3
        + e.c + (long)e.f + (long)f.a * 11 + (long)f.b;
}

long take_big(L3 a, L3 b, long x, L3 c)
{
    /* Change the copies: the caller's objects must not change. */
    a.a += 1000;
    b.c += 1000;
    return a.a + a.b + a.c + b.a * 2 + b.b + b.c + x * 100 + c.a + c.c;
}

long take_q(int a, Q b, __int128 c, int d)
{
    return a + (long)(b.v >> 70) + (long)(b.v & 0xff) + (long)(c >> 64) + (long)c + d;
}

long take_narrow(char a, signed char b, unsigned char c, short d, unsigned short e, _Bool f,
                 int g, long h, char i, short j, unsigned char k, _Bool l)
{
    return a + b * 3 + c + d * 5 + e + f * 7 + g + h + i * 11 + j * 13 + k + l * 17;
}

double take_floats(float a, double b, float c, double d, float e, double f, float g, double h,
                   float i, double j, float k)
{
    return a + b * 2 + c + d * 3 + e + f + g + h * 4 + i * 5 + j * 6 + k * 7;
}

long double take_ld(long double a, int b, long double c) { return a * 2 + b + c; }

long sum_va(int n, ...)
{
    __builtin_va_list ap;
    long s = 0;
    __builtin_va_start(ap, n);
    for (int i = 0; i < n; i++)
        s = s * 3 + __builtin_va_arg(ap, int);
    __builtin_va_end(ap);
    return s;
}

double sum_va_fp(int n, ...)
{
    __builtin_va_list ap;
    double s = 0;
    __builtin_va_start(ap, n);
    for (int i = 0; i < n; i++)
        s = s * 2 + __builtin_va_arg(ap, double);
    __builtin_va_end(ap);
    return s;
}

/* spec letters: i int, l long, d double, c C3, f F4, D D4, L L3, q __int128,
   g long double, s I3, x FD */
long sum_va_mixed(const char *spec, ...)
{
    __builtin_va_list ap, copy;
    long s = 0;
    __builtin_va_start(ap, spec);
    __builtin_va_copy(copy, ap);
    for (const char *p = spec; *p; p++) {
        s *= 7;
        switch (*p) {
        case 'i': s += __builtin_va_arg(ap, int); break;
        case 'l': s += __builtin_va_arg(ap, long); break;
        case 'd': s += (long)(__builtin_va_arg(ap, double) * 4); break;
        case 'c': { C3 v = __builtin_va_arg(ap, C3); s += v.a + v.b * 2 + v.c * 3; break; }
        case 'f': { F4 v = __builtin_va_arg(ap, F4); s += (long)(v.a + v.b * 2 + v.c * 3 + v.d * 4); break; }
        case 'D': { D4 v = __builtin_va_arg(ap, D4); s += (long)(v.a + v.b * 2 + v.c * 3 + v.d * 4); break; }
        case 'L': { L3 v = __builtin_va_arg(ap, L3); s += v.a + v.b * 2 + v.c * 3; break; }
        case 'q': { __int128 v = __builtin_va_arg(ap, __int128); s += (long)(v >> 64) + (long)v; break; }
        case 'g': s += (long)(__builtin_va_arg(ap, long double) * 8); break;
        case 's': { I3 v = __builtin_va_arg(ap, I3); s += v.a + v.b * 2 + v.c * 3; break; }
        case 'x': { FD v = __builtin_va_arg(ap, FD); s += (long)(v.a * 2 + v.b); break; }
        }
    }
    /* The copy still starts at the first variadic argument. */
    if (*spec == 'i' && __builtin_va_arg(copy, int) != 1)
        s = -1;
    __builtin_va_end(copy);
    __builtin_va_end(ap);
    return s;
}
