/* Unary ! in integer constant expressions (C11 6.5.3.3p5, 6.6, 6.7.10). */

#if !0
int pp_true = 1;
#else
int pp_true = 0;
#endif
#if !1
int pp_false = 1;
#else
int pp_false = 0;
#endif

int init_not0 = !0;
int init_not1 = !1;
int init_notnot0 = !!0;
int init_notnot1 = !!1;
int init_notnotnot0 = !!!0;
int init_not5 = !5;
int init_notnot5 = !!5;
int init_sc0 = !(0 && (1 / 0));
int init_sc1 = !(1 || (1 / 0));

enum {
    E0 = !0,
    E1 = !1,
    EN0 = !!0,
    EN1 = !!1,
    ENN0 = !!!0,
    E5 = !5,
    EN5 = !!5,
    ESC0 = !(0 && (1 / 0)),
    ESC1 = !(1 || (1 / 0))
};

int a[!0 + 1];
int b[!!1 + 1];
int c[!!!0 + 1];
int d[!5 + 1];
int e[!!5 + 1];
int f[!(0 && (1 / 0)) + 1];
int g[!(1 || (1 / 0)) + 1];

struct Bits {
    unsigned u : !0 + 1;
    unsigned v : !!1 + 1;
};

_Static_assert(!0, "ok");
_Static_assert(!!1, "ok2");
_Static_assert(!!!0, "ok3");
_Static_assert(!(0 && (1 / 0)), "sc");
_Static_assert((!5) == 0, "not5");
_Static_assert(!!5, "notnot5");
_Static_assert(E0 == 1, "E0");
_Static_assert(E1 == 0, "E1");
_Static_assert(sizeof a == 2 * sizeof(int), "a");
_Static_assert(sizeof b == 2 * sizeof(int), "b");
_Static_assert(sizeof c == 2 * sizeof(int), "c");
_Static_assert(sizeof d == sizeof(int), "d");
_Static_assert(sizeof e == 2 * sizeof(int), "e");
_Static_assert(sizeof f == 2 * sizeof(int), "f");
_Static_assert(sizeof g == sizeof(int), "g");

int main(void) {
    _Static_assert(!0, "block");
    _Static_assert(!!1, "block2");
    _Static_assert(!!!0, "block3");
    struct S {
        int x;
        _Static_assert(!0, "member");
    } s;
    enum { L = !0, M = !!0, N = !!!0, SC = !(0 && (1 / 0)) };
    int loc[!0 + 1];
    int sc[!(0 && (1 / 0)) + 1];
    struct Bits bits;

    if (pp_true != 1 || pp_false != 0)
        return 1;
    if (init_not0 != 1 || init_not1 != 0 || init_notnot0 != 0 || init_notnot1 != 1
        || init_notnotnot0 != 1 || init_not5 != 0 || init_notnot5 != 1
        || init_sc0 != 1 || init_sc1 != 0)
        return 2;
    if (E0 != 1 || E1 != 0 || EN0 != 0 || EN1 != 1 || ENN0 != 1 || E5 != 0
        || EN5 != 1 || ESC0 != 1 || ESC1 != 0)
        return 3;
    if (sizeof a != 2 * sizeof(int) || sizeof b != 2 * sizeof(int)
        || sizeof c != 2 * sizeof(int) || sizeof d != sizeof(int)
        || sizeof e != 2 * sizeof(int) || sizeof f != 2 * sizeof(int)
        || sizeof g != sizeof(int))
        return 4;
    if (L != 1 || M != 0 || N != 1 || SC != 1 || sizeof loc != 2 * sizeof(int)
        || sizeof sc != 2 * sizeof(int))
        return 5;
    /* A 2-bit field holds 3; a wrongly computed 1-bit width would truncate. */
    bits.u = 3;
    bits.v = 3;
    if (bits.u != 3 || bits.v != 3)
        return 6;
    switch (1) {
    case !0:
        break;
    default:
        return 7;
    }
    switch (0) {
    case !1:
        break;
    default:
        return 8;
    }
    switch (1) {
    case !!1:
        break;
    default:
        return 9;
    }
    s.x = 0;
    return s.x;
}
