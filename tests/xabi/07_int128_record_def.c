/* A record holding an __int128 is two INTEGER eightbytes on x86-64 (System
   V AMD64 3.2.3: __int128 is a pair of eightbytes), so it goes in two
   general registers, or wholly on the stack when only one is left; Clang
   and GCC agree. This compiler passed it in memory. Found by the Intel
   macOS signature check against Clang (harness/run_arm64_ir.py). */
int printf(const char *, ...);

typedef struct { __int128 q; } Q;
typedef union { __int128 q; long l; } U;
typedef struct { long a; __int128 q; } Wide;

static unsigned long hi(__int128 v) { return (unsigned long)((unsigned __int128)v >> 64); }
static unsigned long lo(__int128 v) { return (unsigned long)v; }

void show(int a, Q q, U u, Wide w, int z) {
    printf("show %d %lx %lx %lx %lx %ld %lx %d\n", a, hi(q.q), lo(q.q), hi(u.q), lo(u.q),
           w.a, hi(w.q), z);
}

/* Five ints take five of the six registers: q goes on the stack. */
void late(int a, int b, int c, int d, int e, Q q, long after) {
    printf("late %d %d %d %d %d %lx %lx %ld\n", a, b, c, d, e, hi(q.q), lo(q.q), after);
}

void vshow(int n, ...) {
    __builtin_va_list ap;
    __builtin_va_start(ap, n);
    Q q = __builtin_va_arg(ap, Q);
    U u = __builtin_va_arg(ap, U);
    long l = __builtin_va_arg(ap, long);
    Q r = __builtin_va_arg(ap, Q);
    printf("vshow %d %lx %lx %lx %lx %ld %lx %lx\n", n, hi(q.q), lo(q.q), hi(u.q), lo(u.q), l,
           hi(r.q), lo(r.q));
    __builtin_va_end(ap);
}

Q make_q(long k) {
    Q q = {((__int128)k << 64) | (unsigned long)(k * 3)};
    return q;
}

U make_u(long k) {
    U u;
    u.q = ((__int128)(k + 1) << 64) | (unsigned long)k;
    return u;
}
