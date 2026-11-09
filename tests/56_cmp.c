int flags(int *a, int *b) {
    int r = 0;
    if (a == b) r = r + 1;
    if (a != b) r = r + 2;
    if (a < b) r = r + 4;
    if (a <= b) r = r + 8;
    if (a > b) r = r + 16;
    if (a >= b) r = r + 32;
    return r;
}

int main(void) {
    int a[3] = {1, 2, 3};
    int *p = a;
    int *q = &a[2];
    int *z = 0;
    void *v = 0;
    if (flags(p, p) != 1 + 8 + 32) return 1;
    if (flags(p, q) != 2 + 4 + 8) return 2;
    if (flags(q, p) != 2 + 16 + 32) return 3;
    if (z != 0 || z != (void *)0) return 4;
    if (p == 0 || p == (void *)0) return 5;
    if (!p) return 6;
    if (!z == 0) return 7;
    if (z && p) return 8;
    if (!(p && q)) return 9;
    if (z || 0) return 10;
    if (!(p || z)) return 11;
    v = p;
    if (v != (void *)p) return 12;
    if ((p < q) != 1) return 13;
    return 0;
}
