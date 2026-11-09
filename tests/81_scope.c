int printf(const char *, ...);

struct S { int x; };

int outer_size(void) {
    return sizeof(struct S);
}

int main(void) {
    struct S outer;
    outer.x = 1;
    int inner_sum = 0;
    {
        struct S { int x; int y; } inner;
        inner.x = 2;
        inner.y = 3;
        inner_sum = inner.x + inner.y;
        if (sizeof(inner) != 8ul) {
            return 1;
        }
    }
    if (sizeof(outer) != 4ul) {
        return 2;
    }
    if (outer.x != 1) {
        return 3;
    }
    if (outer_size() != 4) {
        return 4;
    }
    struct Name { char text[4]; int n; };
    struct Name name = {"ok", 5};
    printf("%d %d %c %c %d\n", inner_sum, outer.x, name.text[0], name.text[1], name.n);
    struct S *p = &outer;
    printf("%d\n", p->x);
    return 0;
}
