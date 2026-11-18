/* Bit-fields inside a nested struct that starts in the middle of an
   eightbyte. On x86-64, Outer is two INTEGER eightbytes and Zero is
   <2 x float>, double (the `int : 0` has no storage), fixed or variadic.
   tools/abi_fuzz.py seed 71 found Outer passed in memory; review found the
   zero-width case. */
int printf(const char *, ...);

typedef struct { int b : 4; } Bits;
typedef struct { char a[2]; Bits s; long m; } Outer;
typedef struct { double f0, f1, f2; } Big;
typedef struct { float x; struct { int : 0; float f; } s; double y; } Zero;

void show(Outer o, Big g, Zero z) {
    printf("show %d %d %d %ld %g %g %g %g\n", o.a[0], o.a[1], o.s.b, o.m, g.f2,
           z.x, z.s.f, z.y);
}

void vshow(int n, ...) {
    __builtin_va_list ap;
    __builtin_va_start(ap, n);
    Outer o = __builtin_va_arg(ap, Outer);
    Big g = __builtin_va_arg(ap, Big);
    Zero z = __builtin_va_arg(ap, Zero);
    printf("vshow %d %d %d %d %ld %g %g %g %g\n", n, o.a[0], o.a[1], o.s.b, o.m,
           g.f2, z.x, z.s.f, z.y);
    __builtin_va_end(ap);
}

Outer make(int k) {
    Outer o = {{(char)(k + 1), (char)(k + 2)}, {k - 7}, 1000L * k};
    return o;
}

Zero make_zero(float k) {
    Zero z = {k, {k + 1.0f}, k * 2.0};
    return z;
}
