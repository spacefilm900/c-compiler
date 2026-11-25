/* N1570 7.1.2 and 4p6 the freestanding headers (float.h, iso646.h, limits.h, stdalign.h, stdarg.h, stdbool.h, stddef.h, stdint.h, stdnoreturn.h) */
#include <float.h>
#include <iso646.h>
#include <limits.h>
#include <stdalign.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdnoreturn.h>
/* 7.1.2p4: every header may be included more than once */
#include <stddef.h>
#include <stdint.h>
#include "check.h"
#include <stdlib.h>

struct off { char c; double d; int arr[3]; };
static int vsum(int n, ...) {
    va_list ap, copy;
    va_start(ap, n);
    va_copy(copy, ap);
    int s = 0;
    for (int i = 0; i < n; i++) s += va_arg(ap, int);
    for (int i = 0; i < n; i++) s += va_arg(copy, int);
    va_end(copy);
    va_end(ap);
    return s;
}
noreturn static void stop(void) { exit(0); }

int main(void) {
    /* iso646.h */
    CHECK((1 and 1) && (0 or 1) && (not 0) && (6 bitand 3) == 2 && (6 bitor 1) == 7 && (6 xor 3) == 5 && (compl 0) == -1);
    int v = 1; v and_eq 1; v or_eq 2; v xor_eq 1; CHECK(v == 2 && (v not_eq 3));
    /* stdbool.h */
    bool b = true;
    CHECK(b == 1 && false == 0 && __bool_true_false_are_defined == 1 && sizeof(bool) == sizeof(_Bool));
    /* stddef.h */
    CHECK(offsetof(struct off, c) == 0 && offsetof(struct off, arr[1]) == offsetof(struct off, arr) + sizeof(int));
    CHECK(NULL == (void *)0 && sizeof(size_t) == sizeof(sizeof 0) && sizeof(ptrdiff_t) == sizeof((char *)0 - (char *)0));
    CHECK(_Alignof(max_align_t) >= _Alignof(long double));
    wchar_t wc = L'x';
    CHECK(wc == L'x');
    /* stdint.h */
    CHECK(sizeof(int8_t) == 1 && sizeof(int16_t) == 2 && sizeof(int32_t) == 4 && sizeof(int64_t) == 8);
    CHECK(INT8_MIN == -128 && INT16_MAX == 32767 && UINT32_MAX == 4294967295u && INT64_MIN == -INT64_MAX - 1);
    CHECK(sizeof(int_least8_t) >= 1 && sizeof(int_fast32_t) >= 4 && sizeof(intptr_t) == sizeof(void *) && sizeof(intmax_t) >= 8);
    CHECK(INT32_C(5) == 5 && UINT64_C(1) << 63 != 0 && INTMAX_C(1) == 1 && _Generic(UINT8_C(1), int: 1, unsigned: 1, default: 0));
    CHECK(SIZE_MAX >= 65535u && PTRDIFF_MAX >= 65535 && SIG_ATOMIC_MAX >= 127 && WCHAR_MAX >= 127 && WINT_MAX >= 255);
    printf("%d %d %d\n", (int)sizeof(int_fast8_t), (int)sizeof(int_fast16_t), (int)sizeof(uint_fast64_t));
    /* stdalign.h */
    alignas(16) char buf[16];
    CHECK(alignof(int) == _Alignof(int) && ((uintptr_t)buf & 15) == 0 && __alignas_is_defined && __alignof_is_defined);
    /* stdarg.h */
    CHECK(vsum(3, 1, 2, 3) == 12);
    /* float.h, limits.h: see 5.2.4.2 */
    CHECK(FLT_RADIX == 2 && CHAR_BIT == 8);
    int r = DONE();
    if (r)
        return r;
    stop();
}
