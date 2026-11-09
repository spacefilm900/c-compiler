int sum(int a[], int n) {
    int s = 0;
    int i;
    for (i = 0; i < n; i = i + 1) s = s + a[i];
    return s;
}

int sized(int a[8]) {
    a[0] = a[0] + 1;
    return (int)sizeof(a);
}

int row(int m[][3]) { return m[1][2]; }

int main(void) {
    int a[4] = {1, 2, 3, 4};
    if (sum(a, 4) != 10) return 1;
    if (sized(a) != (int)sizeof(int *)) return 2;
    if (a[0] != 2) return 3;
    int m[2][3] = {{1, 2, 3}, {4, 5, 6}};
    if (row(m) != 6) return 4;
    if ((int)sizeof(m) != 24) return 5;
    return 0;
}
