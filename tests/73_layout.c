int printf(const char *, ...);

struct A { char c; int i; };
struct B { int i; char c; };
struct C { char c; long l; };
struct E { short s; char c; };
struct F { char c; short s; };
struct G { long a; char b; int c; };
struct H { char c; long a[2]; };
struct J { char c; char a[16]; };
struct P { char c; int *p; int x; };

unsigned long off(void *base, void *field) {
    return (unsigned long)((char *)field - (char *)base);
}

int main(void) {
    struct A a;
    struct B b;
    struct C c;
    struct E e;
    struct F f;
    struct G g;
    struct H h;
    struct J j;
    struct P p;
    printf("%lu %lu %lu\n", sizeof(struct A), off(&a, &a.c), off(&a, &a.i));
    printf("%lu %lu %lu\n", sizeof(struct B), off(&b, &b.i), off(&b, &b.c));
    printf("%lu %lu %lu\n", sizeof(struct C), off(&c, &c.c), off(&c, &c.l));
    printf("%lu %lu %lu\n", sizeof(struct E), off(&e, &e.s), off(&e, &e.c));
    printf("%lu %lu %lu\n", sizeof(struct F), off(&f, &f.c), off(&f, &f.s));
    printf("%lu %lu %lu %lu\n", sizeof(struct G), off(&g, &g.a), off(&g, &g.b), off(&g, &g.c));
    printf("%lu %lu %lu\n", sizeof(struct H), off(&h, &h.c), off(&h, &h.a));
    printf("%lu %lu %lu\n", sizeof(struct J), off(&j, &j.c), off(&j, &j.a));
    printf("%lu %lu %lu %lu\n", sizeof(struct P), off(&p, &p.c), off(&p, &p.p), off(&p, &p.x));
    return 0;
}
