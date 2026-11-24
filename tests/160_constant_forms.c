/* Constant-expression forms the coverage run found untested: array bounds
   using ?:, unary + and -, every comparison and bitwise operator, unsigned
   wrap-around; arithmetic constant expressions mixing integer and floating
   operands; union constants whose first member is floating or an array;
   and pointer constants built from integers. Compared with Clang. */
int printf(const char *, ...);

/* Integer constant expressions in array bounds (C11 6.6p6). */
int b1[1 ? 3 : 4];
int b2[0 ? 1 : 5u];
int b3[+3];
int b4[-(-7)];
int b5[(3 > 2) + (2 < 3) + (2 <= 2) + (4 >= 4) + (1 == 1) + (1 != 2) + (2 > 3) + (3 <= 2) + (2 >= 3)];
int b6[(1 | 2) ^ 1];
int b7[(6 & 3) | 8];
int b8[(-1 < 0u) ? 1 : 9];
int b9[(unsigned char)-1 % 7 + 1];
int b10[-1u / 1000000000u];
int b11[(0u - 1u > 0) + ((long long)-1 < 0) + (-3 / 2 == -1) + (-3 % 2 == -1)];
int b12[1 ? 2 : (1 / 0)];
int b13[0 && (1 / 0) ? 1 : 2];
int b14[1 || (1 / 0) ? 3 : 4];
int b15[(char)300 + 0 > 0 ? 2 : 1];
int b16[sizeof(1 ? (short)1 : (char)2) * 3];
int b17[(0 ? 1u : -1) > 0 ? 5 : 6];
int b18[~0u >> 30];
int b19[(1u << 31) / (1u << 29)];
int b20[!0 + !5 + ~~2];
enum { E1 = (int)(1 ? -1 : 0u), E2 = (unsigned short)-1 > 0, E3 = +(-2) * -2, E4 = 7 ^ 5 | 8 };

/* Arithmetic constant expressions with floating operands (C11 6.6p8). */
static double f1 = 1 ? 2 : 3.0;
static double f2 = 0 ? 2.5 : 4;
static int f3 = !1.5;
static int f4 = !0.0;
static double f5 = +1.25 + -(3);
static double f6 = ~3 + 0.5;
static int f7 = 1.5 && 0.0;
static int f8 = 0.0 || -0.0;
static int f9 = 0.5 || 0;
static int f10 = 2.0 != 2.0f;
static int f11 = 1.0 < 1.5f;
static int f12 = 2.0 <= 1.0;
static int f13 = 3.0 >= 3;
static int f14 = 3.0 > 3;
static int f15 = 1.0 == 1;
static double f16 = (1 << 3) + 0.25;
static double f17 = (5 > 3) * 1.5;
static double f18 = (double)(7 / 2) + 7 / 2.0;
static double f19 = 1e300 * 1e10;
static float f20 = (float)(1u << 31) + -0.0f;
static double f21 = 3 % 2 + 0.5;
static double f22 = -(unsigned)1 + 0.0;
static long double f23 = 0 ? 1.0 : 2.0L;
static int f24 = (int)(0 ? 1.0 : 2.75);
static double f25 = (0.0 ? 1 : 2) + (0.5 ? 10 : 20);
static double f26 = -0.0 == 0.0 ? 1.0 / 4 : 7;

/* Union constants: the first member's bytes, padded with zeros. */
union UF { float f; int i; double pad; } u1 = { 1.5f };
union UD { double d[2]; char c; } u2 = { { 1.0, -2.0 } };
union UL { long double ld; char c[32]; } u3 = { 3.0L };
union UA { short s[3]; long long l; } u4 = { { 1, -2, 3 } };
union UN { struct { float x; char y; } s; double d; } u5 = { { 2.5f, 'y' } };
union UZ { float f; int i; } u6 = { 0.0f };
union UM { float f; int i; } u7 = { -0.0f };
union US { struct { unsigned a : 3, b : 5; float c; } s; long l; } u8 = { { 5, 17, 0.5f } };
int g;
struct BR { int a : 3; union { int *p; long l; } u; int b : 4; } s1 = { 3, { &g }, -2 };
struct BR2 { unsigned a : 4; struct { union { char *p; long l; } u; int k; } in; } s2 = { 9, { { (char *)&g + 2 }, 7 } };

/* Pointer constants made from integers. */
static char *p1 = (char *)0 + 0;
static int *p2 = (int *)16 + 2;
static int *p3 = (int *)64 - 4;
static char *p4 = (char *)(unsigned short)(char *)0x12345;
static char *p7 = (char *)(int)(char *)0;
static long long p5 = (long long)((char *)0x100 + 0x23);
static short *p6 = &((short *)0x40)[3];

/* The pre-offsetof idiom: an address formed with -> or [] on an integer
   constant cast to a pointer is an address constant (C11 6.6p9). */
struct OS { int a; short b[4]; struct { char c; long d; } in; };
static long long o1 = (long long)&((struct OS *)0)->b;
static unsigned long long o2 = (unsigned long long)&((struct OS *)0)->in.d;
static short *o3 = &((struct OS *)0)->b[2];
static struct OS *o4 = &((struct OS *)0x100)[1];
static short *o5 = ((struct OS *)0x100)->b;
static char *o6 = &(*(struct OS *)0x200).in.c;
static long long o7[] = { (long long)&((struct OS *)0)->in, (long long)&((struct OS *)8)->b[3] };

static void bytes(const char *name, const void *p, unsigned long n) {
    const unsigned char *b = p;
    printf("%s:", name);
    for (unsigned long i = 0; i < n; i++)
        printf("%02x", b[i]);
    printf("\n");
}

int main(void) {
    printf("%d %d %d %d %d %d %d %d %d %d\n", (int)(sizeof b1 / 4), (int)(sizeof b2 / 4), (int)(sizeof b3 / 4),
           (int)(sizeof b4 / 4), (int)(sizeof b5 / 4), (int)(sizeof b6 / 4), (int)(sizeof b7 / 4), (int)(sizeof b8 / 4),
           (int)(sizeof b9 / 4), (int)(sizeof b10 / 4));
    printf("%d %d %d %d %d %d %d %d %d %d\n", (int)(sizeof b11 / 4), (int)(sizeof b12 / 4), (int)(sizeof b13 / 4),
           (int)(sizeof b14 / 4), (int)(sizeof b15 / 4), (int)(sizeof b16 / 4), (int)(sizeof b17 / 4),
           (int)(sizeof b18 / 4), (int)(sizeof b19 / 4), (int)(sizeof b20 / 4));
    printf("%d %d %d %d\n", E1, E2, E3, E4);
    printf("%g %g %d %d %g %g %d %d %d\n", f1, f2, f3, f4, f5, f6, f7, f8, f9);
    printf("%d %d %d %d %d %d %g %g %g %g\n", f10, f11, f12, f13, f14, f15, f16, f17, f18, f19);
    printf("%g %g %g %Lg %d %g %g\n", f20, f21, f22, f23, f24, f25, f26);
    bytes("u1", &u1, sizeof u1);
    bytes("u2", &u2, sizeof u2);
    bytes("u3", &u3, 10);
    bytes("u3t", (char *)&u3 + 16, 16);
    bytes("u4", &u4, sizeof u4);
    bytes("u5", &u5, 5);
    bytes("u6", &u6, sizeof u6);
    bytes("u7", &u7, sizeof u7);
    bytes("u8", &u8, sizeof u8);
    printf("%d %d %d\n", s1.a, s1.u.p == &g, s1.b);
    printf("%u %d %d\n", s2.a, s2.in.u.p == (char *)&g + 2, s2.in.k);
    printf("%d %lld %lld %llx %llx %lld %d\n", p1 == 0, (long long)p2, (long long)p3, (long long)p4, p5, (long long)p6, p7 == 0);
    printf("%lld %llu %lld %lld %lld %lld %lld %lld\n", o1, o2, (long long)o3, (long long)o4, (long long)o5, (long long)o6, o7[0], o7[1]);
    return 0;
}
