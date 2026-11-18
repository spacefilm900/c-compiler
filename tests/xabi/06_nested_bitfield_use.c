#include <stdio.h>

typedef struct { int b : 4; } Bits;
typedef struct { char a[2]; Bits s; long m; } Outer;
typedef struct { double f0, f1, f2; } Big;

void show(Outer o, Big g);
void vshow(int n, ...);
Outer make(int k);

int main(void) {
    Outer o = {{-66, 68}, {-3}, -1247512282960137855L};
    Big g = {1.0, 2.0, 3.0};
    show(o, g);
    vshow(5, o, g);
    Outer r = make(4);
    printf("make %d %d %d %ld\n", r.a[0], r.a[1], r.s.b, r.m);
    return 0;
}
