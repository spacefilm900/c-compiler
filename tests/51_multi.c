int flat(int m[2][3]) {
    return m[0][0] + m[0][2] + m[1][1];
}

int main(void) {
    int m[2][3] = {{1, 2, 3}, {4, 5, 6}};
    int (*row)[3] = &m[1];
    int *e = &m[1][2];
    if (m[0][0] != 1 || m[0][1] != 2 || m[0][2] != 3) return 1;
    if (m[1][0] != 4 || m[1][1] != 5 || m[1][2] != 6) return 2;
    if ((*row)[0] != 4 || (*row)[2] != 6) return 3;
    if (*e != 6) return 4;
    if (flat(m) != 1 + 3 + 5) return 5;
    int n[2][3] = {1, 2, 3, 4};
    if (n[0][2] != 3 || n[1][0] != 4 || n[1][2] != 0) return 6;
    return 0;
}
