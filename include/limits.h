/* Compiler-provided <limits.h> for the LP64 targets (x86_64 Linux,
 * Apple arm64 and x86_64, AArch64 Linux) and 64-bit Windows (LLP64).
 *
 * This is the header a hosted compiler supplies. It is searched before
 * the C library, so a program's `#include <limits.h>` gets these macros.
 * `char` is signed unless the target defines __CHAR_UNSIGNED__ (AArch64
 * Linux). `long long` is 64 bits; `long` is 64 bits too, except on
 * 64-bit Windows (_WIN64), where it is 32.
 */
#ifndef _LIMITS_H
#define _LIMITS_H

/* On Apple targets other SDK headers include <machine/limits.h> directly,
 * which defines these macros with different spellings (CHAR_MAX as 127,
 * MB_LEN_MAX as 6). Two different definitions of one macro are a constraint
 * violation (C11 6.10.3p2), so pull the SDK's <limits.h> in first (it also
 * carries the POSIX limits) and then replace its values with ours. The SDK's
 * MB_LEN_MAX (6, the longest UTF-8 sequence its locales produce) is kept,
 * since it describes the C library rather than the compiler. */
/* MinGW-w64's <limits.h> is pulled in the same way on Windows, for the
 * limits it adds (PATH_MAX, _I64_MAX, SSIZE_MAX and so on). */
#if (defined(__APPLE__) || defined(_WIN32)) && defined(__has_include_next)
#if __has_include_next(<limits.h>)
#include_next <limits.h>
#endif
#undef CHAR_BIT
#undef SCHAR_MIN
#undef SCHAR_MAX
#undef UCHAR_MAX
#undef CHAR_MIN
#undef CHAR_MAX
#undef SHRT_MIN
#undef SHRT_MAX
#undef USHRT_MAX
#undef INT_MIN
#undef INT_MAX
#undef UINT_MAX
#undef LONG_MIN
#undef LONG_MAX
#undef ULONG_MAX
#undef LLONG_MIN
#undef LLONG_MAX
#undef ULLONG_MAX
#endif

#define CHAR_BIT 8
#ifndef MB_LEN_MAX
#define MB_LEN_MAX 16
#endif

#define SCHAR_MIN (-128)
#define SCHAR_MAX 127
#define UCHAR_MAX 255
#ifdef __CHAR_UNSIGNED__
#define CHAR_MIN 0
#define CHAR_MAX UCHAR_MAX
#else
#define CHAR_MIN SCHAR_MIN
#define CHAR_MAX SCHAR_MAX
#endif

#define SHRT_MIN (-32767 - 1)
#define SHRT_MAX 32767
#define USHRT_MAX 65535

#define INT_MIN (-2147483647 - 1)
#define INT_MAX 2147483647
#define UINT_MAX 4294967295U

#ifdef _WIN64
#define LONG_MIN (-2147483647L - 1L)
#define LONG_MAX 2147483647L
#define ULONG_MAX 4294967295UL
#else
#define LONG_MIN (-9223372036854775807L - 1L)
#define LONG_MAX 9223372036854775807L
#define ULONG_MAX 18446744073709551615UL
#endif

#define LLONG_MIN (-9223372036854775807LL - 1LL)
#define LLONG_MAX 9223372036854775807LL
#define ULLONG_MAX 18446744073709551615ULL

#endif
