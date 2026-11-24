/* GNU C: a struct or union cast to its own type is a no-op that yields its
   value, a copy (GCC and Clang accept it in gnu11; strict C11 6.5.4p2
   requires a scalar type). QuickJS writes `(JSValue)v` in <quickjs.h>. */
int printf(const char *, ...);
struct S { int a; double b; char c[3]; };
union U { int i; float f; };
typedef struct { long v; } V;
static struct S id(struct S s) { return (struct S)s; }
static V mk(long v) { V x = { v }; return (V)x; }
int main(void) {
    struct S s = { 1, 2.5, "ab" };
    const struct S cs = { 7, 8.5, "cd" };
    volatile struct S vs = { 9, 1.5, "ef" };
    struct S t = (struct S)cs;
    union U u = { 3 };
    union U w = (union U)u;
    t = (const struct S)s;
    struct S *p = &s;
    int a = ((struct S)s).a + ((struct S)*p).c[1] + ((struct S)vs).a;
    struct S r = id((struct S)cs);
    s.a = 100;
    struct S cp = (struct S)s;
    (struct S)s;
    int k = (1 ? (struct S)s : cs).a;
    V v = (V)mk(42);
    printf("%d %g %s %d %d %d %g %s %d %d %ld\n", t.a, t.b, t.c, w.i, a, r.a, r.b, cp.c, cp.a, k, v.v);
    return 0;
}
