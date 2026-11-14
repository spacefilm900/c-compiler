/* GNU 128-bit integers (__int128, unsigned __int128, __int128_t,
   __uint128_t). SQLite uses __uint128_t for 64x64->128 multiplication
   when the compiler defines __GNUC__ on a 64-bit target. */

int printf(const char *, ...);

unsigned long long mul_lo(unsigned long long a, unsigned long long b) {
    __uint128_t r = (__uint128_t)a * b;
    return (unsigned long long)r;
}

unsigned long long mul_hi(unsigned long long a, unsigned long long b) {
    __uint128_t r = (__uint128_t)a * b;
    return (unsigned long long)(r >> 64);
}

/* The shape of sqlite3Multiply160: a 96-bit value times a 64-bit value. */
unsigned long long mul96_lo(unsigned long long a, unsigned a_lo, unsigned long long b) {
    __uint128_t r = (__uint128_t)a * b;
    r += ((__uint128_t)a_lo * b) >> 32;
    return (unsigned long long)r;
}

unsigned long long mul96_hi(unsigned long long a, unsigned a_lo, unsigned long long b) {
    __uint128_t r = (__uint128_t)a * b;
    r += ((__uint128_t)a_lo * b) >> 32;
    return (unsigned long long)(r >> 64);
}

__uint128_t add128(__uint128_t x, __uint128_t y) {
    return x + y;
}

int main(void) {
    unsigned long long lo = mul_lo(0xffffffffffffffffULL, 2);
    unsigned long long hi = mul_hi(0xffffffffffffffffULL, 2);
    unsigned long long a = mul96_lo(1, 0x80000000U, 0xffffffffffffffffULL);
    unsigned long long b = mul96_hi(1, 0x80000000U, 0xffffffffffffffffULL);
    __uint128_t s = add128(((__uint128_t)1) << 64, 3);
    unsigned long long s_lo = (unsigned long long)s;
    unsigned long long s_hi = (unsigned long long)(s >> 64);
    signed __int128 n = (signed __int128)-1;
    __int128_t m = (__int128_t)2 * (__int128_t)3;
    printf("%llu %llu %llu %llu %llu %llu %d %d\n",
           lo, hi, a, b, s_lo, s_hi, (int)n, (int)m);
    return 0;
}
