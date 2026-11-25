/* N1570 5.2.4.2 numerical limits: <limits.h> and <float.h> */
#include "check.h"
#include <limits.h>
#include <float.h>

int main(void) {
    CHECK(CHAR_BIT >= 8);
    CHECK(SCHAR_MIN <= -127 && SCHAR_MAX >= 127 && UCHAR_MAX >= 255);
    CHECK(SHRT_MAX >= 32767 && USHRT_MAX >= 65535);
    CHECK(INT_MAX >= 32767 && UINT_MAX >= 65535);
    CHECK(LONG_MAX >= 2147483647L && ULONG_MAX >= 4294967295UL);
    CHECK(LLONG_MAX >= 9223372036854775807LL && ULLONG_MAX >= 18446744073709551615ULL);
    CHECK(MB_LEN_MAX >= 1);
    CHECK((CHAR_MIN == 0) == ((char)-1 > 0));
#if CHAR_MIN == 0
    CHECK(CHAR_MAX == UCHAR_MAX);
#else
    CHECK(CHAR_MIN == SCHAR_MIN && CHAR_MAX == SCHAR_MAX);
#endif
    CHECK(FLT_RADIX >= 2 && FLT_DIG >= 6 && DBL_DIG >= 10 && LDBL_DIG >= 10);
    CHECK(FLT_MAX >= 1e37 && DBL_MAX >= 1e37 && LDBL_MAX >= 1e37);
    CHECK(FLT_EPSILON <= 1e-5 && DBL_EPSILON <= 1e-9 && LDBL_EPSILON <= 1e-9);
    CHECK(1.0f + FLT_EPSILON != 1.0f && 1.0 + DBL_EPSILON != 1.0);
    printf("%d %d %d %d %d\n", FLT_EVAL_METHOD, DECIMAL_DIG, FLT_DECIMAL_DIG, DBL_DECIMAL_DIG, LDBL_DECIMAL_DIG);
    printf("%d %d %d %d\n", FLT_HAS_SUBNORM, DBL_HAS_SUBNORM, LDBL_HAS_SUBNORM, FLT_ROUNDS);
    printf("%d %d %d %d %d %d\n", FLT_MANT_DIG, DBL_MANT_DIG, LDBL_MANT_DIG, FLT_MIN_EXP, DBL_MAX_EXP, LDBL_MAX_10_EXP);
    printf("%a %a %La %a %a\n", FLT_MIN, DBL_MIN, LDBL_MIN, FLT_TRUE_MIN, DBL_TRUE_MIN);
    printf("%d %d %d %d %lld\n", SCHAR_MIN, SHRT_MIN, INT_MIN, (int)sizeof(long), LLONG_MIN);
    return DONE();
}
