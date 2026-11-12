/* The C11 7.12.14 comparison macros from <math.h>. glibc expands them
   to __builtin_isgreater and the other GCC comparison builtins. The
   macros are quiet: comparing a quiet NaN raises no "invalid"
   floating-point exception (7.12.14p1), unlike the relational
   operators, so FE_INVALID must stay clear. */

#include <fenv.h>
#include <float.h>
#include <math.h>
#include <stdio.h>

static int row(double a, double b) {
    return isgreater(a, b) << 5 | isgreaterequal(a, b) << 4 | isless(a, b) << 3
        | islessequal(a, b) << 2 | islessgreater(a, b) << 1 | isunordered(a, b);
}

int main(void) {
    volatile double x = 1.25, y = -7.5, qnan = NAN, inf = INFINITY;
    volatile float fx = 1.25f, fnan = NAN;
    volatile long double lx = 1.25L, lmax = LDBL_MAX, lnan = NAN;
    int rows[6];

    feclearexcept(FE_ALL_EXCEPT);
    rows[0] = row(qnan, x);
    rows[1] = row(y, qnan);
    rows[2] = isless(fnan, fx) | isgreater(lnan, lx) | islessgreater(fnan, lnan);
    rows[3] = isunordered(lnan, fx) + isunordered(fnan, qnan);
    if (fetestexcept(FE_INVALID))
        return 1;

    rows[4] = row(x, y);
    rows[5] = row(-inf, inf);
    if (rows[0] != 0x01 || rows[1] != 0x01 || rows[2] != 0 || rows[3] != 2)
        return 2;
    if (rows[4] != 0x32 || rows[5] != 0x0e)
        return 3;
    /* Mixed precision: compare in long double without rounding. */
    if (!isgreater(lmax, (double)DBL_MAX) || !islessequal(fx, lx) || islessgreater(fx, x))
        return 4;
    if (row(0.0, -0.0) != 0x14)
        return 5;
    printf("%d %d %d %d %d %d\n", rows[0], rows[1], rows[2], rows[3], rows[4], rows[5]);
    return 0;
}
