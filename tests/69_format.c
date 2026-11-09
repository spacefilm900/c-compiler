int printf(const char *, ...);

void utoa(unsigned long n, char *out) {
    char tmp[32];
    int i = 0;
    int j = 0;
    if (n == 0) {
        out[0] = '0';
        out[1] = 0;
        return;
    }
    while (n != 0) {
        tmp[i] = (char)('0' + (int)(n % 10));
        n = n / 10;
        i = i + 1;
    }
    while (i > 0) {
        i = i - 1;
        out[j] = tmp[i];
        j = j + 1;
    }
    out[j] = 0;
}

int main(void) {
    char buf[32];
    utoa(0, buf);
    printf("%s\n", buf);
    utoa(42, buf);
    printf("%s\n", buf);
    utoa(10000000000UL, buf);
    printf("%s\n", buf);
    return 0;
}
