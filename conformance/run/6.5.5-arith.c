/* N1570 6.5.5-6.5.15: multiplicative to conditional operators */
#include "check.h"
#include <stddef.h>
#include <limits.h>

int main(void) {
    /* 6.5.5: division truncates toward zero; (a/b)*b + a%b == a */
    CHECK(7 / 2 == 3 && -7 / 2 == -3 && 7 / -2 == -3 && -7 / -2 == 3);
    CHECK(7 % 2 == 1 && -7 % 2 == -1 && 7 % -2 == 1 && -7 % -2 == -1);
    CHECK(7.0 / 2 == 3.5 && 7u / 2 == 3);
    /* 6.5.6: additive and pointer arithmetic */
    int a[10];
    int *p = a + 3, *q = a + 9;
    CHECK(q - p == 6 && _Generic(q - p, ptrdiff_t: 1, default: 0));
    CHECK(p + 6 == q && 6 + p == q && q - 6 == p && a + 10 > p);   /* one past the end */
    CHECK(UINT_MAX + 1u == 0);
    /* 6.5.7: shifts; the type is the promoted left operand */
    CHECK(1 << 3 == 8 && 256 >> 4 == 16 && (1u << 31) == 0x80000000u);
    CHECK(_Generic((char)1 << 1L, int: 1, default: 0) && _Generic(1L << 1, long: 1, default: 0));
    printf("%d %u\n", -16 >> 2, 0x80000000u >> 31);    /* right shift of a negative value is implementation-defined */
    /* 6.5.8, 6.5.9: relational and equality, result is int 0 or 1 */
    CHECK((3 < 4) == 1 && (3 > 4) == 0 && (3 <= 3) == 1 && (3 >= 4) == 0 && (2 == 2) == 1 && (2 != 2) == 0);
    CHECK(_Generic(1 < 2, int: 1, default: 0) && _Generic(1.0 == 2.0, int: 1, default: 0));
    CHECK(p < q && (void *)p != (void *)q && p != 0);
    struct { int x, y; } s;
    CHECK(&s.x < &s.y);                         /* later members compare greater */
    /* 6.5.10-12: bitwise */
    CHECK((0xF0 & 0x3C) == 0x30 && (0xF0 ^ 0x3C) == 0xCC && (0xF0 | 0x0F) == 0xFF);
    /* 6.5.13, 6.5.14: logical, short-circuit, sequence point */
    int i = 0;
    CHECK((0 && ++i) == 0 && i == 0 && (1 || ++i) == 1 && i == 0);
    CHECK((i++ == 0 && i == 1) == 1);
    CHECK(_Generic(1.0 && 2, int: 1, default: 0) && (0.5 || 0) == 1 && (p && 0) == 0);
    /* 6.5.15: conditional; only one of the second and third operands is evaluated */
    int e = 0;
    int r = 1 ? 10 : ++e;
    CHECK(r == 10 && e == 0);
    CHECK(_Generic(1 ? 1 : 1.0, double: 1, default: 0));
    CHECK(_Generic(1 ? (void *)0 : p, int *: 1, default: 0));     /* a null pointer constant takes the other type */
    CHECK(_Generic(1 ? (void *)p : p, void *: 1, default: 0));
    const int *cp = 0;
    CHECK(_Generic(1 ? cp : p, const int *: 1, default: 0));      /* qualifiers combine */
    int arr2[2] = { 1, 2 };
    CHECK((0 ? a : arr2)[1] == 2);
    (1 ? (void)0 : (void)1);
    return DONE();
}
