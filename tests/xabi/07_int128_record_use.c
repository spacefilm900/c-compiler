int printf(const char *, ...);

typedef struct { __int128 q; } Q;
typedef union { __int128 q; long l; } U;
typedef struct { long a; __int128 q; } Wide;

void show(int a, Q q, U u, Wide w, int z);
void late(int a, int b, int c, int d, int e, Q q, long after);
void vshow(int n, ...);
Q make_q(long k);
U make_u(long k);

static unsigned long hi(__int128 v) { return (unsigned long)((unsigned __int128)v >> 64); }
static unsigned long lo(__int128 v) { return (unsigned long)v; }

int main(void) {
    Q q = {((__int128)0x1234 << 64) | 0x5678};
    U u;
    u.q = ((__int128)0x9abc << 64) | 0xdef0;
    Wide w = {-7, (__int128)0x42 << 64};
    show(1, q, u, w, 2);
    late(1, 2, 3, 4, 5, q, 99);
    vshow(3, q, u, 77L, q);
    Q r = make_q(0x11);
    printf("make_q %lx %lx\n", hi(r.q), lo(r.q));
    U s = make_u(0x21);
    printf("make_u %lx %lx\n", hi(s.q), lo(s.q));
    return 0;
}
