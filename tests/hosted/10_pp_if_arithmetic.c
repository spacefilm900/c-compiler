/* #if arithmetic (C11 6.10.1p4): every operand is intmax_t or uintmax_t,
   and one unsigned operand makes the operation unsigned, so -1 compares
   greater than 0u. Each test sets a bit in R; the program prints R. */
int printf(const char *, ...);

#define R0 0
#if -1 > 0u
#define R1 1
#else
#define R1 0
#endif
#if (0u - 1) / 2 == 0x7fffffffffffffff
#define R2 2
#else
#define R2 0
#endif
#if ~0u == 18446744073709551615u && ~0 == -1
#define R3 4
#else
#define R3 0
#endif
#if +3 == 3 && -(-3) == 3 && -1u == 0xffffffffffffffff
#define R4 8
#else
#define R4 0
#endif
#if (7u % 4u) == 3 && (7 % -4) == 3 && (-7 / 2) == -3 && (-7 % 2) == -1
#define R5 16
#else
#define R5 0
#endif
#if (6u & 3u) == 2 && (6u ^ 3u) == 5 && (6u | 3u) == 7 && (6 & 3 | 8 ^ 1) == 11
#define R6 32
#else
#define R6 0
#endif
#if 2u * 3u == 6 && 0xffffffffffffffffu * 2u == 0xfffffffffffffffe
#define R7 64
#else
#define R7 0
#endif
#if (-1 <= 0u) == 0 && (-1 >= 0u) == 1 && (1 <= 1) && (2 >= 3) == 0
#define R8 128
#else
#define R8 0
#endif
#if (1 ? -1 : 0u) > 0 && (0 ? 0u : -1) > 0 && (1 ? -1 : 0) < 0
#define R9 256
#else
#define R9 0
#endif
#if (1u << 63) > 0 && (1u << 63 >> 63) == 1 && (-8 >> 1) == -4 && (0u - 8 >> 60) == 15
#define R10 512
#else
#define R10 0
#endif
#if !defined(UNDEFINED_NAME) && (UNDEFINED_NAME + 1) == 1
#define R11 1024
#else
#define R11 0
#endif
#if 'a' == 97 && '\377' < 0 && L'\xff' > 0 && u'x' == 120
#define R12 2048
#else
#define R12 0
#endif
#if 0x7fffffffffffffff + 0 > 0 && 18446744073709551615u == -1
#define R13 4096
#else
#define R13 0
#endif

int main(void) {
    printf("%d\n", R0 + R1 + R2 + R3 + R4 + R5 + R6 + R7 + R8 + R9 + R10 + R11 + R12 + R13);
    return 0;
}
