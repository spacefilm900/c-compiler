/* A __builtin_nan tag that is not a compile-time constant is lowered
   to a call of the C library nan/nanf/nanl (C11 7.12.11.2). The
   result is a NaN. The harness links libm for every program. */

#include <math.h>

int main(void) {
    volatile double d = __builtin_nan("1xyz");
    volatile float f = __builtin_nanf("1xyz");
    volatile long double l = __builtin_nanl("1xyz");

    if (!(d != d) || isnan(d) == 0)
        return 1;
    if (!(f != f) || isnan((double)f) == 0)
        return 2;
    if (!(l != l) || isnan((double)l) == 0)
        return 3;
    /* A valid tag still produces a NaN at runtime. */
    volatile double ok = __builtin_nan("1");
    if (!(ok != ok) || isnan(ok) == 0)
        return 4;
    return 0;
}
