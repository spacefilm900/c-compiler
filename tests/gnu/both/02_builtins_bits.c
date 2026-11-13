/* Bit-manipulation builtins (GCC manual, "Other Built-in Functions"). */

int printf(const char *, ...);

int main(void)
{
    unsigned u[] = {1u, 2u, 0x80000000u, 0xf0u, 0x12345678u, 0xffffffffu};
    for (unsigned i = 0; i < 6; i++)
        printf("%d %d %d %d %d %d\n", __builtin_clz(u[i]), __builtin_ctz(u[i]),
               __builtin_popcount(u[i]), __builtin_parity(u[i]), __builtin_ffs((int)u[i]),
               __builtin_clrsb((int)u[i]));
    unsigned long l = 0x0000100000000000ul;
    unsigned long long ll = 0x8000000000000001ull;
    printf("%d %d %d %d %d %d\n", __builtin_clzl(l), __builtin_ctzl(l), __builtin_popcountl(l),
           __builtin_parityl(l), __builtin_ffsl((long)l), __builtin_clrsbl(-1L));
    printf("%d %d %d %d %d %d\n", __builtin_clzll(ll), __builtin_ctzll(ll),
           __builtin_popcountll(ll), __builtin_parityll(ll), __builtin_ffsll((long long)ll),
           __builtin_clrsbll(1LL));
    printf("%d %d %d\n", __builtin_ffs(0), __builtin_ffsl(0), __builtin_ffsll(0));
    printf("%x %x %llx\n", __builtin_bswap16(0x1234), __builtin_bswap32(0x12345678u),
           (unsigned long long)__builtin_bswap64(0x0102030405060708ull));
    printf("%zu %zu %zu\n", sizeof(__builtin_bswap16(1)), sizeof(__builtin_bswap32(1)),
           sizeof(__builtin_bswap64(1)));
    printf("%d %d\n", _Generic(__builtin_clz(1u), int: 1, default: 0),
           _Generic(__builtin_bswap64(1), unsigned long: 1, unsigned long long: 2, default: 0));
    enum { C = __builtin_popcount(0xffu), D = __builtin_ctz(8), E = __builtin_bswap16(0x0100) };
    printf("%d %d %d\n", C, D, E);
    return 0;
}
