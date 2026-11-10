struct S {
    int n;
    int a[];
};

struct T {
    char c;
    int a[];
};

struct D {
    int n;
    double d[];
};

struct S id(struct S s) {
    return s;
}

int main(void) {
    struct S probe_s;
    struct T probe_t;
    struct D probe_d;
    if (sizeof(struct S) != 4 || (char *)&probe_s.a - (char *)&probe_s != 4) {
        return 1;
    }
    if (sizeof(struct T) != 4 || (char *)&probe_t.a - (char *)&probe_t != 4) {
        return 2;
    }
    if (sizeof(struct D) != 8 || (char *)&probe_d.d - (char *)&probe_d != 8) {
        return 3;
    }
    struct S local = {7};
    if (local.n != 7 || sizeof local != 4) {
        return 4;
    }
    struct {
        int n;
        int extra[2];
    } buf;
    buf.n = 2;
    buf.extra[0] = 10;
    buf.extra[1] = 20;
    struct S *p = (struct S *)&buf;
    if (p->n != 2 || p->a[0] != 10 || p->a[1] != 20) {
        return 5;
    }
    p->a[0] = 11;
    if (buf.extra[0] != 11) {
        return 6;
    }
    struct S copy = id(local);
    if (copy.n != 7) {
        return 7;
    }
    return 0;
}
