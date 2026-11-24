/* Type qualifiers and `static` in the brackets of a parameter's array
   declarator (C11 6.7.6.2p1, 6.7.6.3p7): the qualifiers go to the pointer
   the parameter becomes, `static` is a promise of at least N elements.
   QuickJS declares `double fields[static 9]`. */
int printf(const char *, ...);
static double sum(int n, const double v[static 3]) { double s = 0; for (int i = 0; i < n; i++) s += v[i]; return s; }
static int first(int a[const static 2]) { return a[0] + a[1]; }
static int g(int m[static 2][3]) { return m[1][2]; }
static int h(int n, int a[restrict n]) { return a[n - 1]; }
static int k(int a[volatile]) { return a[0]; }
static int proto(int [static 4]);
static int proto(int a[4]) { return a[3]; }
int main(void) {
    double v[4] = { 1, 2, 3, 4 };
    int a[4] = { 5, 6, 7, 8 };
    int m[2][3] = { { 1, 2, 3 }, { 4, 5, 6 } };
    printf("%g %d %d %d %d %d %zu\n", sum(4, v), first(a), g(m), h(3, a), k(a), proto(a), sizeof(int (*)(int [static 1])));
    return 0;
}
