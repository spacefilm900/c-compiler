int printf(const char *, ...);

typedef struct { int b : 4; } Bits;
typedef struct { char a[2]; Bits s; long m; } Outer;
typedef struct { double f0, f1, f2; } Big;
typedef struct { float x; struct { int : 0; float f; } s; double y; } Zero;

void show(Outer o, Big g, Zero z);
void vshow(int n, ...);
Outer make(int k);
Zero make_zero(float k);

int main(void) {
    Outer o = {{-66, 68}, {-3}, -1247512282960137855L};
    Big g = {1.0, 2.0, 3.0};
    Zero z = {1.5f, {2.5f}, 3.5};
    show(o, g, z);
    vshow(5, o, g, z);
    Outer r = make(4);
    printf("make %d %d %d %ld\n", r.a[0], r.a[1], r.s.b, r.m);
    Zero q = make_zero(0.25f);
    printf("make_zero %g %g %g\n", q.x, q.s.f, q.y);
    return 0;
}
