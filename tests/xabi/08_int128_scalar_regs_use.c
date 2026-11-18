int printf(const char *, ...);

typedef struct { __int128 q; } Q;
typedef struct { long a, b; } P;

void two(__int128 a, long b, long c, long d, Q q, long after);
void three(__int128 a, __int128 b, long c, P p, int z);
void late(int a, int b, int c, int d, int e, __int128 x, long after);

int main(void) {
    __int128 a = ((__int128)0xa1 << 64) | 0xa2;
    __int128 b = ((__int128)0xb1 << 64) | 0xb2;
    Q q = {((__int128)0x1234 << 64) | 0x5678};
    P p = {-5, 6};
    two(a, 1, 2, 3, q, 99);
    three(a, b, 7, p, 8);
    late(1, 2, 3, 4, 5, b, 42);
    return 0;
}
