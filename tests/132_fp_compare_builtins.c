/* The GCC comparison builtins behind the C11 7.12.14 macros
   (isgreater, isgreaterequal, isless, islessequal, islessgreater,
   isunordered). The GCC manual (Library Builtins) documents them as
   built-in versions of the ISO C99 floating-point comparison macros
   that avoid raising exceptions for unordered operands. Each takes two
   arithmetic operands, at least one of them real floating, converted
   by the usual arithmetic conversions, and yields an int 0 or 1. When
   either operand is a NaN the four ordered relations and
   islessgreater are 0 and isunordered is 1. No headers: the builtins
   are called directly. */

static int table(double a, double b) {
    return __builtin_isgreater(a, b) << 5 | __builtin_isgreaterequal(a, b) << 4
        | __builtin_isless(a, b) << 3 | __builtin_islessequal(a, b) << 2
        | __builtin_islessgreater(a, b) << 1 | __builtin_isunordered(a, b);
}

static int tablef(float a, float b) {
    return __builtin_isgreater(a, b) << 5 | __builtin_isgreaterequal(a, b) << 4
        | __builtin_isless(a, b) << 3 | __builtin_islessequal(a, b) << 2
        | __builtin_islessgreater(a, b) << 1 | __builtin_isunordered(a, b);
}

static int tablel(long double a, long double b) {
    return __builtin_isgreater(a, b) << 5 | __builtin_isgreaterequal(a, b) << 4
        | __builtin_isless(a, b) << 3 | __builtin_islessequal(a, b) << 2
        | __builtin_islessgreater(a, b) << 1 | __builtin_isunordered(a, b);
}

int main(void) {
    volatile double one = 1.0, two = 2.0, pz = 0.0, nz = -0.0;
    volatile double inf = __builtin_inf(), nan = __builtin_nan("");
    volatile float f_half = 0.5f, f_nan = __builtin_nanf("");
    volatile long double l_three = 3.0L, l_nan = __builtin_nanl("");
    int r;

    /* gt ge lt le lg un */
    if (table(two, one) != 0x32)
        return 1;
    if (table(one, two) != 0x0e)
        return 2;
    if (table(one, one) != 0x14)
        return 3;
    if (table(pz, nz) != 0x14 || table(nz, pz) != 0x14)
        return 4;
    if (table(inf, one) != 0x32 || table(-inf, one) != 0x0e || table(inf, inf) != 0x14)
        return 5;
    if (table(nan, one) != 0x01 || table(one, nan) != 0x01 || table(nan, nan) != 0x01)
        return 6;
    if (table(nan, inf) != 0x01 || table(-nan, -inf) != 0x01)
        return 7;
    if (tablef(f_half, 0.25f) != 0x32 || tablef(f_nan, f_half) != 0x01)
        return 8;
    if (tablel(l_three, 3.0L) != 0x14 || tablel(l_nan, l_three) != 0x01)
        return 9;

    /* Mixed operand types use the usual arithmetic conversions. */
    if (!__builtin_isless(f_half, one) || !__builtin_isgreater(l_three, two))
        return 10;
    /* 0.1f converted to double is slightly greater than 0.1. */
    if (!__builtin_isgreater(0.1f, 0.1) || __builtin_isless(0.1f, 0.1))
        return 11;
    if (!__builtin_isgreaterequal(3, l_three) || __builtin_islessgreater(2, two))
        return 12;
    if (!__builtin_isunordered(f_nan, l_three) || __builtin_isunordered(1, two))
        return 13;
    if (__builtin_isgreater(nan, 1) || __builtin_islessequal(1u, f_nan))
        return 14;

    /* The result is an int with value 0 or 1. */
    r = __builtin_isgreater(two, one) + __builtin_isunordered(nan, nan);
    if (r != 2 || sizeof(__builtin_isless(one, two)) != sizeof(int))
        return 15;
    if (_Generic(__builtin_islessgreater(one, two), int: 0, default: 1))
        return 16;
    return 0;
}
