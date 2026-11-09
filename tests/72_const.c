int printf(const char *, ...);

const int g = 41;

int main(void) {
    const int x = 1;
    const char *p = "ok";
    volatile int v = 2;
    v = v + x;
    printf("%d %s %d\n", g + x, p, v);
    return 0;
}
