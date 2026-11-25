/* GNU labels as values with extended identifiers: the IR block names of
   `l\u00e0` and `fin\u00e9` must be quoted (LLVM LangRef identifiers). */
int printf(const char *, ...);
int main(void) {
    static void *cibles[] = { &&l\u00e0, &&fin\u00e9 };
    int compte = 0;
    goto *cibles[0];
là:
    compte += 10;
    goto *cibles[1];
finé:
    printf("%d\n", compte);
    return 0;
}
