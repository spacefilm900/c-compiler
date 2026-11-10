struct LD {
    long double x;
};

union LU {
    long double x;
};

struct Big {
    long double a;
    long double b;
};

struct Mix {
    int i;
    long double x;
};

struct LD id(struct LD s) {
    return s;
}

struct LD make(long double v) {
    struct LD s;
    s.x = v;
    return s;
}

union LU idu(union LU u) {
    return u;
}

struct Big idb(struct Big s) {
    return s;
}

struct Mix idm(struct Mix s) {
    return s;
}

int main(void) {
    struct LD s = make(1.5L);
    if (s.x != 1.5L) {
        return 1;
    }
    s.x = 2.5L;
    struct LD t = id(s);
    if (t.x != 2.5L) {
        return 2;
    }
    union LU u;
    u.x = 3.5L;
    if (idu(u).x != 3.5L) {
        return 3;
    }
    struct Big b;
    b.a = 1.0L;
    b.b = 2.0L;
    struct Big c = idb(b);
    if (c.a != 1.0L || c.b != 2.0L) {
        return 4;
    }
    struct Mix m;
    m.i = 7;
    m.x = 4.0L;
    struct Mix n = idm(m);
    if (n.i != 7 || n.x != 4.0L) {
        return 5;
    }
    return 0;
}
