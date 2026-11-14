/* Compiler-provided <float.h>.
   float is IEEE binary32 and double is IEEE binary64 on every target.
   long double is the x87 80-bit extended format on x86_64 Linux, the
   binary64 format on Apple arm64 (__LDBL_MANT_DIG__ == 53), and IEEE
   binary128 on AArch64 Linux (__LDBL_MANT_DIG__ == 113). The long double
   macros use an L suffix so they have the right type. */
#ifndef _FLOAT_H
#define _FLOAT_H

#define FLT_ROUNDS 1
#define FLT_EVAL_METHOD 0
#define FLT_RADIX 2

#define FLT_MANT_DIG 24
#define DBL_MANT_DIG 53

#define FLT_DIG 6
#define DBL_DIG 15

#define FLT_MIN_EXP (-125)
#define DBL_MIN_EXP (-1021)

#define FLT_MIN_10_EXP (-37)
#define DBL_MIN_10_EXP (-307)

#define FLT_MAX_EXP 128
#define DBL_MAX_EXP 1024

#define FLT_MAX_10_EXP 38
#define DBL_MAX_10_EXP 308

#define FLT_MAX 0x1.fffffep+127f
#define FLT_MIN 0x1p-126f
#define FLT_EPSILON 0x1p-23f
#define FLT_TRUE_MIN 0x1p-149f
#define FLT_DECIMAL_DIG 9
#define FLT_HAS_SUBNORM 1

#define DBL_MAX 0x1.fffffffffffffp+1023
#define DBL_MIN 0x1p-1022
#define DBL_EPSILON 0x1p-52
#define DBL_TRUE_MIN 0x0.0000000000001p-1022
#define DBL_DECIMAL_DIG 17
#define DBL_HAS_SUBNORM 1

#if __LDBL_MANT_DIG__ == 53
#define LDBL_MANT_DIG 53
#define LDBL_DIG 15
#define LDBL_MIN_EXP (-1021)
#define LDBL_MIN_10_EXP (-307)
#define LDBL_MAX_EXP 1024
#define LDBL_MAX_10_EXP 308
#define LDBL_MAX 0x1.fffffffffffffp+1023L
#define LDBL_MIN 0x1p-1022L
#define LDBL_EPSILON 0x1p-52L
#define LDBL_TRUE_MIN 0x0.0000000000001p-1022L
#define LDBL_DECIMAL_DIG 17
#define LDBL_HAS_SUBNORM 1
#define DECIMAL_DIG 17
#elif __LDBL_MANT_DIG__ == 113
#define LDBL_MANT_DIG 113
#define LDBL_DIG 33
#define LDBL_MIN_EXP (-16381)
#define LDBL_MIN_10_EXP (-4931)
#define LDBL_MAX_EXP 16384
#define LDBL_MAX_10_EXP 4932
#define LDBL_MAX 1.18973149535723176508575932662800702e+4932L
#define LDBL_MIN 3.36210314311209350626267781732175260e-4932L
#define LDBL_EPSILON 1.92592994438723585305597794258492732e-34L
#define LDBL_TRUE_MIN 6.47517511943802511092443895822764655e-4966L
#define LDBL_DECIMAL_DIG 36
#define LDBL_HAS_SUBNORM 1
#define DECIMAL_DIG 36
#else
#define LDBL_MANT_DIG 64
#define LDBL_DIG 18
#define LDBL_MIN_EXP (-16381)
#define LDBL_MIN_10_EXP (-4931)
#define LDBL_MAX_EXP 16384
#define LDBL_MAX_10_EXP 4932
#define LDBL_MAX 1.18973149535723176502e+4932L
#define LDBL_MIN 3.36210314311209350626e-4932L
#define LDBL_EPSILON 1.08420217248550443401e-19L
#define LDBL_TRUE_MIN 3.64519953188247460253e-4951L
#define LDBL_DECIMAL_DIG 21
#define LDBL_HAS_SUBNORM 1
#define DECIMAL_DIG 21
#endif

#endif
