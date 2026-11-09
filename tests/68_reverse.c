int printf(const char *, ...);

unsigned long len(const char *s) {
    unsigned long n = 0;
    while (s[n] != 0) {
        n = n + 1;
    }
    return n;
}

void reverse(const char *in, char *out) {
    unsigned long n = len(in);
    unsigned long i = 0;
    while (i < n) {
        out[i] = in[n - 1 - i];
        i = i + 1;
    }
    out[n] = 0;
}

int main(void) {
    char buf[16];
    reverse("stressed", buf);
    printf("%s\n", buf);
    reverse("", buf);
    printf("%d\n", (int)buf[0]);
    return 0;
}
