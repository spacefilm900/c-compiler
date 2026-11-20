/* Compiler-provided <tgmath.h> (C11 7.25).
 *
 * The header includes <math.h> and defines type-generic macros for the
 * real-floating functions in 7.25. __STDC_NO_COMPLEX__ is predefined, so
 * <complex.h> is not included and the macros dispatch only to the real
 * float / double / long double functions. Type selection follows 7.25p3:
 * long double if any generic argument has that type; otherwise double if
 * any generic argument has type double or an integer type; otherwise
 * float.
 */
#ifndef _TGMATH_H
#define _TGMATH_H

#include <math.h>

#define __tg_real_type(x) _Generic((x), \
    long double: (long double)0, \
    default: _Generic((x), \
        float: (float)0, \
        default: (double)0))

#define __tg_real_type2(x, y) _Generic(__tg_real_type(x), \
    long double: (long double)0, \
    default: _Generic(__tg_real_type(y), \
        long double: (long double)0, \
        default: _Generic(__tg_real_type(x), \
            float: _Generic(__tg_real_type(y), \
                float: (float)0, \
                default: (double)0), \
            default: (double)0)))

#define __tg_real_type3(x, y, z) __tg_real_type2(__tg_real_type2((x), (y)), (z))

#define __TGMATH1(fn, x) _Generic(__tg_real_type(x), \
    float: fn##f, \
    long double: fn##l, \
    default: fn)(x)

#define __TGMATH2(fn, x, y) _Generic(__tg_real_type2((x), (y)), \
    float: fn##f, \
    long double: fn##l, \
    default: fn)((x), (y))

#define __TGMATH3(fn, x, y, z) _Generic(__tg_real_type3((x), (y), (z)), \
    float: fn##f, \
    long double: fn##l, \
    default: fn)((x), (y), (z))

#define __TGMATH1_EXTRA(fn, x, extra) _Generic(__tg_real_type(x), \
    float: fn##f, \
    long double: fn##l, \
    default: fn)((x), (extra))

#define acos(x) __TGMATH1(acos, (x))
#define asin(x) __TGMATH1(asin, (x))
#define atan(x) __TGMATH1(atan, (x))
#define acosh(x) __TGMATH1(acosh, (x))
#define asinh(x) __TGMATH1(asinh, (x))
#define atanh(x) __TGMATH1(atanh, (x))
#define cos(x) __TGMATH1(cos, (x))
#define sin(x) __TGMATH1(sin, (x))
#define tan(x) __TGMATH1(tan, (x))
#define cosh(x) __TGMATH1(cosh, (x))
#define sinh(x) __TGMATH1(sinh, (x))
#define tanh(x) __TGMATH1(tanh, (x))
#define exp(x) __TGMATH1(exp, (x))
#define log(x) __TGMATH1(log, (x))
#define pow(x, y) __TGMATH2(pow, (x), (y))
#define sqrt(x) __TGMATH1(sqrt, (x))
#define fabs(x) __TGMATH1(fabs, (x))

#define atan2(x, y) __TGMATH2(atan2, (x), (y))
#define cbrt(x) __TGMATH1(cbrt, (x))
#define ceil(x) __TGMATH1(ceil, (x))
#define copysign(x, y) __TGMATH2(copysign, (x), (y))
#define erf(x) __TGMATH1(erf, (x))
#define erfc(x) __TGMATH1(erfc, (x))
#define exp2(x) __TGMATH1(exp2, (x))
#define expm1(x) __TGMATH1(expm1, (x))
#define fdim(x, y) __TGMATH2(fdim, (x), (y))
#define floor(x) __TGMATH1(floor, (x))
#define fma(x, y, z) __TGMATH3(fma, (x), (y), (z))
#define fmax(x, y) __TGMATH2(fmax, (x), (y))
#define fmin(x, y) __TGMATH2(fmin, (x), (y))
#define fmod(x, y) __TGMATH2(fmod, (x), (y))
#define frexp(x, exp) __TGMATH1_EXTRA(frexp, (x), (exp))
#define hypot(x, y) __TGMATH2(hypot, (x), (y))
#define ilogb(x) __TGMATH1(ilogb, (x))
#define ldexp(x, exp) __TGMATH1_EXTRA(ldexp, (x), (exp))
#define lgamma(x) __TGMATH1(lgamma, (x))
#define llrint(x) __TGMATH1(llrint, (x))
#define llround(x) __TGMATH1(llround, (x))
#define log10(x) __TGMATH1(log10, (x))
#define log1p(x) __TGMATH1(log1p, (x))
#define log2(x) __TGMATH1(log2, (x))
#define logb(x) __TGMATH1(logb, (x))
#define lrint(x) __TGMATH1(lrint, (x))
#define lround(x) __TGMATH1(lround, (x))
#define nearbyint(x) __TGMATH1(nearbyint, (x))
#define nextafter(x, y) __TGMATH2(nextafter, (x), (y))
#define nexttoward(x, y) __TGMATH1_EXTRA(nexttoward, (x), (y))
#define remainder(x, y) __TGMATH2(remainder, (x), (y))
#define remquo(x, y, quo) _Generic(__tg_real_type2((x), (y)), \
    float: remquof, \
    long double: remquol, \
    default: remquo)((x), (y), (quo))
#define rint(x) __TGMATH1(rint, (x))
#define round(x) __TGMATH1(round, (x))
#define scalbn(x, n) __TGMATH1_EXTRA(scalbn, (x), (n))
#define scalbln(x, n) __TGMATH1_EXTRA(scalbln, (x), (n))
#define tgamma(x) __TGMATH1(tgamma, (x))
#define trunc(x) __TGMATH1(trunc, (x))

#endif
