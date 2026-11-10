struct A {
    int a : 3;
    int b : 5;
};

struct O {
    struct A in;
    int n;
};

struct O g = {{1, 2}, 9};

struct A arr[2] = {{1, 2}, {3, -1}};

struct W {
    struct A in;
    int *p;
};

int glob = 5;

struct W w = {{1, 2}, &glob};

struct B {
    int a : 3;
    char c;
};

struct P {
    struct B in;
    int n;
};

struct P plain = {{1, 'x'}, 9};

struct Q {
    struct B in;
    int *p;
};

struct Q qplain = {{2, 'y'}, &glob};

struct R {
    struct B in;
    int *p;
};

struct R designated = {.in.c = 'z', .p = &glob};

int main(void) {
    if (g.in.a != 1 || g.in.b != 2 || g.n != 9) {
        return 1;
    }
    if (arr[0].a != 1 || arr[0].b != 2) {
        return 2;
    }
    if (arr[1].a != 3 || arr[1].b != -1) {
        return 3;
    }
    if (w.in.a != 1 || w.in.b != 2 || *w.p != 5) {
        return 4;
    }
    struct O local = {{3, -1}, 4};
    if (local.in.a != 3 || local.in.b != -1 || local.n != 4) {
        return 5;
    }
    if (plain.in.a != 1 || plain.in.c != 'x' || plain.n != 9) {
        return 6;
    }
    if (qplain.in.a != 2 || qplain.in.c != 'y' || *qplain.p != 5) {
        return 7;
    }
    if (designated.in.a != 0 || designated.in.c != 'z' || *designated.p != 5) {
        return 8;
    }
    return 0;
}
