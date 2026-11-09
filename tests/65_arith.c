int printf(const char *, ...);

int main(void) {
    unsigned u = 0;
    unsigned char c = 255;
    int sum = 0;
    int i = 1;
    u = u - 1;
    while (i <= 10) {
        sum = sum + i;
        i = i + 1;
    }
    printf("%u\n", u);
    printf("%d\n", (int)(c + 1));
    printf("%d\n", sum);
    printf("%d\n", (int)sizeof(int));
    printf("%d\n", -1 < 1u);
    printf("%ld\n", -1L < 1u);
    return 0;
}
