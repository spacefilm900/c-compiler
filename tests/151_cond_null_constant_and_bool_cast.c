/* C11 6.5.15p6: a null pointer constant takes the type of the other
   operand before `void *` merging, in sema and in the parser's typing of
   `_Static_assert` and `_Generic`. An explicit `(_Bool)` cast of an address
   is its truth value (6.3.1.2), as the implicit conversion is. */
int printf(const char *, ...);
int g;
_Bool b1 = (_Bool)&g;
int i1 = (_Bool)&g;
struct A { int x; };
int main(void) {
    int *p = &g;
    void *v = p;
    const int *cp = p;
    _Static_assert(_Generic(1 ? (int *)1 : (void *)0, int *: 1, default: 0), "n1");
    _Static_assert(_Generic(1 ? (void *)0 : cp, const int *: 1, default: 0), "n2");
    _Static_assert(_Generic(1 ? v : cp, const void *: 1, default: 0), "n3");
    _Static_assert(sizeof(1 ? (struct A){0} : (struct A){1}) == sizeof(int), "rec");
    int s = _Generic(g ? p : (void *)0, int *: 1, default: 0)
          + _Generic(g ? (void *)0 : cp, const int *: 2, default: 0)
          + _Generic(g ? v : p, void *: 4, default: 0);
    printf("%d %d %d\n", b1, i1, s);
    return 0;
}
