/* Operands that are not evaluated: a _Generic controlling expression, an
   unselected _Generic association, and a sizeof operand. Each holds a
   statement expression with a block-scope static whose initializer is a
   label address, a string literal, and a compound literal. None of that
   code is emitted, so none of it may leave a global behind that names a
   missing label. The selected association is evaluated as usual. Found
   by review of the Lua fix (LUA-1). GNU C only. */

int printf(const char *, ...);
int control(void) {
    return _Generic(({ static void *q = &&l; l:; q; }), void *: 7, default: 0);
}
int unselected(void) {
    return _Generic(1.0, int: ({ static void *q = &&m; m:; (int)(long)q; }),
                    double: 8, default: ({ (int)"dropped"[0]; }));
}
long unevaluated_size(void) {
    return (long)sizeof(({ static void *q = &&n; n:; (int)(long)q + (int)"x"[0]; }));
}
int selected(void) {
    return _Generic(0, int: ({ static void *q = &&s; static int calls; s:; ++calls + (q != 0); }),
                    default: 0);
}
long literals(void) {
    return _Generic(({ "abc"; }), char *: (long)sizeof((int[]){1, 2, 3}), default: 0);
}
int main(void) {
    int first = selected();
    int second = selected();
    printf("%d %d %ld %d %d %ld\n", control(), unselected(), unevaluated_size(),
           first, second, literals());
    return 0;
}
