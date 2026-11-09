int printf(const char *, ...);

int main(void) {
    const char *s = "hello";
    int sum = 0;
    int n = 0;
    while (*s != 0) {
        sum = sum + *s;
        s = s + 1;
        n = n + 1;
    }
    printf("%d %d %c\n", n, sum, "hello"[1]);
    return 0;
}
