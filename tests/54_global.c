int g = 7;
int arr[4] = {1, 2};
int raw[] = {3, 1, 4};
int m[2][3] = {{1, 2}, {4}};
int flat[2][3] = {1, 2, 3, 4};
int *p = &g;
int *q = &arr[2];
int *r = arr + 1;
int *s = raw;
int *t = &m[1][0];
int (*row)[3] = &m[1];
int *np = 0;
int *np2 = (void *)0;
long gl = (long)&g;
int *via = (int *)(unsigned long)&g;
static int hidden[3];

int main(void) {
    if (*p != 7) return 1;
    if (arr[0] != 1 || arr[1] != 2 || arr[2] != 0 || arr[3] != 0) return 2;
    if (*q != 0 || *r != 2) return 3;
    if (s[0] != 3 || s[2] != 4) return 4;
    if (*t != 4 || (*row)[0] != 4 || (*row)[1] != 0) return 5;
    if (flat[1][0] != 4 || flat[1][2] != 0) return 6;
    if (np != 0 || np2 != 0) return 7;
    if (gl != (long)p || via != p) return 8;
    if (hidden[0] != 0 || hidden[2] != 0) return 9;
    if ((int)sizeof(raw) != 12) return 10;
    return 0;
}
