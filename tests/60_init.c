int unsized[] = {1, 2, 3};
int nested[][3] = {{1, 2}, {4, 5, 6}};
int flat[][2] = {1, 2, 3};
int *ps[2];
int g = 1;
int *pg = &g;

int main(void) {
    int a[4] = {1, 2};
    int b[] = {7, 8, 9};
    int m[2][3] = {{1}, {2, 3}};
    int s = {4};
    int *local[2] = {&s, 0};
    if (a[0] != 1 || a[1] != 2 || a[2] != 0 || a[3] != 0) return 1;
    if (sizeof(b) != 12ul || b[2] != 9) return 2;
    if (m[0][0] != 1 || m[0][2] != 0) return 3;
    if (m[1][0] != 2 || m[1][1] != 3 || m[1][2] != 0) return 4;
    if (s != 4) return 5;
    if (*local[0] != 4 || local[1] != 0) return 6;
    if (sizeof(unsized) != 12ul || unsized[1] != 2) return 7;
    if (nested[0][1] != 2 || nested[0][2] != 0) return 8;
    if (nested[1][0] != 4 || nested[1][2] != 6) return 9;
    if (sizeof(flat) != 16ul || flat[1][0] != 3 || flat[1][1] != 0) return 10;
    if (ps[0] != 0 || *pg != 1) return 11;
    return 0;
}
