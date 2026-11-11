#include <stdio.h>
struct In { int a, b; };
union V { int i; struct In in; double d; };
struct O { int k; union V v; };
struct In arr[] = { [1] = 3, 4, 5 };
struct In arr2[] = { [0].a = 1, 2, 3 };
union U { struct In in; double d; } ua[] = { [2] = 1, 2, [0].d = 1.5 };
struct T { int x[2]; union U u; } t[] = { [1].u = 7, 8, 9 };
struct O g4 = { .v = { .d = 2.5 } };
struct O g5 = { 1, { .d = 2.5 } };
struct O g6[2] = { [1].v.d = 0.5, [0].v.in.b = 3 };
int main(void) {
    printf("%zu %d %d %d\n", sizeof arr / sizeof *arr, arr[1].a, arr[1].b, arr[2].a);
    printf("%zu %d %d %d\n", sizeof arr2 / sizeof *arr2, arr2[0].a, arr2[0].b, arr2[1].a);
    printf("%zu %d %d %f\n", sizeof ua / sizeof *ua, ua[2].in.a, ua[2].in.b, ua[0].d);
    printf("%zu %d %d %d\n", sizeof t / sizeof *t, t[1].u.in.a, t[1].u.in.b, t[2].x[0]);
    printf("%f %f %f %d\n", g4.v.d, g5.v.d, g6[1].v.d, g6[0].v.in.b);
    struct In l[] = { [1] = 3, 4, 5 };
    struct O l4 = { .v = { .d = 2.5 } };
    printf("%zu %d %f\n", sizeof l / sizeof *l, l[2].a, l4.v.d);
    return 0;
}
