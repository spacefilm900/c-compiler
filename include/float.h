/* Compiler-provided <float.h> for x86_64 System V.
   float is IEEE binary32, double is IEEE binary64, and long double is
   80-bit extended precision. The long double macros use an L suffix so
   they have the right type. This compiler does not compute that value. */
#ifndef _FLOAT_H
#define _FLOAT_H

#define FLT_ROUNDS 1
#define FLT_EVAL_METHOD 0
#define FLT_RADIX 2

#define FLT_MANT_DIG 24
#define DBL_MANT_DIG 53
#define LDBL_MANT_DIG 64

#define FLT_DIG 6
#define DBL_DIG 15
#define LDBL_DIG 18

#define FLT_MIN_EXP (-125)
#define DBL_MIN_EXP (-1021)
#define LDBL_MIN_EXP (-16381)

#define FLT_MIN_10_EXP (-37)
#define DBL_MIN_10_EXP (-307)
#define LDBL_MIN_10_EXP (-4931)

#define FLT_MAX_EXP 128
#define DBL_MAX_EXP 1024
#define LDBL_MAX_EXP 16384

#define FLT_MAX_10_EXP 38
#define DBL_MAX_10_EXP 308
#define LDBL_MAX_10_EXP 4932

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

#define LDBL_MAX 1.18973149535723176502e+4932L
#define LDBL_MIN 3.36210314311209350626e-4932L
#define LDBL_EPSILON 1.08420217248550443401e-19L
#define LDBL_TRUE_MIN 3.64519953188247460253e-4951L
#define LDBL_DECIMAL_DIG 21
#define LDBL_HAS_SUBNORM 1

#define DECIMAL_DIG 21

#endif
