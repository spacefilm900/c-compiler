int printf(const char *, ...);

struct S { int x; char c; };
struct T { struct S s; int n; };

int main(void) {
    struct S a[3] = {{1, 'a'}, {2, 'b'}};
    struct S b[2] = {3, 'c', 4, 'd'};
    struct T t[2] = {{{5, 'e'}, 6}};
    int i = 0;
    int sum = 0;
    while (i < 3) {
        sum += a[i].x + a[i].c;
        i++;
    }
    printf("%d %d %d\n", sum, a[2].x, a[2].c);
    printf("%d %d %d %d\n", b[0].x, b[0].c, b[1].x, b[1].c);
    printf("%d %d %d %d\n", t[0].s.x, t[0].s.c, t[0].n, t[1].n);
    printf("%lu %lu\n", sizeof(a), sizeof(struct S));
    a[1].c = 'z';
    printf("%d\n", a[1].c);
    return 0;
}
