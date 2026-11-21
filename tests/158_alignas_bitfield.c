/* Bit-field next to an over-aligned member (C11 6.7.2.1, 6.7.5).
   _Alignas on a later member inserts padding after the bit-field's
   storage unit. That is valid C11 and must be accepted; the bit-field
   itself stays at its natural offset. An enclosing struct and array
   must keep that alignment in sizeof, member offsets, and stride. */

int printf(const char *, ...);

struct Inner {
    unsigned int b : 3;
    _Alignas(16) int value;
};

struct Outer {
    char lead;
    struct Inner inner;
    char tail;
};

_Static_assert(_Alignof(struct Inner) == 16, "inner align");
_Static_assert(sizeof(struct Inner) == 32, "inner size");
_Static_assert(_Alignof(struct Outer) == 16, "outer align");
_Static_assert(sizeof(struct Outer) == 64, "outer size");

int main(void) {
    struct Outer a[3];
    struct Outer y;
    int ok = 1;
    int i;
    for (i = 0; i < 3; ++i) {
        a[i].lead = 0;
        a[i].inner.b = 0;
        a[i].inner.value = 0;
        a[i].tail = 0;
    }
    a[1].inner.b = 5;
    a[1].inner.value = 71;
    a[1].tail = 9;
    y = a[1];
    if ((unsigned long)&a[1].inner.value % 16 != 0)
        ok = 0;
    if ((unsigned long)&a[1].inner.value - (unsigned long)&a[1] != 32)
        ok = 0;
    if ((unsigned long)&a[2] - (unsigned long)&a[1] != 64)
        ok = 0;
    if (y.inner.value != 71)
        ok = 0;
    if (y.tail != 9)
        ok = 0;
    if (a[1].inner.b != 5)
        ok = 0;
    printf("%d %d %d %d\n", (int)sizeof(struct Outer), (int)_Alignof(struct Outer),
           (int)sizeof(struct Inner), ok);
    return ok ? 0 : 1;
}
