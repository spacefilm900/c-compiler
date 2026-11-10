int printf(const char *, ...);

long double add(long double a, long double b) { return a + b; }

long double mix(int n, long double x, int m) { return x + (long double)n + (long double)m; }

long double pick(int count, ...) {
    __builtin_va_list ap;
    __builtin_va_start(ap, count);
    long double value = __builtin_va_arg(ap, long double);
    __builtin_va_end(ap);
    return value;
}

static void dump(long double value) {
    unsigned char bytes[16];
    unsigned char *src = (unsigned char *)&value;
    int i;
    for (i = 0; i < 10; i++) {
        bytes[i] = src[i];
    }
    for (i = 0; i < 10; i++) {
        printf("%02x", bytes[i]);
    }
    printf("\n");
}

int main(void) {
    long double a = 0.1L;
    long double b = 0.2L;
    long double sum = a + b;
    long double quot = 1.0L / 3.0L;
    long double scaled = 2.5L * 4.0L - 0.5L;
    long double huge = 1.18973149535723176502e+4932L;
    dump(1.0L);
    dump(0.1L);
    dump(sum);
    dump(quot);
    dump(add(1.5L, 2.5L));
    dump(mix(2, 3.5L, 4));
    dump(scaled);
    dump(huge);
    dump(-0.1L);
    a += 2.0L;
    dump(a);
    a++;
    dump(a);
    printf("%d %d %d\n", sum > 0.3L, quot * 3.0L == 1.0L, scaled == 9.5L);
    printf("%d\n", huge > 1.0L && (int)1.9L == 1 && (int)(-1.9L) == -1);
    dump(pick(1, 4.5L));
    return 0;
}
