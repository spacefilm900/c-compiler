/* GNU C: a struct ending in a flexible array member may be a member of
   another struct, and a record of only zero-length arrays has size 0
   (GCC manual, "Arrays of Length Zero"). QuickJS nests its allocator's
   block header and gives each string a zero-size union of `str8[0]` and
   `str16[0]` at its end. */
int printf(const char *, ...);
struct E { char none[0]; };
union Z { char a[0]; short b[0]; };
struct H { unsigned len; union Z u; };
struct W { int x; struct E e; int y; };
struct Block { unsigned short idx; unsigned char size; int refs; unsigned char data[]; };
struct Large { long link; struct Block header; };
static struct E pass(struct E e, int k) { (void)e; printf("k=%d\n", k); return e; }
static int sum(int a, struct E e, double d, struct E f, long g) { (void)e; (void)f; return a + (int)d + (int)g; }
static int vsum(int n, ...) { __builtin_va_list ap; __builtin_va_start(ap, n); int a = __builtin_va_arg(ap, int); struct E e = __builtin_va_arg(ap, struct E); int b = __builtin_va_arg(ap, int); __builtin_va_end(ap); (void)e; return a * 10 + b; }
struct E ge; struct E ga[4];
int main(void) {
    struct H h = { 5 };
    struct W w = { 1, {}, 2 };
    struct E e = {}, f;
    f = pass(e, 3);
    struct E *p = &ga[0];
    printf("%zu %zu %zu %zu %zu %zu %u %d %d %d %d\n", sizeof(struct E), sizeof(union Z), sizeof(struct H),
           sizeof(struct W), __builtin_offsetof(struct W, y), sizeof ga, h.len, w.x, w.y,
           sum(1, e, 2.0, f, 3), vsum(3, 4, e, 5));
    printf("%zu %zu %zu\n", sizeof(struct Block), sizeof(struct Large),
           __builtin_offsetof(struct Large, header));
    printf("%d %d\n", (int)((char *)&h.u - (char *)&h), (int)((char*)(p + 1) - (char*)p));
    return 0;
}
