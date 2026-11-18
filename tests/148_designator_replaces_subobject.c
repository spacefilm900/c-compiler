/* A later initializer for a whole aggregate subobject overrides everything
   earlier inside it, and the elements its own list omits are zero (C11
   6.7.9p19, p21). A designator for one element, or brace elision, updates
   only what it names. Clang 19 and GCC 14 agree on every value here. */
int printf(const char *, ...);
struct S { int a[2]; int b; };
struct T { struct S s; int z; };
struct P { int a[2]; int b[2]; };
struct Q { char c[4]; };
union U { int x; char y[4]; };
struct V { union U u; int k; };
int main(void) {
  struct S s1 = { .a = {1,2}, .a = {3} };
  struct S s2 = { .a = {1,2}, .a[0] = 3 };
  struct S s3 = { .a = {1,2}, .a = 3 };          /* elision */
  struct T t1 = { .s.a = {1,2}, .s = { .a[1] = 9 } };
  struct T t2 = { .s = { {1,2}, 5 }, .s.a = {7} };
  struct P p1 = { .b = {1,2}, .a = {3,4}, {5} };
  struct Q q1 = { .c = "abc", .c = "x" };
  struct Q q2 = { .c = {1,2,3,4}, .c = "" };
  struct S src = { {8,8}, 8 };
  struct T t3 = { .s = { {1,2}, 5 }, .s = src };
  struct T t4 = { .s.b = 4, .s.a = {6}, .s.a[1] = 7 };
  struct V v1 = { .u.y = {1,2,3,4}, .u = { .y = {9} } };
  struct S arr[2] = { [0] = { {1,2}, 3 }, [1] = { {4,5}, 6 }, [0].a = {7} };
  static struct S g1 = { .a = {1,2}, .a = {3} };
  static struct P g2 = { .b = {1,2}, .a = {3,4}, {5} };
  printf("%d %d %d|%d %d|%d %d %d|", s1.a[0], s1.a[1], s1.b, s2.a[0], s2.a[1], s3.a[0], s3.a[1], s3.b);
  printf("%d %d %d|%d %d %d|", t1.s.a[0], t1.s.a[1], t1.s.b, t2.s.a[0], t2.s.a[1], t2.s.b);
  printf("%d %d %d %d|%d %d %d %d|%d %d %d %d|", p1.a[0], p1.a[1], p1.b[0], p1.b[1], q1.c[0], q1.c[1], q1.c[2], q1.c[3], q2.c[0], q2.c[1], q2.c[2], q2.c[3]);
  printf("%d %d %d|%d %d %d|%d %d %d %d|", t3.s.a[0], t3.s.a[1], t3.s.b, t4.s.a[0], t4.s.a[1], t4.s.b, v1.u.y[0], v1.u.y[1], v1.u.y[2], v1.u.y[3]);
  printf("%d %d %d %d %d %d|%d %d|%d %d %d %d\n", arr[0].a[0], arr[0].a[1], arr[0].b, arr[1].a[0], arr[1].a[1], arr[1].b, g1.a[0], g1.a[1], g2.a[0], g2.a[1], g2.b[0], g2.b[1]);
}
