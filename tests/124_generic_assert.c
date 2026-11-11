#include <stdio.h>
#include <stddef.h>
#include <stdalign.h>
#include <assert.h>
#define TYPE(x) _Generic((x), int: "int", unsigned: "unsigned", long: "long", char: "char", \
    const char *: "const char *", char *: "char *", double: "double", float: "float", \
    int *: "int *", struct S: "struct S", void (*)(void): "fnptr", default: "other")
struct S { int a; char b; _Static_assert(sizeof(int) == 4, "int"); double d; };
_Static_assert(_Alignof(struct S) == 8, "align S");
_Static_assert(sizeof(struct S) == 16, "size S");
static_assert(alignof(char) == 1, "macro form");
enum { A = _Alignof(double), B = _Alignof(char[40]) };
int arr[_Alignof(long double)];
static void fn(void) {}
int main(void) {
    const int ci = 1;
    int a[3];
    char c = 'a';
    const char *cs = "x";
    struct S s = {0};
    volatile long vl = 2;
    _Static_assert(A == 8, "block");
    puts(TYPE(ci));
    puts(TYPE(a));
    puts(TYPE(c));
    puts(TYPE(c + 1));
    puts(TYPE("lit"));
    puts(TYPE(cs));
    puts(TYPE(1.0f));
    puts(TYPE(2.0));
    puts(TYPE(s));
    puts(TYPE(fn));
    puts(TYPE(vl));
    puts(TYPE(3u));
    puts(TYPE((short)1));
    puts(TYPE(&ci));
    int x = 0;
    _Generic(x, int: x, default: c) = 5;
    int n = _Generic(x++, int: 10, long: 20);
    printf("%d %d\n", x, n);
    printf("%zu %zu %zu %d %d %zu\n", _Alignof(int), _Alignof(struct S), _Alignof(long double),
        A, B, sizeof arr / sizeof arr[0]);
    printf("%zu %zu\n", alignof(max_align_t), _Alignof(const volatile short));
    int t[_Generic(1L, long: 3, default: 1)];
    printf("%zu\n", sizeof t);
    switch (2) { case _Generic(1, int: 2, default: 3): puts("case"); }
    for (int i = 0; i < 1; i++) { _Static_assert(1, "loop"); }
    return 0;
}
