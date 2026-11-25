/* N1570 6.3 conversions */
#include "check.h"
#include <limits.h>
#include <stddef.h>

#define TYPE(e) _Generic((e), _Bool: "_Bool", char: "char", signed char: "schar", unsigned char: "uchar", \
    short: "short", unsigned short: "ushort", int: "int", unsigned: "unsigned", long: "long", \
    unsigned long: "ulong", long long: "llong", unsigned long long: "ullong", float: "float", \
    double: "double", long double: "ldouble", default: "other")

struct bits { unsigned u3 : 3; int i3 : 3; unsigned long ul40 : 31; _Bool b : 1; };
static int fn(void) { return 4; }

int main(void) {
    char c = 1; short s = 1; unsigned short us = 1; unsigned char uc = 1;
    struct bits b = { 1, 1, 1, 1 };
    /* 6.3.1.1: integer promotions */
    printf("%s %s %s %s %s %s\n", TYPE(+c), TYPE(+s), TYPE(+us), TYPE(+uc), TYPE(b.u3 + 0), TYPE(b.i3 + 0));
    printf("%s %s %s\n", TYPE(b.b + 0), TYPE(c + c), TYPE(~uc));
    /* 6.3.1.2: conversion to _Bool */
    CHECK((_Bool)0.5 == 1 && (_Bool)0.0 == 0 && (_Bool)(void *)&c == 1 && (_Bool)256 == 1);
    /* 6.3.1.3: to unsigned is modulo, to signed out of range is implementation-defined */
    CHECK((unsigned char)-1 == UCHAR_MAX && (unsigned)-1 == UINT_MAX && (unsigned short)70000 == 4464);
    printf("%d %d\n", (signed char)200, (short)70000);
    /* 6.3.1.4: real floating to integer truncates toward zero */
    CHECK((int)2.9 == 2 && (int)-2.9 == -2 && (unsigned)3.99f == 3);
    CHECK((double)(1LL << 53) == 9007199254740992.0);
    /* 6.3.1.5: real floating types */
    CHECK((float)0.1 != 0.1 && (double)(float)0.5 == 0.5);
    /* 6.3.1.8: usual arithmetic conversions */
    printf("%s %s %s %s\n", TYPE(1u + 1), TYPE(1L + 1u), TYPE(1ULL + 1LL), TYPE(1L + 1ULL));
    printf("%s %s %s %s\n", TYPE(1.0f + 1), TYPE(1.0 + 1.0f), TYPE(1.0L + 1.0), TYPE('a' + 1.0f));
    CHECK(-1 < 0u ? 0 : 1);   /* -1 converts to unsigned */
    CHECK((-1L < 1u) == (sizeof(long) > sizeof(unsigned)));
    /* 6.3.2.1: lvalues, arrays, function designators */
    int arr[5];
    CHECK(sizeof arr == 5 * sizeof(int) && sizeof(arr + 0) == sizeof(int *));
    CHECK(sizeof &arr == sizeof(int (*)[5]));
    CHECK((*fn)() == 4 && (**fn)() == 4 && (&fn)() == 4);
    const int ci = 3;
    printf("%s\n", TYPE(ci));
    CHECK(_Generic("abc", char *: 1, default: 0));
    /* 6.3.2.2: void */
    (void)ci;
    /* 6.3.2.3: pointers */
    const void *vp = &ci;
    const int *cip = vp;
    CHECK(*cip == 3);
    int *np = 0;
    CHECK(np == NULL && !np && (void *)0 == np);
    CHECK((char *)(void *)arr == (char *)arr);
    int (*fp)(void) = fn;
    void (*other)(void) = (void (*)(void))fp;
    CHECK(((int (*)(void))other)() == 4);    /* 6.3.2.3p8: round-tripping a function pointer */
    return DONE();
}
