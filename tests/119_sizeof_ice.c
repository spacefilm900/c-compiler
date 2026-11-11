#include <stdio.h>
int a[5];
int g[sizeof a / sizeof a[0]];
static const char msg[] = "hello";
char buf[sizeof msg];
int tbl[] = {1, 2, 3, [9] = 4};
enum { N = sizeof tbl / sizeof *tbl, M = sizeof(g) };
struct P { int x; double d; } pts[] = {{1, 2}, {3, 4}, {5, 6}};
struct B { unsigned f : sizeof(char) * 3; unsigned h : sizeof a[0]; };
long L[sizeof pts / sizeof pts[0] + sizeof(struct P)];
int main(void) {
    char b[sizeof g];
    int loc[] = {1, 2};
    char c2[sizeof loc * 2];
    struct P *pp = pts;
    char c3[sizeof pp->d + sizeof(pts[1].x)];
    int (*fp)(const char *, ...) = printf;
    char c4[sizeof fp("x") + sizeof(1 ? a : 0) + sizeof(&a) + sizeof(*a)];
    switch (3) { case sizeof(int) - 2: return 9; default: break; }
    printf("%zu %zu %d %d %zu %zu %zu %zu %zu\n", sizeof b, sizeof buf, N, M, sizeof L, sizeof c2, sizeof c3, sizeof c4, sizeof(struct B));
    return sizeof b != 20;
}
