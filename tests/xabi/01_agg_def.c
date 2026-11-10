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

struct s1 id1(struct s1 s) { return s; }
struct s2 id2(struct s2 s) { return s; }
struct s4 id4(struct s4 s) { return s; }
struct s8 id8(struct s8 s) { return s; }
struct s12 id12(struct s12 s) { return s; }
struct s16 id16(struct s16 s) { return s; }
struct big idbig(struct big s) {
    s.a += 1;
    return s;
}
struct outer idouter(struct outer s) { return s; }
struct arr idarr(struct arr s) { return s; }
int viaptr(struct ptrs s) { return s.c + *s.p; }
int viau(union u9 u) { return u.b[8]; }
int viamix(struct mixed s) { return s.c + s.s + s.i + (int)s.l; }
int apply(int (*f)(int), int x) { return f(x); }
int many(int a, struct s12 s, int b, struct big g, int c) {
    return a + s.b + b + (int)g.c + c;
}
