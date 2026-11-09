int printf(const char *, ...);

struct Inner { int x; char y; };
struct Outer { char c; struct Inner in; int z; };
struct Point { int x; int y; };
struct Line { struct Point a; struct Point b; };

int main(void) {
    struct Outer nested = {1, {2, 3}, 4};
    struct Outer flat = {5, 6, 7, 8};
    struct Outer partial = {9};
    struct Point p = {1, 2};
    struct Point q = {3, 4};
    struct Line line = {p, q};
    line.b.y = 9;
    printf("%d %d %d %d\n", nested.c, nested.in.x, nested.in.y, nested.z);
    printf("%d %d %d %d\n", flat.c, flat.in.x, flat.in.y, flat.z);
    printf("%d %d %d %d\n", partial.c, partial.in.x, partial.in.y, partial.z);
    printf("%lu %lu\n", sizeof(struct Inner), sizeof(struct Outer));
    printf("%d %d %d %d\n", line.a.x, line.a.y, line.b.x, line.b.y);
    printf("%d %d\n", p.y, q.x);
    return 0;
}
