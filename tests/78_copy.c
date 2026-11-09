int printf(const char *, ...);

struct S {
    int a;
    char b;
    int c;
    int arr[2];
};

void copy(struct S *dst, struct S *src) {
    *dst = *src;
}

int main(void) {
    struct S s;
    s.a = 1;
    s.b = 2;
    s.c = 3;
    s.arr[0] = 4;
    s.arr[1] = 5;
    struct S t;
    t = s;
    t.a = 9;
    t.arr[0] = 8;
    struct S u;
    copy(&u, &s);
    s.c = 7;
    printf("%d %d %d %d %d\n", s.a, s.c, t.a, t.c, t.arr[0]);
    printf("%d %d %d %d\n", u.a, u.b, u.c, u.arr[1]);
    const struct S cs = {1, 2, 3, {4, 5}};
    struct S from_const;
    from_const = cs;
    printf("%d %d %d\n", from_const.a, from_const.arr[1], cs.c);
    struct Inner { int x; int y; };
    struct Wrap { struct Inner in; };
    struct Inner iv = {1, 2};
    struct Wrap w = {iv};
    printf("%d\n", w.in.x + w.in.y);
    return 0;
}
