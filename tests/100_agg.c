struct s1 {
    unsigned char a;
};
struct s2 {
    unsigned char a, b;
};
struct s4 {
    short a;
    char b;
};
struct s8 {
    char c;
    int i;
};
struct s12 {
    int a, b, c;
};
struct s16 {
    long a;
    int b;
};
struct big {
    long a, b, c;
};
struct nest {
    char c;
    int i;
};
struct outer {
    struct nest n;
    char z;
};
struct arr {
    int v[3];
};
struct ptrs {
    char c;
    int *p;
};
union u16 {
    long a;
    unsigned char b[9];
};

struct s1 id1(struct s1 s) { return s; }
struct s8 id8(struct s8 s) { return s; }
struct s12 id12(struct s12 s) { return s; }
struct big idbig(struct big s) {
    s.b += 1;
    return s;
}
struct arr make(void) {
    struct arr a = {{1, 2, 3}};
    return a;
}
int pick(int c, struct s8 a, struct s8 b) { return (c ? a : b).i; }

int main(void) {
    struct s1 a = {7};
    if (id1(a).a != 7) {
        return 1;
    }
    struct s2 b = {1, 2};
    if (b.a + b.b != 3) {
        return 2;
    }
    struct s4 c = {8, 9};
    if (c.a != 8 || c.b != 9) {
        return 3;
    }
    struct s8 d = {1, 42};
    struct s8 e = id8(d);
    if (e.c != 1 || e.i != 42 || id8(d).i != 42) {
        return 4;
    }
    struct s12 f = {3, 4, 5};
    if (id12(f).a != 3 || id12(f).c != 5) {
        return 5;
    }
    struct s16 g = {9, 8};
    if (g.a != 9 || g.b != 8) {
        return 6;
    }
    struct big h = {10, 20, 30};
    struct big i = idbig(h);
    if (h.b != 20 || i.a != 10 || i.b != 21 || i.c != 30) {
        return 7;
    }
    struct outer j = {{4, 5}, 6};
    if (j.n.c != 4 || j.n.i != 5 || j.z != 6) {
        return 8;
    }
    if (make().v[0] != 1 || make().v[1] != 2 || make().v[2] != 3) {
        return 9;
    }
    struct s8 k = {0, 1};
    struct s8 m = {0, 2};
    if (pick(1, k, m) != 1 || pick(0, k, m) != 2) {
        return 10;
    }
    int n = 40;
    struct ptrs p = {2, &n};
    if (p.c + *p.p != 42) {
        return 11;
    }
    union u16 u;
    u.a = 0;
    u.b[8] = 6;
    if (u.b[8] != 6) {
        return 12;
    }
    return 0;
}
