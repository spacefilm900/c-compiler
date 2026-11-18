/* A scalar __int128 parameter takes two general registers (System V AMD64
   3.2.3), so later arguments see one register fewer than for a long. This
   compiler counted one, and a two-eightbyte record that no longer fitted
   was split between the last register and the stack. Found by
   tools/abi_fuzz.py --int128 (seed 5017). */
int printf(const char *, ...);

typedef struct { __int128 q; } Q;
typedef struct { long a, b; } P;

static unsigned long hi(__int128 v) { return (unsigned long)((unsigned __int128)v >> 64); }
static unsigned long lo(__int128 v) { return (unsigned long)v; }

/* a: 2 registers, b c d: 3, so q goes on the stack and after takes %r9. */
void two(__int128 a, long b, long c, long d, Q q, long after) {
    printf("two %lx %lx %ld %ld %ld %lx %lx %ld\n", hi(a), lo(a), b, c, d, hi(q.q), lo(q.q),
           after);
}

/* a and b: 4 registers, c: 1, so p (two INTEGER eightbytes) goes on the
   stack and z takes the last register. */
void three(__int128 a, __int128 b, long c, P p, int z) {
    printf("three %lx %lx %lx %ld %ld %ld %d\n", lo(a), hi(b), lo(b), c, p.a, p.b, z);
}

/* Five ints leave one register: x goes on the stack whole and after
   still gets %r9. */
void late(int a, int b, int c, int d, int e, __int128 x, long after) {
    printf("late %d %d %d %d %d %lx %lx %ld\n", a, b, c, d, e, hi(x), lo(x), after);
}
