/* N1570 6.5.3 unary operators, 6.5.4 cast operators */
#include "check.h"
#include <stddef.h>

static int evals;
static int side(void) { return ++evals; }

int main(void) {
    int x = 5, *p = &x;
    CHECK(*&x == 5 && &*p == p && *p == 5);
    CHECK(&*(int *)0 == 0);                     /* 6.5.3.2p3: &* is a no-op */
    int a[3];
    CHECK(&a[1] == a + 1 && &*a == a);
    CHECK(-x == -5 && +x == 5 && ~0 == -1 && !x == 0 && !0 == 1);
    CHECK(_Generic(!x, int: 1, default: 0) && _Generic(-(char)1, int: 1, default: 0));
    CHECK(++x == 6 && --x == 5);
    /* 6.5.3.4: sizeof and _Alignof */
    CHECK(sizeof x == sizeof(int) && sizeof(char) == 1 && sizeof a == 3 * sizeof(int));
    CHECK(_Generic(sizeof x, size_t: 1, default: 0));
    (void)sizeof side();
    CHECK(evals == 0);                         /* not evaluated */
    int n = 4;
    char vla[n + side()];
    CHECK(sizeof vla == 5 && evals == 1);      /* a VLA size is evaluated */
    CHECK(sizeof(int[n]) == 4 * sizeof(int));
    CHECK(_Alignof(int[3]) == _Alignof(int) && _Alignof(const int) == _Alignof(int));
    char str[] = "hello";
    CHECK(sizeof str == 6 && sizeof "hello" == 6 && sizeof(str + 0) == sizeof(char *));
    /* 6.5.4: casts */
    CHECK((char)0x141 == 0x41 && (unsigned char)-1 == 255 && (int)3.7 == 3 && (double)1 / 2 == 0.5);
    CHECK((void *)p == (void *)&x);
    (void)(char)x;
    long l = (long)(int)-1;
    CHECK(l == -1);
    return DONE();
}
