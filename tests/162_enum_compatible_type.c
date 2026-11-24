/* The type compatible with an enumerated type is implementation-defined
   (C11 6.7.2.2p4). GCC documents `unsigned int` when no value is negative,
   else `int`; Clang agrees except on MSVC Windows (always `int`). It shows
   in enum bit-fields and in arithmetic on enum objects. The constants
   are `int` (C11 6.4.4.3p2). QuickJS's `JSClosureTypeEnum closure_type : 3`
   holds 0..7. */
int printf(const char *, ...);

enum Kind { K0, K1, K2, K3, K4, K5, K6, K7 };
enum Signed { SNEG = -1, S0, S1, S2 };
struct Packed {
    enum Kind kind : 3;
    enum Signed sign : 3;
    unsigned rest : 26;
};

static int classify(enum Kind k) {
    switch (k) {
    case K4: return 40;
    case K7: return 70;
    default: return k;
    }
}

int main(void) {
    struct Packed p = { K7, S2, 5 };
    struct Packed q = { K4, SNEG, 6 };
    enum Kind zero = K0;
    enum Signed neg = SNEG;
    printf("%d %d %d %d\n", p.kind, p.sign, q.kind, q.sign);
    printf("%d %d\n", classify(p.kind), classify(q.kind));
    printf("%d %d %d\n", zero - 1 > 0, neg < 0, K0 - 1 < 0);
    printf("%d %d\n", (long long)zero - 1 < 0, (int)(zero + 0u - 1 > 0));
    printf("%zu %zu %d\n", sizeof(enum Kind), sizeof(p), _Generic(K1, int: 1, default: 0));
    return 0;
}
