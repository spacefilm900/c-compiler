/* Variadic 128-bit register accounting and va_arg alignment. A 128-bit
   argument that does not fit in the remaining GP registers goes on the
   stack, but a later argument can still use the leftover register.
   va_arg of a 128-bit value from the register save area reads an 8-byte
   aligned slot, and a 16-byte aligned record in the overflow area skips
   to the next 16-byte boundary. */

int printf(const char *, ...);

struct Mix {
    double d;
    long x;
};

struct Wide {
    __uint128_t v;
};

/* Two INTEGER eightbytes, so register-class, but 16-byte aligned. */
struct Al16 {
    long a __attribute__((aligned(16)));
    long b;
};

struct WideTag {
    __uint128_t v;
    long tag;
};

static unsigned long long lo(__uint128_t v)
{
    return (unsigned long long)v;
}

static unsigned long long hi(__uint128_t v)
{
    return (unsigned long long)(v >> 64);
}

/* skip, a, b, c, d use five GP registers. The __int128 needs two, so it
   is passed on the stack. struct Mix then has the sixth GP register and
   one XMM register (System V AMD64 3.2.3: only the spilled argument's own
   registers are given back). GCC passes m.x in %r9, but the LLVM 19
   backend puts it on the stack while Clang's va_arg reads %r9, so only
   the SSE half and the values before it are compared here. A caller that
   sent the whole struct to the stack would leave m.d unset in %xmm0.
   LLVM 15-17 backends split the __int128 between %r9 and the stack, so
   this test (and the compiler) needs LLVM 18 or newer. */
static void spill_then_mix(int skip, ...)
{
    __builtin_va_list ap;
    __builtin_va_start(ap, skip);
    long a = __builtin_va_arg(ap, long);
    long b = __builtin_va_arg(ap, long);
    long c = __builtin_va_arg(ap, long);
    long d = __builtin_va_arg(ap, long);
    __int128 w = __builtin_va_arg(ap, __int128);
    struct Mix m = __builtin_va_arg(ap, struct Mix);
    __builtin_va_end(ap);
    printf("spill %d %ld %ld %ld %ld %llu %llu %.1f\n", skip, a, b, c, d,
           hi((__uint128_t)w), lo((__uint128_t)w), m.d);
}

/* The first variadic __int128 starts at gp_offset 8, so its register
   save slot is 8 bytes past a 16-byte boundary. */
static void from_regs(int skip, ...)
{
    __builtin_va_list ap;
    __builtin_va_start(ap, skip);
    __int128 x = __builtin_va_arg(ap, __int128);
    __uint128_t y = __builtin_va_arg(ap, __uint128_t);
    int after = __builtin_va_arg(ap, int);
    __builtin_va_end(ap);
    printf("regs %d %llu %llu %llu %llu %d\n", skip, hi((__uint128_t)x),
           lo((__uint128_t)x), hi(y), lo(y), after);
}

/* All six GP registers are used and one long is in the overflow area,
   so each 16-byte aligned record starts 8 bytes past a 16-byte boundary
   and must skip to the next one. struct Al16 is register-class but has
   no registers left. */
static void from_stack(int skip, ...)
{
    __builtin_va_list ap;
    __builtin_va_start(ap, skip);
    long sum = 0;
    int i;
    for (i = 0; i < 6; i++) {
        sum += __builtin_va_arg(ap, long);
    }
    struct Wide w = __builtin_va_arg(ap, struct Wide);
    struct WideTag t = __builtin_va_arg(ap, struct WideTag);
    long odd = __builtin_va_arg(ap, long);
    struct Al16 al = __builtin_va_arg(ap, struct Al16);
    long last = __builtin_va_arg(ap, long);
    __builtin_va_end(ap);
    printf("stack %d %ld %llu %llu %llu %llu %ld %ld %ld %ld %ld\n", skip,
           sum, hi(w.v), lo(w.v), hi(t.v), lo(t.v), t.tag, odd, al.a, al.b,
           last);
}

int main(void)
{
    __int128 big = ((__int128)0x123456789abcdefLL << 64) | 0xfedcba987654321ULL;
    __uint128_t ubig = ((__uint128_t)0xdeadbeefULL << 64) | 0xcafef00dULL;
    struct Mix m = { 2.5, 77 };
    spill_then_mix(1, 10L, 20L, 30L, 40L, big, m);
    m.d = -0.5;
    spill_then_mix(2, 11L, 21L, 31L, 41L, -big, m);
    from_regs(3, big, ubig, 42);
    struct Wide w = { ubig };
    struct WideTag t = { ~ubig, -5 };
    struct Al16 al = { 31, 32 };
    from_stack(4, 1L, 2L, 3L, 4L, 5L, 6L, w, t, 7L, al, 123L);
    al.a = -31;
    from_stack(5, 1L, 2L, 3L, 4L, 5L, 6L, w, t, 8L, al, 456L);
    return 0;
}
