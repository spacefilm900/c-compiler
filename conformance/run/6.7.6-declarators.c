/* N1570 6.7.6 declarators, 6.7.7 type names, 6.7.8 type definitions */
#include "check.h"

static int add1(int x) { return x + 1; }
static int (*pick(int which))(int) { return which ? add1 : 0; }   /* function returning a pointer to function */
static int table[3][4];
static int (*row(int i))[4] { return &table[i]; }
typedef int (*binop)(int, int);
static int addi(int a, int b) { return a + b; }
static int apply(binop f, int a, int b) { return f(a, b); }
static int kr(a, b) int a; double b; { return a + (int)b; }    /* 6.9.1: identifier list */
static int vla_sum(int n, int m, int a[n][m]) { int s = 0; for (int i = 0; i < n; i++) for (int j = 0; j < m; j++) s += a[i][j]; return s; }
static int proto_vla(int n, int a[*]);
static int proto_vla(int n, int a[n]) { return a[n - 1]; }
static void noargs(void) {}

int main(void) {
    int *ap[3], (*pa)[3], **pp, *(*pfa[2])(void);
    int x[3] = { 1, 2, 3 };
    pa = &x; ap[0] = &x[1]; pp = ap;
    CHECK((*pa)[2] == 3 && **pp == 2);
    (void)pfa;
    CHECK(pick(1)(4) == 5 && pick(0) == 0);
    (*row(1))[2] = 7;
    CHECK(table[1][2] == 7);
    CHECK(apply(addi, 2, 3) == 5 && kr(1, 2.5) == 3);
    int n = 2, m = 3;
    int vla[n][m];
    for (int i = 0; i < n; i++) for (int j = 0; j < m; j++) vla[i][j] = i + j;
    CHECK(vla_sum(n, m, vla) == 0 + 1 + 2 + 1 + 2 + 3);
    CHECK(sizeof vla == 6 * sizeof(int) && sizeof vla[0] == 3 * sizeof(int));
    typedef int VLA[n];
    n = 10;
    VLA later;
    CHECK(sizeof later == 2 * sizeof(int));     /* 6.7.8p3: a VLA typedef's size is fixed at the definition */
    CHECK(proto_vla(3, x) == 3);
    CHECK(sizeof(int (*)[3]) == sizeof(void *) && sizeof(int [2][5]) == 10 * sizeof(int));
    CHECK(sizeof(int (*)(void)) == sizeof(void (*)(int)));
    noargs();
    typedef struct { int a; } T, *PT;
    T t = { 3 };
    PT pt = &t;
    CHECK(pt->a == 3);
    int (x2) = 4, ((y2)) = 5;                    /* redundant parentheses */
    CHECK(x2 + y2 == 9);
    return DONE();
}
