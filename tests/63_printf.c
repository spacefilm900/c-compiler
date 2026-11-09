int printf(const char *, ...);

int main(void) {
    int d = -42;
    unsigned u = 42;
    long ld = -10000000000L;
    unsigned long lu = 10000000000UL;
    long long lld = -10000000000LL;
    unsigned x = 0x2a;
    printf("%d %i %u\n", d, d, u);
    printf("%ld %lu %lld\n", ld, lu, lld);
    printf("%x %X %o\n", x, x, 64u);
    printf("%c %s %%\n", 65, "ok");
    return 0;
}
