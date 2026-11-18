int printf(const char *, ...);

/* The second eightbyte of each union is the upper half of a long double
   alone (X87UP not after X87): memory in the psABI, an SSE double on Intel
   macOS. */
typedef union { long double d; void *p; } A;
typedef union { long double d; int i; } B;

void take(int a, A x, double f, B y, int b) {
    printf("take %d %p %d %g %d\n", a, x.p, y.i, f, b);
}

void many(double d0, double d1, double d2, double d3, double d4, double d5, double d6,
          A x, double d7, double d8, long tail) {
    printf("many %g %g %p %g %g %ld\n", d0, d6, x.p, d7, d8, tail);
}

A make(long v) {
    A a = {0};
    a.p = (void *)v;
    return a;
}

B make_b(int v) {
    B b = {0};
    b.i = v;
    return b;
}
