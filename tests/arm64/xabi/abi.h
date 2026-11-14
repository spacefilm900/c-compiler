/* Shared declarations for the AArch64 calling-convention pair (AAPCS64 and
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
