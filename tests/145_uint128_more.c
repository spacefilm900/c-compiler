/* Further 128-bit coverage: overflow builtins, va_arg, struct passing
   and return, alignment, and conversions to and from floating types. */

int printf(const char *, ...);

static unsigned long long lo(__uint128_t v)
{
    return (unsigned long long)v;
}

static unsigned long long hi(__uint128_t v)
{
    return (unsigned long long)(v >> 64);
}

static void p128(const char *s, __uint128_t v, int o)
{
    printf("%s %d %llu %llu\n", s, o, hi(v), lo(v));
}

_Static_assert(_Alignof(__int128) == 16, "signed align");
_Static_assert(_Alignof(__uint128_t) == 16, "unsigned align");
_Static_assert(sizeof(__int128) == 16, "signed size");

struct A {
    char c;
    __int128 x;
};

_Static_assert(_Alignof(struct A) == 16, "struct align");
_Static_assert(sizeof(struct A) == 32, "struct size");

struct Wide {
    __uint128_t x;
};

struct Wide add_wide(struct Wide a, struct Wide b)
{
    a.x += b.x;
    return a;
}

struct Big {
    __uint128_t x;
    long y;
};

struct Big add_big(struct Big a, struct Big b)
{
    a.x += b.x;
    a.y += b.y;
    return a;
}

static __uint128_t take_after(int skip, ...)
{
    __builtin_va_list ap;
    __builtin_va_start(ap, skip);
    int i;
    for (i = 0; i < skip; i++) {
        (void)__builtin_va_arg(ap, int);
    }
    __uint128_t v = __builtin_va_arg(ap, __uint128_t);
    int after = __builtin_va_arg(ap, int);
    __builtin_va_end(ap);
    return v + (__uint128_t)(unsigned)after;
}

int main(void)
{
    { __uint128_t r; int o = __builtin_add_overflow((__uint128_t)-1, (__uint128_t)1, &r); p128("uadd max+1", r, o); }
    { __uint128_t r; int o = __builtin_add_overflow((__uint128_t)1, (__uint128_t)2, &r); p128("uadd 1+2", r, o); }
    { __int128 r; int o = __builtin_add_overflow((__int128)(((__uint128_t)1 << 127) - 1), (__int128)1, &r); p128("sadd max+1", (__uint128_t)r, o); }
    { __int128 r; int o = __builtin_add_overflow((__int128)0, (__int128)-1, &r); p128("sadd 0-1", (__uint128_t)r, o); }
    { __int128 r; int o = __builtin_add_overflow((__int128)((__uint128_t)1 << 127), (__int128)-1, &r); p128("sadd min-1", (__uint128_t)r, o); }
    { __uint128_t r; int o = __builtin_sub_overflow((__uint128_t)0, (__uint128_t)1, &r); p128("usub 0-1", r, o); }
    { __int128 r; int o = __builtin_sub_overflow((__int128)((__uint128_t)1 << 127), (__int128)1, &r); p128("ssub min-1", (__uint128_t)r, o); }
    { __uint128_t r; int o = __builtin_mul_overflow((__uint128_t)1 << 64, (__uint128_t)1 << 64, &r); p128("umul 2^64*2^64", r, o); }
    { __uint128_t r; int o = __builtin_mul_overflow((__uint128_t)1 << 63, (__uint128_t)2, &r); p128("umul 2^63*2", r, o); }
    { __int128 r; int o = __builtin_mul_overflow((__int128)1 << 62, (__int128)4, &r); p128("smul 2^62*4", (__uint128_t)r, o); }
    { __int128 r; int o = __builtin_mul_overflow((__int128)1 << 63, (__int128)2, &r); p128("smul 2^63*2", (__uint128_t)r, o); }
    { __int128 r; int o = __builtin_mul_overflow((__int128)-1, (__int128)-1, &r); p128("smul -1*-1", (__uint128_t)r, o); }
    { int r; int o = __builtin_add_overflow((__int128)1 << 40, (__int128)1, &r); printf("add into int %d %d\n", r, o); }
    { __int128 r; int o = __builtin_add_overflow(1, 2, &r); p128("add small into i128", (__uint128_t)r, o); }
    { __int128 r; int o = __builtin_add_overflow((__uint128_t)-1, (__uint128_t)0, &r); p128("u128max into s128", (__uint128_t)r, o); }
    { __uint128_t r; int o = __builtin_add_overflow((__int128)-1, (__int128)0, &r); p128("s-1 into u128", r, o); }
    { __uint128_t r; int o = __builtin_add_overflow((__int128)-1, (__int128)1, &r); p128("s-1+1 into u128", r, o); }

    struct Wide wa = { (__uint128_t)1 << 64 };
    struct Wide wb = { 3 };
    struct Wide ws = add_wide(wa, wb);
    printf("wide %llu %llu\n", hi(ws.x), lo(ws.x));

    struct Big ba = { (__uint128_t)-1, 4 };
    struct Big bb = { 2, 5 };
    struct Big bs = add_big(ba, bb);
    printf("big %llu %llu %ld\n", hi(bs.x), lo(bs.x), bs.y);

    struct A a;
    a.c = 7;
    a.x = -1;
    printf("field %d %d\n", (int)a.c, (int)a.x);

    __uint128_t v = ((__uint128_t)9 << 64) | 8;
    /* skip=4 would leave one GP register free. The ABI then passes the
       128-bit argument (and the trailing int) on the stack; Clang 19
       disagrees with itself at -O0 and -O2 on the trailing int, so that
       placement is not compared here. */
    __uint128_t r0 = take_after(0, v, 1);
    __uint128_t r3 = take_after(3, 10, 11, 12, v, 2);
    __uint128_t r5 = take_after(5, 10, 11, 12, 13, 14, v, 4);
    printf("va %llu %llu\n", hi(r0), lo(r0));
    printf("va %llu %llu\n", hi(r3), lo(r3));
    printf("va %llu %llu\n", hi(r5), lo(r5));

    double d = (double)((__uint128_t)1 << 80);
    __int128 back = (__int128)d;
    printf("flt %d %llu %llu\n", d == 0x1p80, hi((__uint128_t)back), lo((__uint128_t)back));
    long double ld = (long double)((__int128)-123);
    printf("ld %d\n", (int)(__int128)ld);
    printf("cast %d\n", (int)(__uint128_t)(float)3.0);
    return 0;
}
