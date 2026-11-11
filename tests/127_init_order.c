int printf(const char *, ...);
struct S { int x, y; };
struct T { struct S s; int z; };
struct B { struct S s; unsigned k : 3; };
int main(void) {
    struct S one = {1, 9};
    struct T t = {.s = (struct S){1, 9}, .s.x = 2};
    struct T u = {.s = one, .s.y = 7, .z = 3};
    struct B b = {.s = one, .k = 5, .s.x = 4};
    printf("%d %d %d\n", t.s.x, t.s.y, t.z);
    printf("%d %d %d\n", u.s.x, u.s.y, u.z);
    printf("%d %d %u\n", b.s.x, b.s.y, b.k);
    return 0;
}
