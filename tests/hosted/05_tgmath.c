/* Type-generic math macros (C11 7.25): result types and values against
   the selected float / double / long double functions. */

#include <tgmath.h>
#include <stdio.h>

int main(void) {
    float f = 0.25f;
    double d = 0.25;
    long double ld = 0.25L;
    int n = 0;
    int exp;
    volatile float vf = 4.0f;

    if (sizeof(sin(f)) != sizeof(float))
        return 1;
    if (sizeof(sin(d)) != sizeof(double))
        return 2;
    if (sizeof(sin(ld)) != sizeof(long double))
        return 3;
    if (sizeof(sin(n)) != sizeof(double))
        return 4;
    if (sizeof(pow(f, f)) != sizeof(float))
        return 5;
    if (sizeof(pow(f, d)) != sizeof(double))
        return 6;
    if (sizeof(pow(f, ld)) != sizeof(long double))
        return 7;
    if (sizeof(pow(f, n)) != sizeof(double))
        return 8;
    if (sizeof(fma(f, f, f)) != sizeof(float))
        return 9;
    if (sizeof(fma(f, f, d)) != sizeof(double))
        return 10;
    if (sizeof(atan2(f, ld)) != sizeof(long double))
        return 11;
    if (sizeof(fabs(f)) != sizeof(float) || sizeof(fabs(n)) != sizeof(double))
        return 12;
    if (sizeof(ldexp(f, 1)) != sizeof(float) || sizeof(frexp(d, &exp)) != sizeof(double))
        return 13;
    if (sizeof(nexttoward(f, ld)) != sizeof(float))
        return 14;
    if (sizeof(ceil(f)) != sizeof(float) || sizeof(sqrt(n)) != sizeof(double))
        return 15;

    if (fabs(sinf(f) - sin(f)) > 0.0f)
        return 16;
    if (fabs(sin(d) - (double)sinf((float)d)) > 0.05)
        return 17;
    if (sqrt(vf) < 1.9f || sqrt(vf) > 2.1f)
        return 18;
    if (pow(2.0f, 3.0f) < 7.9f || pow(2.0f, 3.0f) > 8.1f)
        return 19;
    if (fma(2.0f, 3.0f, 4.0f) < 9.9f || fma(2.0f, 3.0f, 4.0f) > 10.1f)
        return 20;
    if (fabs(copysign(-2.0, 1.0) - 2.0) > 0.0)
        return 21;
    if (ilogb(8.0f) != 3)
        return 22;
    if (ldexp(1.5f, 1) < 2.9f || ldexp(1.5f, 1) > 3.1f)
        return 23;

    printf("%d %d %d %d\n", (int)sizeof(sin(f)), (int)sizeof(sin(d)),
           (int)sizeof(sin(ld)), (int)sizeof(sin(1)));
    return 0;
}
