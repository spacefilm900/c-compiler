/* Variable-length arrays (C11 6.7.6.2, 6.5.3.4p2, 6.7.6.3). */

int printf(const char *, ...);

int fsize3(int n)
{
    char b[n + 3];
    b[0] = 1;
    b[n + 2] = 2;
    return (int)sizeof b;
}

int sum2(int n, int a[n][n])
{
    int s = 0;
    int i, j;
    for (i = 0; i < n; i++)
        for (j = 0; j < n; j++)
            s += a[i][j];
    return s;
}

int side_decl(void)
{
    int n = 4;
    int a[n++];
    a[0] = 1;
    a[3] = 2;
    return n + (int)(sizeof a / sizeof a[0]);
}

int side_typedef(void)
{
    int n = 3;
    typedef int T[n++];
    T x;
    x[0] = 1;
    return n + (int)(sizeof x / sizeof(int)) + (int)(sizeof(T) / sizeof(int));
}

int side_sizeof_type(void)
{
    int n = 5;
    int s = (int)sizeof(int[n++]);
    return n + s / (int)sizeof(int);
}

int alignof_uneval(void)
{
    int n = 7;
    int a = (int)_Alignof(int[n++]);
    return n + a;
}

int loop_reuse(void)
{
    int n = 16;
    int i;
    int acc = 0;
    for (i = 0; i < 1000000; i++) {
        char buf[n];
        buf[0] = (char)i;
        buf[n - 1] = 1;
        acc += buf[0] + buf[n - 1];
    }
    return acc;
}

int ptr_arith(int n)
{
    int a[n];
    int (*p)[n];
    int i;
    for (i = 0; i < n; i++)
        a[i] = i + 1;
    p = &a;
    return (*p)[n - 1] + (int)(sizeof(*p) / sizeof(int));
}

int param_side(int i, int array[i++])
{
    (void)array;
    return i;
}

int param_side2(int a, int b[a++], int c, int d[c++])
{
    (void)b;
    (void)d;
    return a + c;
}

int goto_reuse(void)
{
    int n = 0;
    void *volatile p;
lab:;
    char x[n % 1000 + 1];
    x[0] = 1;
    x[n % 1000] = 2;
    p = x;
    n++;
    if (n < 1000000)
        goto lab;
    return n;
}

int goto_inner_lab(void)
{
    int n = 0;
    void *volatile p;
    if (0) {
    lab:;
    }
    char x[n % 1000 + 1];
    x[0] = 1;
    x[n % 1000] = 2;
    p = x;
    n++;
    if (n < 1000000)
        goto lab;
    return n;
}

int inherit_size(void)
{
    int n = 2, m = 3;
    typedef int T[n++];
    T a[m++];
    return n + m + (int)(sizeof(T) / sizeof(int)) + (int)(sizeof(a) / sizeof(int));
}

int cast_side(void)
{
    int n = 3;
    (void)(int (*)[n++])0;
    return n;
}

int ptrdiff_size(void)
{
    int n = 4;
    int a[2][n];
    return (int)sizeof(a - a);
}

struct S { int x, y; };

struct S ret_elem(int n)
{
    struct S a[n];
    a[0].x = 1;
    a[0].y = 2;
    return a[0];
}

int main(void)
{
    int m = 3;
    int square[3][3] = {{1, 2, 3}, {4, 5, 6}, {7, 8, 9}};
    printf("%d\n", fsize3(10));
    printf("%d\n", sum2(m, square));
    printf("%d\n", side_decl());
    printf("%d\n", side_typedef());
    printf("%d\n", side_sizeof_type());
    printf("%d\n", alignof_uneval());
    printf("%d\n", loop_reuse());
    printf("%d\n", ptr_arith(4));
    {
        int buf[10];
        printf("%d\n", param_side(10, buf));
        printf("%d\n", param_side2(1, buf, 1, buf));
    }
    printf("%d\n", goto_reuse());
    printf("%d\n", goto_inner_lab());
    printf("%d\n", inherit_size());
    printf("%d\n", cast_side());
    printf("%d\n", ptrdiff_size());
    {
        struct S s = ret_elem(4);
        printf("%d\n", s.x + s.y);
    }
    return 0;
}
