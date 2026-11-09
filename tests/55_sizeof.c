int main(void) {
    int a[3];
    int b[2][3];
    int *p;
    int *ap[3];
    int (*pa)[3];
    char c[5];
    if (sizeof(a) != 12ul) return 1;
    if (sizeof(b) != 24ul) return 2;
    if (sizeof(p) != 8ul) return 3;
    if (sizeof(ap) != 24ul) return 4;
    if (sizeof(pa) != 8ul) return 5;
    if (sizeof(c) != 5ul) return 6;
    if (sizeof(int *) != 8ul) return 7;
    if (sizeof(int[3]) != 12ul) return 8;
    if (sizeof(int *[3]) != 24ul) return 9;
    if (sizeof(int (*)[3]) != 8ul) return 10;
    if (sizeof(void *) != 8ul) return 11;
    if (sizeof(*pa) != 12ul) return 12;
    if (sizeof(a[0]) != 4ul) return 13;
    if (sizeof(&a) != 8ul) return 14;
    return 0;
}
