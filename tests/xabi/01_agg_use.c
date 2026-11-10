struct s1 {
    unsigned char a;
};
struct s2 {
    unsigned char a, b;
};
struct s4 {
    int a;
};
struct s8 {
    char c;
    int i;
};
struct s12 {
    int a, b, c;
};
struct s16 {
    long a, b;
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
struct mixed {
    char c;
    short s;
    int i;
    long l;
};
union u9 {
    long a;
    unsigned char b[9];
};

struct s1 id1(struct s1);
struct s2 id2(struct s2);
struct s4 id4(struct s4);
struct s8 id8(struct s8);
struct s12 id12(struct s12);
struct s16 id16(struct s16);
struct big idbig(struct big);
struct outer idouter(struct outer);
struct arr idarr(struct arr);
int viaptr(struct ptrs);
int viau(union u9);
int viamix(struct mixed);
int apply(int (*)(int), int);
int many(int, struct s12, int, struct big, int);
int inc(int x) { return x + 1; }

int main(void) {
    struct s1 a = {1};
    if (id1(a).a != 1) {
        return 1;
    }
    struct s2 b = {2, 3};
    if (id2(b).a != 2 || id2(b).b != 3) {
        return 2;
    }
    struct s4 c = {4};
    if (id4(c).a != 4) {
        return 3;
    }
    struct s8 d = {5, 6};
    if (id8(d).c != 5 || id8(d).i != 6) {
        return 4;
    }
    struct s12 e = {7, 8, 9};
    if (id12(e).a != 7 || id12(e).b != 8 || id12(e).c != 9) {
        return 5;
    }
    struct s16 f = {10, 11};
    if (id16(f).a != 10 || id16(f).b != 11) {
        return 6;
    }
    struct big g = {12, 13, 14};
    struct big h = idbig(g);
    if (g.a != 12 || h.a != 13 || h.b != 13 || h.c != 14) {
        return 7;
    }
    struct outer o = {{15, 16}, 17};
    struct outer p = idouter(o);
    if (p.n.c != 15 || p.n.i != 16 || p.z != 17) {
        return 8;
    }
    struct arr q = {{18, 19, 20}};
    struct arr r = idarr(q);
    if (r.v[0] != 18 || r.v[1] != 19 || r.v[2] != 20) {
        return 9;
    }
    int n = 21;
    struct ptrs ps = {1, &n};
    if (viaptr(ps) != 22) {
        return 10;
    }
    union u9 u;
    u.a = 0;
    u.b[8] = 23;
    if (viau(u) != 23) {
        return 11;
    }
    struct mixed m = {1, 2, 3, 4};
    if (viamix(m) != 10) {
        return 12;
    }
    if (apply(inc, 41) != 42) {
        return 13;
    }
    if (many(1, e, 2, g, 3) != 1 + 8 + 2 + 14 + 3) {
        return 14;
    }
    return 0;
}
