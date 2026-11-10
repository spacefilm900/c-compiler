struct Pair {
    int a;
    int b;
    int c;
};

struct Nested {
    int a[3];
    int b;
};

int main(void) {
    struct Pair s = {.b = 1, 2};
    if (s.a != 0 || s.b != 1 || s.c != 2) {
        return 1;
    }
    int arr[5] = {[2] = 1, 2, 3};
    if (arr[0] != 0 || arr[1] != 0 || arr[2] != 1 || arr[3] != 2 || arr[4] != 3) {
        return 2;
    }
    struct Nested n = {.a[1] = 4, 5, 6};
    if (n.a[0] != 0 || n.a[1] != 4 || n.a[2] != 5 || n.b != 6) {
        return 3;
    }
    int m[2][2] = {[1][0] = 3, 4};
    if (m[0][0] != 0 || m[0][1] != 0 || m[1][0] != 3 || m[1][1] != 4) {
        return 4;
    }
    struct Pair t = {1, .a = 2, 3};
    if (t.a != 2 || t.b != 3 || t.c != 0) {
        return 5;
    }
    int unk[] = {[2] = 5};
    if (sizeof unk != 3 * sizeof(int) || unk[2] != 5) {
        return 6;
    }
    union {
        int a;
        char b;
    } u = {.b = 3};
    if (u.b != 3) {
        return 7;
    }
    return 0;
}
