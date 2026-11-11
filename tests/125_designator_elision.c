int printf(const char *, ...);
struct In { int a, b; };
union V { int i; struct In in; double d; };
struct O { int k; struct In in; int arr[3]; union V v; int z; };
struct O g1 = { .in = 1, 2, .z = 9 };
struct O g2 = { .arr = 4, 5, .v = 7 };
struct O g3 = { .v.in = 1, 2, .k = 3 };
struct O g4 = { .v = { .d = 2.5 } };
union V gu = { .in = 3, 4 };
union V gu2 = { .d = 1.25 };
struct W { struct O o[2]; } gw = { .o[1].in = 5, 6, 7 };
static void show(const char *n, struct O *o) {
    printf("%s %d %d %d %d %d %d %d %d\n", n, o->k, o->in.a, o->in.b, o->arr[0], o->arr[1], o->arr[2], o->v.i, o->z);
}
int main(void) {
    show("g1", &g1); show("g2", &g2); show("g3", &g3);
    printf("%f %d %d %f\n", g4.v.d, gu.in.a, gu.in.b, gu2.d);
    printf("%d %d %d\n", gw.o[1].in.a, gw.o[1].in.b, gw.o[1].arr[0]);
    struct O l1 = { .in = 1, 2, .z = 9 };
    struct O l2 = { .arr = 4, 5, .v = 7 };
    struct O l3 = { .v.in = 1, 2, .k = 3 };
    union V lu = { .in = 3, 4 };
    struct W lw = { .o[1].in = 5, 6, 7 };
    show("l1", &l1); show("l2", &l2); show("l3", &l3);
    printf("%d %d\n", lu.in.a, lu.in.b);
    printf("%d %d %d\n", lw.o[1].in.a, lw.o[1].in.b, lw.o[1].arr[0]);
    struct O z = { .in = {0} };
    struct O z2 = { .in = 0 };
    show("z", &z); show("z2", &z2);
    return 0;
}
