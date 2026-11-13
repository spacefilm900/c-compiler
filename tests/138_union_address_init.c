/* File-scope initializers that put an address constant (C11 6.6p9) in a
   union member. 6.7.9p17 initializes the first named member when there
   is no designator. Reduced from DOOM's info.c, whose state table holds
   a union of action function pointers inside each struct element. */

int printf(const char *, ...);

int x = 5;
int y = 7;
static void act_a(void) { printf("a\n"); }
static void act_b(void *p) { printf("b %d\n", *(int *)p); }

typedef union { void (*v)(void); void (*p1)(void *); } action;
typedef struct { int sprite; int tics; action act; int next; } state;

/* DOOM's pattern: a null action and real actions in one array. */
static state states[] = {
    {0, -1, {0}, 0},
    {1, 4, {act_a}, 2},
    {2, 6, {(void (*)(void))act_b}, 0},
};

/* A top-level union. */
union U { int *p; long n; } u = {&x};
/* A designator for a member that is not the first. */
union V { long n; int *p; char c[12]; } v = {.p = &y};
/* A union inside a union, then a struct around it. */
union W { union U inner; double d; } w = {{&y}};
struct Holder { char tag; union V v; short s; } holders[2] = {{'h', {.p = &x}, 3}, {'i', {4}, 5}};
/* A later designator replaces the earlier member of the same union. */
union V later = {.p = &x, .n = 9};
union V again = {.n = 9, .p = &y};
/* A struct with a pointer and an int inside a union's first member. */
union Pair { struct { int *p; int n; } s; long l[3]; } pair = {{&x, 11}};
/* A bit-field record with an address in a plain member. */
struct Bits { unsigned a : 3; int *p; unsigned b : 5; } bits = {5, &y, 17};
/* An address converted to an integer member (6.6p10 lets an
   implementation accept this form; Clang does). */
union Q { long n; int *p; } q = {(long)&x};
struct R { char c; union Q u; } r[2] = {{1, {(long)&y}}, {2, {5}}};

int main(void)
{
    int i;
    for (i = 0; i < 3; i++) {
        printf("%d %d %d\n", states[i].sprite, states[i].tics, states[i].next);
        if (i == 1)
            states[i].act.v();
        if (i == 2)
            states[i].act.p1(&x);
    }
    printf("%d\n", states[0].act.v == 0);
    printf("%d %d %d\n", *u.p, *v.p, *w.inner.p);
    printf("%c %d %d %c %ld %d\n", holders[0].tag, *holders[0].v.p, holders[0].s,
           holders[1].tag, holders[1].v.n, holders[1].s);
    printf("%ld %d\n", later.n, *again.p);
    printf("%d %d\n", *pair.s.p, pair.s.n);
    printf("%u %d %u\n", bits.a, *bits.p, bits.b);
    printf("%d %d %d %ld\n", *(int *)q.n, r[0].c, *(int *)r[0].u.n, r[1].u.n);
    return 0;
}
