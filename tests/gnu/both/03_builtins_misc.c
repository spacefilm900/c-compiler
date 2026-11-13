/* Builtins that map onto LLVM intrinsics or C library calls (GCC manual,
   "Other Built-in Functions"). */

int printf(const char *, ...);

struct s { char c; int a[4]; double d; };
static int global = 3;

static int pick(int v)
{
    if (__builtin_expect(v > 10, 0))
        return -1;
    switch (v) {
    case 1: return 10;
    case 2: return 20;
    }
    if (v == 99)
        __builtin_unreachable();
    return v;
}

static void never(void) { __builtin_trap(); }

int main(void)
{
    int x = 4;
    char buf[32];
    char *heap = __builtin_alloca(16);
    __builtin_memset(heap, 'z', 15);
    heap[15] = 0;
    __builtin_memcpy(buf, "hello", 6);
    __builtin_strcpy(buf + 5, "!");
    printf("%d %d %d %d\n", pick(1), pick(2), pick(5), pick(50));
    printf("%d %d %d %d %d\n", __builtin_constant_p(3), __builtin_constant_p(3 + 4 * 2),
           __builtin_constant_p(x), __builtin_constant_p(global), __builtin_constant_p(1.5));
    printf("%d %d %d %d %d\n", __builtin_types_compatible_p(int, int),
           __builtin_types_compatible_p(int, const int), __builtin_types_compatible_p(int, long),
           __builtin_types_compatible_p(int *, int[]), __builtin_types_compatible_p(char, signed char));
    printf("%d %zu\n", __builtin_choose_expr(1, 10, 20.0), sizeof(__builtin_choose_expr(0, 'a', 2.0)));
    printf("%zu %zu\n", __builtin_offsetof(struct s, a[2]), __builtin_offsetof(struct s, d));
    printf("%s %zu %d %d\n", buf, __builtin_strlen(buf), __builtin_strcmp(buf, "hello!") == 0,
           __builtin_memcmp(buf, "help", 3));
    printf("%s %d %ld %.1f\n", heap, __builtin_abs(-5), __builtin_labs(-6L), __builtin_fabs(-2.5));
    __builtin_printf("%d %s\n", 42, "via builtin");
    __builtin_prefetch(buf);
    __builtin_prefetch(buf, 1, 3);
    char out[16];
    __builtin_sprintf(out, "%d", 77);
    __builtin_snprintf(out + 2, 8, "%c", 'q');
    printf("%s %d %d\n", out, __builtin_isnan(0.0 / 0.0), __builtin_isinf(1.0 / 0.0));
    if (x == 5) {
        never();
        __builtin_abort();
    }
    void *ret = __builtin_return_address(0);
    void *frame = __builtin_frame_address(0);
    printf("%d %d\n", ret != 0, frame != 0);
    if (x == 4)
        return 0;
    return 1;
}
