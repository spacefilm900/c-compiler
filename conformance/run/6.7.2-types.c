/* N1570 6.7.2 type specifiers (6.7.2.1 struct and union, 6.7.2.2 enum, 6.7.2.3 tags) */
#include "check.h"
#include <stddef.h>
#include <stdlib.h>

struct flex { int n; double d[]; };
struct anon { int a; union { int b; float f; }; struct { int c, d; }; };
struct bits { unsigned a : 3, : 0, b : 4; signed int c : 5; _Bool flag : 1; int : 3; unsigned d : 32; };
enum small { NEG = -3, Z, P = 10, Q, R = P * 2 };
struct fwd;                                    /* incomplete until completed below */
struct fwd *fp;
struct fwd { int v; };
struct list { int v; struct list *next; };
union u2 { char c; int i; double d; };

int main(void) {
    long unsigned int long lu = 1;              /* specifiers in any order */
    signed s = -1; unsigned u = 1; short int si = 2; char signed cs = -1; long double ld = 1;
    int long long ill = 3;
    CHECK(sizeof lu == sizeof(unsigned long long) && s < 0 && u && si == 2 && cs == -1 && ld == 1 && ill == 3);
    CHECK(_Generic(cs, signed char: 1, default: 0) && _Generic(lu, unsigned long long: 1, default: 0));
    /* flexible array member */
    CHECK(sizeof(struct flex) == offsetof(struct flex, d));
    struct flex *f = malloc(sizeof *f + 3 * sizeof(double));
    f->n = 3; f->d[2] = 1.5;
    CHECK(f->d[2] == 1.5);
    free(f);
    /* anonymous members */
    struct anon an = { 1, { 2 }, { 3, 4 } };
    CHECK(an.a == 1 && an.b == 2 && an.c == 3 && an.d == 4);
    an.f = 1.0f;
    CHECK(an.f == 1.0f);
    /* bit-fields */
    struct bits b = { 7, 15, -16, 1, 0xffffffffu };
    CHECK(b.a == 7 && b.b == 15 && b.c == -16 && b.flag == 1 && b.d == 0xffffffffu);
    b.a = 9;                                    /* unsigned bit-fields wrap */
    CHECK(b.a == 1);
    printf("%zu %zu\n", sizeof(struct bits), _Alignof(struct bits));
    /* enumerations */
    CHECK(NEG == -3 && Z == -2 && P == 10 && Q == 11 && R == 20);
    CHECK(_Generic(Q, int: 1, default: 0));      /* enumeration constants have type int */
    printf("%zu %d\n", sizeof(enum small), (enum small)-1 < 0);
    /* tags */
    struct fwd w = { 5 };
    fp = &w;
    CHECK(fp->v == 5);
    struct list n2 = { 2, 0 }, n1 = { 1, &n2 };
    CHECK(n1.next->v == 2);
    {
        struct fwd { char other; };               /* a new type in an inner scope */
        struct fwd x = { 'q' };
        CHECK(x.other == 'q' && sizeof x == 1);
    }
    CHECK(sizeof(union u2) >= sizeof(double) && offsetof(union u2, i) == 0);
    /* 6.7.2.1p15-16: no padding at the start; members in increasing address order */
    struct { char c; int i; char d; } pad;
    CHECK((void *)&pad == (void *)&pad.c && (char *)&pad.i < (char *)&pad.d);
    printf("%zu %zu\n", offsetof(struct anon, c), sizeof pad);
    return DONE();
}
