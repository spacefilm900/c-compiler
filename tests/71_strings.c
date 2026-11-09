int printf(const char *, ...);

int main(void) {
    char a[] = "hi";
    char b[10] = "hi";
    char c[2] = "hi";
    char d[] = "hel" "lo";
    signed char e[] = "A\xff";
    int i;
    int sum = 0;
    printf("%d %d %d\n", (int)sizeof "abc", (int)sizeof "", (int)sizeof a);
    printf("%s %s\n", a, d);
    printf("%d %d\n", (int)c[0], (int)c[1]);
    for (i = 0; i < 10; i = i + 1) {
        sum = sum + b[i];
    }
    printf("%d\n", sum);
    printf("%d %d\n", (int)e[0], (int)e[1]);
    printf("%d\n", (int)sizeof "a\0b");
    return 0;
}
