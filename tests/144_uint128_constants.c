/* 128-bit integer constants: conversions, shifts, union initializers,
   and folding in a floating-constant context. */

int printf(const char *, ...);

static __uint128_t from_neg = -1;
static __uint128_t from_ull = 0xffffffffffffffffULL;
static union U {
    __uint128_t x;
    unsigned long long parts[2];
} u = {1};
static int mixed = (((__uint128_t)18446744073709551615ULL + (__uint128_t)1.0)
                    / 9223372036854775808ULL);
static __uint128_t shr63 = (__uint128_t)-1 >> 63;
static __uint128_t one_high = (__uint128_t)1 << 64;
static signed __int128 neg = -1;
static __uint128_t all = ~(__uint128_t)0;

_Static_assert(((__uint128_t)-1 >> 63) != 1, "not one");
_Static_assert(((__uint128_t)1 << 64) >> 64 == 1, "high bit");
_Static_assert(((__uint128_t)-1 >> 64) != 0, "upper ones");

static unsigned long long lo(__uint128_t v) {
    return (unsigned long long)v;
}

static unsigned long long hi(__uint128_t v) {
    return (unsigned long long)(v >> 64);
}

int main(void) {
    printf("%llu %llu\n", hi(from_neg), lo(from_neg));
    printf("%llu %llu\n", hi(from_ull), lo(from_ull));
    printf("%llu %llu\n", u.parts[0], u.parts[1]);
    printf("%d\n", mixed);
    printf("%llu %llu\n", hi(shr63), lo(shr63));
    printf("%llu %llu\n", hi(one_high), lo(one_high));
    printf("%d %d\n", (int)neg, (int)(neg == (signed __int128)-1));
    printf("%llu %llu\n", hi(all), lo(all));
    printf("%d\n", (__uint128_t)-1 == ~(__uint128_t)0);
    printf("%llu\n", lo(((__uint128_t)2 << 63) / 2));
    return 0;
}
