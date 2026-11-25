/* N1570 6.2.5 types, 6.2.6 representations, 6.2.8 alignment */
#include "check.h"
#include <limits.h>
#include <stddef.h>
#include <string.h>

int main(void) {
    CHECK(sizeof(char) == 1 && sizeof(signed char) == 1 && sizeof(unsigned char) == 1);
    CHECK(sizeof(short) <= sizeof(int) && sizeof(int) <= sizeof(long) && sizeof(long) <= sizeof(long long));
    CHECK(sizeof(_Bool) >= 1 && (_Bool)2 == 1);
    CHECK(sizeof(unsigned) == sizeof(int) && sizeof(unsigned long long) == sizeof(long long));
    CHECK(sizeof(float) <= sizeof(double) && sizeof(double) <= sizeof(long double));
    /* 6.2.5p9: unsigned arithmetic is reduced modulo 2^N */
    unsigned u = UINT_MAX;
    CHECK(u + 1 == 0);
    unsigned char uc = 255;
    uc++;
    CHECK(uc == 0);
    /* 6.2.6.1p4: any object can be copied as an array of unsigned char */
    double d = 1.5, e;
    unsigned char bytes[sizeof d];
    memcpy(bytes, &d, sizeof d);
    memcpy(&e, bytes, sizeof d);
    CHECK(e == 1.5);
    /* 6.2.6.2: integer representation; -1 & 3 is 3 in two's complement, ones' or sign-magnitude differ */
    printf("%d %d\n", -1 & 3, (int)sizeof(void *));
    /* 6.2.8: alignment */
    CHECK(_Alignof(max_align_t) >= _Alignof(long double) && _Alignof(max_align_t) >= _Alignof(long long));
    CHECK(_Alignof(char) == 1);
    printf("%zu %zu %zu %zu\n", _Alignof(short), _Alignof(int), _Alignof(double), _Alignof(max_align_t));
    /* 6.2.5p27-28: void * and char * have the same representation */
    CHECK(sizeof(void *) == sizeof(char *));
    /* 6.2.5p20: derived types */
    int arr[3][4];
    CHECK(sizeof arr == 12 * sizeof(int) && sizeof arr[0] == 4 * sizeof(int));
    union { int i; char c[sizeof(int)]; } un = { 0 };
    CHECK(sizeof un == sizeof(int));
    un.c[0] = 1;
    CHECK(un.i != 0);
    return DONE();
}
