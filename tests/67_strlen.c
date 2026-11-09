int printf(const char *, ...);
unsigned long strlen(const char *);

unsigned long my_strlen(const char *s) {
    unsigned long n = 0;
    while (s[n] != 0) {
        n = n + 1;
    }
    return n;
}

int main(void) {
    const char *s = "hel" "lo";
    char empty[] = "";
    printf("%lu %lu\n", my_strlen(s), strlen(s));
    printf("%lu %lu\n", my_strlen(empty), strlen(empty));
    printf("%lu\n", my_strlen("a\0b"));
    return 0;
}
