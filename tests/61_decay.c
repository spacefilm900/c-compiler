int main(void) {
    int a[3] = {1, 2, 3};
    int *p = a;
    int *q = &a[0];
    int (*whole)[3] = &a;
    int x = 5;
    int *px = &*&x;
    if (sizeof(a) == sizeof(p)) return 1;
    if (p != q) return 2;
    if (sizeof(*whole) != sizeof(a)) return 3;
    if (sizeof(whole) != sizeof(p)) return 4;
    if ((*whole)[1] != 2) return 5;
    if (*px != 5) return 6;
    if (&*p != p) return 7;
    int *stars[2];
    int (*row)[3];
    if (sizeof(stars) != 16ul) return 8;
    if (sizeof(row) != 8ul) return 9;
    row = &a;
    stars[0] = a;
    if (stars[0][2] != 3) return 10;
    return 0;
}
