/* System <math.h> NAN, INFINITY, and HUGE_VAL macros (C11 7.12p3–p5).
   INFINITY and NAN are constant expressions of type float. A quiet NaN
   converted to _Bool is 1 (6.3.1.2). glibc expands these macros to
   GCC floating-point format builtins when __GNUC__ is at least 3.3. */

#include <math.h>

static float nan_s = NAN;
static float inf_s = INFINITY;
static double hv_s = HUGE_VAL;
static float hvf_s = HUGE_VALF;
static long double hvl_s = HUGE_VALL;

_Static_assert(_Generic(NAN, float: 1, default: 0), "NAN type");
_Static_assert(_Generic(INFINITY, float: 1, default: 0), "INFINITY type");
_Static_assert(_Generic(HUGE_VAL, double: 1, default: 0), "HUGE_VAL type");
_Static_assert(_Generic(HUGE_VALF, float: 1, default: 0), "HUGE_VALF type");
_Static_assert(_Generic(HUGE_VALL, long double: 1, default: 0), "HUGE_VALL type");
_Static_assert(sizeof(NAN) == sizeof(float), "NAN size");
_Static_assert(sizeof(INFINITY) == sizeof(float), "INFINITY size");
_Static_assert(sizeof(HUGE_VAL) == sizeof(double), "HUGE_VAL size");
_Static_assert(sizeof(HUGE_VALF) == sizeof(float), "HUGE_VALF size");
_Static_assert(sizeof(HUGE_VALL) == sizeof(long double), "HUGE_VALL size");

int main(void) {
    static float nan_ls = NAN;
    static float inf_ls = INFINITY;
    static double hv_ls = HUGE_VAL;
    static float hvf_ls = HUGE_VALF;
    static long double hvl_ls = HUGE_VALL;
    volatile float n = NAN;
    volatile float i = INFINITY;
    volatile double hv = HUGE_VAL;
    volatile float hvf = HUGE_VALF;
    volatile long double hvl = HUGE_VALL;
    volatile double d = NAN;
    _Bool b = d;

    if (_Generic(NAN, float: 1, default: 0) != 1)
        return 1;
    if (_Generic(INFINITY, float: 1, default: 0) != 1)
        return 2;
    if (_Generic(HUGE_VAL, double: 1, default: 0) != 1)
        return 3;
    if (_Generic(HUGE_VALF, float: 1, default: 0) != 1)
        return 4;
    if (_Generic(HUGE_VALL, long double: 1, default: 0) != 1)
        return 5;

    if (!(nan_s != nan_s) || !(nan_ls != nan_ls) || !(n != n))
        return 6;
    if (isnan((double)nan_s) == 0 || isnan((double)n) == 0)
        return 7;

    if (!(inf_s > 0.0f) || !(inf_ls > 0.0f) || !(i > 0.0f))
        return 8;
    if (isinf((double)inf_s) == 0 || isinf((double)i) == 0)
        return 9;
    if (!(hv_s > 0.0) || !(hv_ls > 0.0) || !(hv > 0.0))
        return 10;
    if (!(hvf_s > 0.0f) || !(hvf_ls > 0.0f) || !(hvf > 0.0f))
        return 11;
    if (!(hvl_s > 0.0L) || !(hvl_ls > 0.0L) || !(hvl > 0.0L))
        return 12;
    if (isinf(hv_s) == 0 || isinf((double)hvf) == 0 || isinf((double)hvl) == 0)
        return 13;

    if (sizeof(NAN) != sizeof(float) || sizeof(INFINITY) != sizeof(float))
        return 14;
    if (sizeof(HUGE_VAL) != sizeof(double) || sizeof(HUGE_VALF) != sizeof(float)
        || sizeof(HUGE_VALL) != sizeof(long double))
        return 15;

    /* C11 6.3.1.2: a scalar converted to _Bool is 0 if the value compares
       equal to 0, otherwise 1. A NaN does not compare equal to 0. */
    if (b != 1)
        return 16;

    if (!((n + 0.0f) != (n + 0.0f)))
        return 17;
    if (!(i * 2.0f > 0.0f) || !(i + 1.0f > 0.0f))
        return 18;
    if ((1.0f / i) != 0.0f)
        return 19;
    if ((hv * 2.0) != hv || (hvf * 2.0f) != hvf)
        return 20;

    return 0;
}
