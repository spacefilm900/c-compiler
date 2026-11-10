/* Compiler-provided <limits.h> for x86_64 System V (LP64).
 *
 * This is the header a hosted compiler supplies. It is searched before
 * the C library, so a program's `#include <limits.h>` gets these macros.
 * `char` is signed. `long` and `long long` are both 64 bits.
 */
#ifndef _LIMITS_H
#define _LIMITS_H

#define CHAR_BIT 8
#define MB_LEN_MAX 16

#define SCHAR_MIN (-128)
#define SCHAR_MAX 127
#define UCHAR_MAX 255
#define CHAR_MIN SCHAR_MIN
#define CHAR_MAX SCHAR_MAX

#define SHRT_MIN (-32767 - 1)
#define SHRT_MAX 32767
#define USHRT_MAX 65535

#define INT_MIN (-2147483647 - 1)
#define INT_MAX 2147483647
#define UINT_MAX 4294967295U

#define LONG_MIN (-9223372036854775807L - 1L)
#define LONG_MAX 9223372036854775807L
#define ULONG_MAX 18446744073709551615UL

#define LLONG_MIN (-9223372036854775807LL - 1LL)
#define LLONG_MAX 9223372036854775807LL
#define ULLONG_MAX 18446744073709551615ULL

#endif
