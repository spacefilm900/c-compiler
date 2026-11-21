/* Nested over-aligned records (C11 6.2.8, 6.7.5, 6.7.2.1).
   A member with _Alignas(32) gives the inner type alignment 32.
   Enclosing structs and arrays must keep that alignment in their
   layout, sizeof, and element stride. */

int printf(const char *, ...);

struct Inner {
    char tag;
    _Alignas(32) int value;
};

struct Outer {
    char tag;
    struct Inner inner;
    char end;
};

struct Row {
    char tag;
    struct Inner cells[2];
    char end;
};

_Static_assert(_Alignof(struct Inner) == 32, "inner align");
_Static_assert(sizeof(struct Inner) == 64, "inner size");
_Static_assert(_Alignof(struct Outer) == 32, "outer align");
_Static_assert(sizeof(struct Outer) == 128, "outer size");
_Static_assert(_Alignof(struct Row) == 32, "row align");
_Static_assert(sizeof(struct Row) == 192, "row size");

struct Outer g[3];

int main(void) {
    struct Outer a[3];
    struct Row r;
    int ok = 1;
    for (int i = 0; i < 3; ++i) {
        a[i].inner.value = i + 7;
        g[i].inner.value = i + 11;
        if ((unsigned long)&a[i].inner.value % 32 != 0)
            ok = 0;
        if ((unsigned long)&g[i].inner.value % 32 != 0)
            ok = 0;
    }
    r.cells[0].value = 21;
    r.cells[1].value = 22;
    if ((unsigned long)&r.cells[0].value % 32 != 0)
        ok = 0;
    if ((unsigned long)&r.cells[1].value % 32 != 0)
        ok = 0;
    if (a[2].inner.value != 9)
        ok = 0;
    if (g[2].inner.value != 13)
        ok = 0;
    if (r.cells[1].value != 22)
        ok = 0;
    if ((unsigned long)&a[0] % 32 != 0)
        ok = 0;
    printf("%d %d %d %d\n", (int)sizeof(struct Outer), (int)_Alignof(struct Outer),
           (int)sizeof(struct Row), ok);
    return ok ? 0 : 1;
}
