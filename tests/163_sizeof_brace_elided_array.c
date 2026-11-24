/* C11 6.7.9p20, p22: an array of unknown size is completed by its
   initializer, also when the initializer elides the braces of its
   elements or uses designators that pick an element. `sizeof` of it is
   then an integer constant expression (6.6p6), usable in an array bound,
   an enumerator or _Static_assert. hcc left such arrays incomplete for
   the parser's constant folding and rejected those uses. */
int printf(const char *, ...);

struct P { int x, y; };
struct N { char c; struct P p; int a[2]; };
struct B { int a : 3; int : 5; int b : 4; };
union U { int i; char c[8]; };
struct A { int a; struct { int b, c; }; int d; };
struct M { struct P p[2]; int z; };

struct P p1[] = {1, 2, 3};
struct P p2[] = {1, 2, 3, 4};
struct N n1[] = {1, 2, 3, 4, 5, 6, 7, 8};
struct N n2[] = {1, {2, 3}, 4, 5, 6};
struct N n3[] = {1, 2, 3, {4, 5}, {6}, 7};
struct B b1[] = {1, 2, 3, 4, 1};
union U u1[] = {1, 2, 3};
struct A a1[] = {1, 2, 3, 4, 5};
struct M m1[] = {1, 2, 3, 4, 5, 6, 7};
int grid[][3] = {1, 2, 3, 4};
double d1[][2] = {1.5, -2.5, 3.0};
struct P desig[] = {[2] = {1, 2}, 3, 4, [1] = 5, 6};
char strs[][4] = {[1] = "ab", "cde", [0] = "x"};
struct P mixed[] = {{1, 2}, 3, 4, (1 ? 5 : 6), sizeof(int)};

char q_p1[sizeof p1];
char q_p2[sizeof p2];
char q_n1[sizeof n1];
char q_n2[sizeof n2];
char q_n3[sizeof n3];
char q_b1[sizeof b1];
char q_u1[sizeof u1];
char q_a1[sizeof a1];
char q_m1[sizeof m1];
char q_grid[sizeof grid];
char q_d1[sizeof d1];
char q_mixed[sizeof mixed];
char q_desig[sizeof desig];
char q_strs[sizeof strs];

enum { E_N1 = sizeof n1 / sizeof n1[0], E_M1 = sizeof m1 / sizeof m1[0] };
_Static_assert(sizeof p1 / sizeof p1[0] == 2, "p1 has two elements");

struct Q { const char *s; int n; };
struct Q qs[] = {"a", 1, "b", 2, "c"};
enum { E_QS = sizeof qs / sizeof qs[0] };
_Static_assert(sizeof q_p1 == 16, "p1");
_Static_assert(sizeof q_p2 == 16, "p2");
_Static_assert(sizeof q_n1 == 40, "n1");
_Static_assert(sizeof q_n2 == 40, "n2");
_Static_assert(sizeof q_n3 == 60, "n3");
_Static_assert(sizeof q_b1 == 12, "b1");
_Static_assert(sizeof q_u1 == 24, "u1");
_Static_assert(sizeof q_a1 == 32, "a1");
_Static_assert(sizeof q_m1 == 40, "m1");
_Static_assert(sizeof q_grid == 24, "grid");
_Static_assert(sizeof q_d1 == 32, "d1");
_Static_assert(sizeof q_mixed == 24, "mixed");
_Static_assert(sizeof q_desig == 32, "desig");
_Static_assert(sizeof q_strs == 12, "strs");
_Static_assert(E_QS == 3, "qs");

/* Items that are scalars whatever their operands are counted, so `W` is
   an integer constant expression. */
int more(void) {
    int i = 0, j = 5, k = 1;
    struct P w[] = {i++, j += 2, (char)3, k ? 1 : 2, (long)&k != 0};
    enum { W = sizeof w / sizeof w[0] };
    return W * 10 + w[1].x;
}

int local(void) {
    struct P lp[] = {1, 2, 3, 4, 5};
    char q[sizeof lp];
    return (int)sizeof q;
}

int main(void) {
    printf("%d %d %d %d %d %d\n", (int)sizeof q_p1, (int)sizeof q_p2, (int)sizeof q_n1, (int)sizeof q_n2,
           (int)sizeof q_n3, (int)sizeof q_b1);
    printf("%d %d %d %d %d %d\n", (int)sizeof q_u1, (int)sizeof q_a1, (int)sizeof q_m1, (int)sizeof q_grid,
           (int)sizeof q_d1, (int)sizeof q_mixed);
    printf("%d %d %d %d %d\n", E_N1, E_M1, local(), (int)sizeof q_desig, (int)sizeof q_strs);
    printf("%d %d %s\n", E_QS, more(), qs[2].s);
    printf("%d %d %d %d\n", n2[1].c, n3[1].p.x, m1[1].p[0].y, mixed[2].x);
    return 0;
}
