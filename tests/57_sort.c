void sort(int *a, int n) {
    int i = 0;
    while (i < n) {
        int j = 0;
        while (j + 1 < n - i) {
            if (*(a + j) > *(a + j + 1)) {
                int t = a[j];
                a[j] = a[j + 1];
                a[j + 1] = t;
            }
            j++;
        }
        i++;
    }
}

int main(void) {
    int a[6] = {4, 1, 5, 2, 6, 3};
    sort(a, 6);
    if (a[0] != 1 || a[1] != 2 || a[2] != 3) return 1;
    if (a[3] != 4 || a[4] != 5 || a[5] != 6) return 2;
    return 0;
}
