/* Bit-fields around an over-aligned member (C11 6.7.2.1, 6.7.5).
   A later bit-field may follow _Alignas padding. That is valid C11:
   the leading storage unit stays at its natural offset, and a new
   unit starts after the aligned member. Variant 2 also uses a
   zero-width : 0 to end the first unit. */

int printf(const char *, ...);

struct V1 {
    unsigned int a : 3;
    _Alignas(16) int x;
    unsigned int b : 5;
};

struct W1 {
    char lead;
    struct V1 s;
    char tail;
};

struct V2 {
    unsigned int a : 3;
    unsigned int : 0;
    _Alignas(16) int x;
    unsigned int b : 5;
};

struct W2 {
    char lead;
    struct V2 s;
    char tail;
};

_Static_assert(_Alignof(struct V1) == 16, "v1 align");
_Static_assert(sizeof(struct V1) == 32, "v1 size");
_Static_assert(_Alignof(struct W1) == 16, "w1 align");
_Static_assert(sizeof(struct W1) == 64, "w1 size");
_Static_assert(_Alignof(struct V2) == 16, "v2 align");
_Static_assert(sizeof(struct V2) == 32, "v2 size");
_Static_assert(_Alignof(struct W2) == 16, "w2 align");
_Static_assert(sizeof(struct W2) == 64, "w2 size");

int main(void) {
    struct W1 a[3];
    struct W1 y;
    struct W2 b[3];
    struct W2 z;
    int ok = 1;
    int i;
    for (i = 0; i < 3; ++i) {
        a[i].lead = 0;
        a[i].s.a = 0;
        a[i].s.x = 0;
        a[i].s.b = 0;
        a[i].tail = 0;
        b[i].lead = 0;
        b[i].s.a = 0;
        b[i].s.x = 0;
        b[i].s.b = 0;
        b[i].tail = 0;
    }
    a[1].s.a = 5;
    a[1].s.x = 71;
    a[1].s.b = 23;
    a[1].tail = 9;
    y = a[1];
    if ((unsigned long)&a[1].s.x % 16 != 0)
        ok = 0;
    if ((unsigned long)&a[1].s.x - (unsigned long)&a[1] != 32)
        ok = 0;
    if ((unsigned long)&a[2] - (unsigned long)&a[1] != 64)
        ok = 0;
    if (y.s.a != 5 || y.s.b != 23 || y.s.x != 71 || y.tail != 9)
        ok = 0;
    if (a[0].s.x != 0)
        ok = 0;
    b[1].s.a = 5;
    b[1].s.x = 71;
    b[1].s.b = 23;
    b[1].tail = 9;
    z = b[1];
    if ((unsigned long)&b[1].s.x % 16 != 0)
        ok = 0;
    if ((unsigned long)&b[1].s.x - (unsigned long)&b[1] != 32)
        ok = 0;
    if ((unsigned long)&b[2] - (unsigned long)&b[1] != 64)
        ok = 0;
    if (z.s.a != 5 || z.s.b != 23 || z.s.x != 71 || z.tail != 9)
        ok = 0;
    printf("%d %d %d %d\n", (int)sizeof(struct W1), (int)_Alignof(struct W1),
           (int)sizeof(struct V1), ok);
    return ok ? 0 : 1;
}
