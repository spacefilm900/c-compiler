#include <stdio.h>

struct S {
    int a;
    union {
        int b;
        int c;
    };
    int d;
};

struct T {
    int a;
    struct {
        int b;
        int c;
    };
    int d;
};

struct V {
    union {
        struct {
            int i;
            int j;
        };
        int z;
    };
    int m;
};

int main(void) {
    struct S s = {1, 2, 3};
    printf("S %d %d %d %d\n", s.a, s.b, s.c, s.d);
    struct S s2 = {.c = 4, 5};
    printf("S2 %d %d %d\n", s2.b, s2.c, s2.d);
    struct S s3 = {.b = 6, 7};
    printf("S3 %d %d\n", s3.c, s3.d);
    s.b = 9;
    printf("set %d\n", s.c);

    struct T t = {1, 2, 3, 4};
    printf("T %d %d %d %d\n", t.a, t.b, t.c, t.d);
    struct T t2 = {.c = 5, 6};
    printf("T2 %d %d %d\n", t2.b, t2.c, t2.d);
    struct T t3 = {1, {2, 3}, 4};
    printf("T3 %d %d %d %d\n", t3.a, t3.b, t3.c, t3.d);

    struct V v = {1, 2, 3};
    printf("V %d %d %d\n", v.i, v.j, v.m);
    struct V v2 = {.z = 7, 8};
    printf("V2 %d %d\n", v2.z, v2.m);
    struct V v3 = {.j = 4, 9};
    printf("V3 %d %d %d\n", v3.i, v3.j, v3.m);
    return 0;
}
