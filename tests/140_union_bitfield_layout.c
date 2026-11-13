/* A union nested in a struct, whose first member holds a bit-field
   record, initialized through a later member with an address (C11
   6.7.9p17, 6.7.2.1p16). The union's object representation must keep its
   full size, so a later field stays at its C offset and the object keeps
   its size. Found by review of the union-address image code added for
   DOOM. */

int printf(const char *, ...);

int x = 21;
int y = 22;

struct B { unsigned f : 1; };
struct A { struct B b; };
union U { struct A a; int *p; };
struct O { union U u; int tail; } o = {.u.p = &x, .tail = 7};

/* The bit-field record first, then the address member. */
struct O o2 = {.u.a.b.f = 1, .tail = 8};
/* A bit-field record that is smaller than the pointer, in an array. */
struct O oa[2] = {{.u.p = &y, .tail = 1}, {.u.a.b.f = 1, .tail = 2}};
/* A wider record: a bit-field and a pointer member inside the union. */
struct C { unsigned g : 4; int *q; };
union V { struct C c; long l; };
struct W { char h; union V v; short t; } w = {'w', {.c = {5, &y}}, 6};
struct W w2 = {'v', {.l = 77}, 4};
/* A bit-field next to a union that holds an address. */
struct M { unsigned k : 3; union U u; unsigned j : 2; } m = {5, {.p = &y}, 3};

int main(void)
{
    struct O lo = {.u.p = &x, .tail = 7};
    printf("%d %d %d\n", (int)sizeof o, *o.u.p, o.tail);
    printf("%u %d\n", o2.u.a.b.f, o2.tail);
    printf("%d %d %u %d\n", *oa[0].u.p, oa[0].tail, oa[1].u.a.b.f, oa[1].tail);
    printf("%c %u %d %d\n", w.h, w.v.c.g, *w.v.c.q, w.t);
    printf("%c %ld %d\n", w2.h, w2.v.l, w2.t);
    printf("%u %d %u\n", m.k, *m.u.p, m.j);
    printf("%d %d\n", *lo.u.p, lo.tail);
    return 0;
}
