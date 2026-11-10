union U {
    int a;
    char b;
};

struct S {
    union U u;
    int y;
};

union P {
    int *a;
    long b;
};

union P np = {0};
union P ip = {(int *)1};

struct Z {
    int a;
    union {
        char *s;
        long n;
    } v;
};

struct Z z = {1};

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
    _Bool from_ptr = p;
    _Bool from_null = q;
    if (from_ptr != 1) {
        return 5;
    }
    if (from_null != 0) {
        return 6;
    }
    if (np.b != 0 || ip.b != 1) {
        return 7;
    }
    if (z.a != 1 || z.v.s != 0) {
        return 8;
    }
    return 0;
}
