int by_index(int *a, int n) {
    int s = 0;
    int i;
    for (i = 0; i < n; i = i + 1) s = s + a[i];
    return s;
}

int by_swap(int *a, int n) {
    int s = 0;
    int i;
    for (i = 0; i < n; i = i + 1) s = s + i[a];
    return s;
}

int by_walk(int *a, int n) {
    int s = 0;
    int *p = a;
    int *end = a + n;
    while (p < end) {
        s = s + *p;
        p = p + 1;
    }
    return s;
}

int main(void) {
    int a[5] = {1, 2, 3, 4, 5};
    char b[4] = {1, 2, 3, 4};
    int cs = 0;
    char *p = b;
    while (p != b + 4) {
        cs = cs + *p;
        p++;
    }
    if (by_index(a, 5) != 15) return 1;
    if (by_swap(a, 5) != 15) return 2;
    if (by_walk(a, 5) != 15) return 3;
    if (cs != 10) return 4;
    if (a[0] + a[4] != 6) return 5;
    return 0;
}
