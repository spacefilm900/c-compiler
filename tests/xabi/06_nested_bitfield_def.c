/* A struct whose bit-field sits in a nested struct that starts in the
   middle of an eightbyte. On x86-64 the outer struct is two INTEGER
   eightbytes (registers), fixed or variadic. tools/abi_fuzz.py seed 71
   found this compiler passing it in memory. */
#include <stdarg.h>
#include <stdio.h>

typedef struct { int b : 4; } Bits;
typedef struct { char a[2]; Bits s; long m; } Outer;
typedef struct { double f0, f1, f2; } Big;

void show(Outer o, Big g) {
    printf("show %d %d %d %ld %g\n", o.a[0], o.a[1], o.s.b, o.m, g.f2);
}

void vshow(int n, ...) {
    va_list ap;
    va_start(ap, n);
    Outer o = va_arg(ap, Outer);
    Big g = va_arg(ap, Big);
    printf("vshow %d %d %d %d %ld %g\n", n, o.a[0], o.a[1], o.s.b, o.m, g.f2);
    va_end(ap);
}

Outer make(int k) {
    Outer o = {{(char)(k + 1), (char)(k + 2)}, {k - 7}, 1000L * k};
    return o;
}
