/* N1570 7.2-7.14 assert.h, ctype.h, errno.h, fenv.h, inttypes.h, locale.h, math.h, setjmp.h, signal.h */
#include <assert.h>
#include <ctype.h>
#include <errno.h>
#include <fenv.h>
#include <inttypes.h>
#include <locale.h>
#include <math.h>
#include <setjmp.h>
#include <signal.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#include "check.h"

static jmp_buf env;
static volatile sig_atomic_t got;
static void handler(int sig) { got = sig; }
static void jump(int v) { longjmp(env, v); }

int main(void) {
    #pragma STDC FENV_ACCESS ON
    static_assert(sizeof(int) >= 2, "assert.h static_assert");
    assert(1);
    CHECK(isalpha('a') && isdigit('7') && !isspace('x') && toupper('q') == 'Q' && tolower('Q') == 'q' && isxdigit('F'));
    errno = 0;
    CHECK(strtol("99999999999999999999999", 0, 10) == LONG_MAX && errno == ERANGE);
    CHECK(EDOM != 0 && EILSEQ != 0);
    CHECK(fegetround() == FE_TONEAREST);
    feclearexcept(FE_ALL_EXCEPT);
    volatile double z = 0.0;
    volatile double q = 1.0 / z;
    CHECK(isinf(q) && fetestexcept(FE_DIVBYZERO));
    int64_t big = -INT64_C(1234567890123);
    char buf[64];
    snprintf(buf, sizeof buf, "%" PRId64 " %" PRIu32 " %" PRIxMAX, big, (uint32_t)7, (uintmax_t)255);
    CHECK(strcmp(buf, "-1234567890123 7 ff") == 0);
    CHECK(imaxabs(-5) == 5 && strtoimax("42", 0, 10) == 42);
    CHECK(setlocale(LC_ALL, "C") != 0 && localeconv()->decimal_point[0] == '.');
    /* math.h: functions and the type-generic classification macros */
    CHECK(fabs(-2.0) == 2.0 && sqrt(16.0) == 4.0 && floor(-1.5) == -2.0 && fmod(7.0, 3.0) == 1.0);
    CHECK(isnan(NAN) && isinf(INFINITY) && !isfinite(HUGE_VAL) && fpclassify(0.0) == FP_ZERO && fpclassify(1.0f) == FP_NORMAL);
    CHECK(signbit(-0.0) && !signbit(0.0f) && isgreater(2.0, 1.0) && isunordered(NAN, 1.0) && isnormal(1.0L));
    CHECK(sqrtf(4.0f) == 2.0f && fabsl(-1.0L) == 1.0L && lround(2.5) == 3 && fma(2.0, 3.0, 1.0) == 7.0);
    printf("%.17g %.9g %.17g\n", exp(1.0), (double)sinf(1.0f), atan2(1.0, 1.0));
    CHECK(math_errhandling & (MATH_ERRNO | MATH_ERREXCEPT));
    /* setjmp.h */
    volatile int count = 0;
    volatile int r = -1;
    switch (setjmp(env)) {              /* 7.13.1.1p4: a permitted context */
    case 0:
        count++;
        jump(3);
        break;
    case 3:
        count++;
        r = 3;
        break;
    default:
        r = -2;
        break;
    }
    CHECK(r == 3 && count == 2);
    /* signal.h */
    CHECK(signal(SIGINT, handler) != SIG_ERR);
    CHECK(raise(SIGINT) == 0 && got == SIGINT);
    return DONE();
}
