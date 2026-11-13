/* C11 6.7.9p17-p21: a designator that names a different member of a
   union selects that member. Members of the newly selected member that
   are not named are initialized as for static storage (zero), so bytes
   or addresses written for the earlier member must not survive.
   Found by review of the union-address image code added for DOOM. */

int printf(const char *, ...);

int x = 3;
int y = 4;

struct P { int *p; int *q; };
union U { struct P a; struct P b; };

/* An address in the first member, then a later designator for b.q. */
union U u1 = {.a.p = &x, .b.q = &y};
/* Plain bytes in the first member, then the other member. */
union N { struct { long m; long n; } s; struct { long k; long l; } t; };
union N n1 = {.s.m = 11, .s.n = 12, .t.l = 13};
/* Re-selecting the same member keeps what was already written. */
union U u2 = {.a.p = &x, .a.q = &y};
/* Switching back and forth: only the last member's stores count. */
union U u3 = {.a.p = &x, .b.q = &y, .a.q = &x};
/* A union inside a struct, then a sibling field after it. */
struct S { union U u; int tail; } s1 = {.u.a.p = &x, .u.b.q = &y, .tail = 9};
/* An array of unions, each element switching members. */
union U arr[2] = {[0].a.p = &x, [0].b.q = &y, [1].b.p = &y, [1].a.q = &x};
/* A union nested in a union's member. */
union I { struct { int *p; long z; } a; struct { long w; int *q; } b; };
union O { union I i; long l[2]; } o1 = {.i.a.p = &x, .i.a.z = 5, .i.b.q = &y};
/* A bit-field record as a member. */
struct BF { unsigned f : 3; unsigned g : 5; long pad; };
union B { struct BF bf; struct { long m; long n; } s; } b1 = {.bf.f = 7, .bf.g = 9, .s.n = 1};

static void show(const char *name, union U *v)
{
    printf("%s a.p=%d a.q=%d\n", name, v->a.p ? *v->a.p : -1, v->a.q ? *v->a.q : -1);
}

int main(void)
{
    /* The same initializers with automatic storage duration. */
    union U l1 = {.a.p = &x, .b.q = &y};
    union N l2 = {.s.m = 11, .s.n = 12, .t.l = 13};
    union U l3 = {.a.p = &x, .b.q = &y, .a.q = &x};
    struct S l4 = {.u.a.p = &x, .u.b.q = &y, .tail = 9};
    union O l5 = {.i.a.p = &x, .i.a.z = 5, .i.b.q = &y};
    union B l6 = {.bf.f = 7, .bf.g = 9, .s.n = 1};
    show("u1", &u1);
    show("u2", &u2);
    show("u3", &u3);
    show("s1", &s1.u);
    printf("s1.tail=%d\n", s1.tail);
    show("arr0", &arr[0]);
    show("arr1", &arr[1]);
    printf("n1 %ld %ld\n", n1.t.k, n1.t.l);
    printf("o1 %ld %d\n", o1.i.b.w, *o1.i.b.q);
    printf("b1 %ld %ld\n", b1.s.m, b1.s.n);
    show("l1", &l1);
    show("l3", &l3);
    show("l4", &l4.u);
    printf("l4.tail=%d\n", l4.tail);
    printf("l2 %ld %ld\n", l2.t.k, l2.t.l);
    printf("l5 %ld %d\n", l5.i.b.w, *l5.i.b.q);
    printf("l6 %ld %ld\n", l6.s.m, l6.s.n);
    return 0;
}
