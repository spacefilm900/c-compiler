union U {
    int a;
    char b;
};

struct S {
    union U u;
    int y;
};

int main(void) {
    int x = 1;
    int *p = &x;
    int *q = 0;
    union U arr[2] = {1, 2};
    struct S s = {3, 4};
    if ((_Bool)p != 1) {
        return 1;
    }
    if ((_Bool)q != 0) {
        return 2;
    }
    if (arr[0].a != 1 || arr[1].a != 2) {
        return 3;
    }
    if (s.u.a != 3 || s.y != 4) {
        return 4;
    }
    return 0;
}
