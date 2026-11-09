int printf(const char *, ...);
void *memcpy(void *, const void *, unsigned long);

struct Pad {
    char c;
    int n;
};

int pad_of(struct Pad *p) {
    unsigned char *b = (unsigned char *)p;
    return (int)b[1] + (int)b[2] + (int)b[3];
}

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

    unsigned char raw[8];
    int i = 0;
    while (i < 8) {
        raw[i] = (unsigned char)(10 + i);
        i++;
    }
    struct Pad src;
    memcpy(&src, raw, 8);
    struct Pad dst;
    dst = src;
    dst = dst;
    struct Pad mid;
    mid = dst;
    struct Pad chained;
    chained = (mid = src);
    struct Pad init = src;
    printf("%d %d %d %d\n", chained.c, pad_of(&chained), pad_of(&dst), pad_of(&init));
    return 0;
}
