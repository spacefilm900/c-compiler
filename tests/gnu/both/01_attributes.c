/* GNU attributes (GCC manual, "Attribute Syntax", "Common Function
   Attributes", "Common Variable Attributes", "Common Type Attributes",
   "Statement Attributes"). The `__attribute__` keyword is reserved, so
   strict C11 accepts these too. */

int printf(const char *, ...);
void exit(int);

static int counter __attribute__((used)) = 1;
static int unused_var __attribute__((unused));
int aligned_global __attribute__((aligned(64))) = 3;
int __attribute__((section(".data.custom"))) in_section = 4;
int weak_value __attribute__((weak)) = 5;

typedef struct { int v; } __attribute__((aligned(16))) aligned_rec;
typedef struct __attribute__((packed)) {
    char c;
    int i;
    short s;
} packed_t;

struct __attribute__((packed)) tagged { char c; long l; };
struct trailing { char c; int i; } __attribute__((packed));
struct member_packed { char c; int i __attribute__((packed)); char d; };
struct member_aligned { char c; int i __attribute__((aligned(8))); };
struct __attribute__((aligned(32))) big_align { int x; };
union __attribute__((packed)) pu { char c; int i; };

__attribute__((noreturn)) static void die(int code) { exit(code); }
static int twice(int) __attribute__((const));
static int twice(int v) { return v * 2; }
static int readg(void) __attribute__((pure));
static int readg(void) { return counter; }
static inline __attribute__((always_inline)) int inl(int v) { return v + 1; }
__attribute__((noinline)) static int noinl(int v) { return v - 1; }
__attribute__((cold)) static int cold_path(void) { return 7; }
__attribute__((hot)) static int hot_path(void) { return 8; }
__attribute__((format(printf, 1, 2))) static int fmt(const char *f, ...) { return f[0]; }
__attribute__((deprecated)) static int old(void) { return 9; }
__attribute__((deprecated("use new"))) static int old2(void) { return 10; }
__attribute__((weak)) int weak_fn(void) { return 11; }
__attribute__((visibility("default"), nothrow, warn_unused_result)) static int many(void) { return 12; }
static int __attribute__((__unused__)) spelled(void) { return 13; }
__attribute__((some_unknown_attribute(1, "x"))) static int unknown(void) { return 14; }

int main(void)
{
    aligned_rec ai = {2};
    int local_aligned __attribute__((aligned(32))) = 6;
    char bytes[3] __attribute__((aligned(16))) = {1, 2, 3};
    packed_t p = {1, 2, 3};
    int x __attribute__((unused)) = 0;
    int sw = 2;
    switch (sw) {
    case 2:
        x += 1;
        __attribute__((fallthrough));
    case 3:
        x += 2;
        break;
    }
lbl: __attribute__((unused));
    printf("%zu %zu %zu %zu %zu\n", sizeof(packed_t), _Alignof(packed_t), sizeof(struct tagged),
           sizeof(struct trailing), sizeof(union pu));
    printf("%zu %zu %zu %zu\n", sizeof(struct member_packed),
           __builtin_offsetof(struct member_packed, d), sizeof(struct member_aligned),
           __builtin_offsetof(struct member_aligned, i));
    printf("%zu %zu %zu\n", sizeof(struct big_align), _Alignof(struct big_align), _Alignof(aligned_rec));
    printf("%d %d %d\n", (int)((unsigned long)&aligned_global % 64),
           (int)((unsigned long)&local_aligned % 32), (int)((unsigned long)bytes % 16));
    printf("%d %d %d %d %d\n", p.c, p.i, p.s, ai.v, x);
    p.i += 40;
    printf("%d %d %d %d %d %d\n", p.i, twice(3), readg(), inl(1), noinl(1), cold_path() + hot_path());
    printf("%d %d %d %d %d %d %d\n", fmt("A"), old(), old2(), weak_fn(), many(), spelled(), unknown());
    printf("%d %d %d %d\n", counter, aligned_global, in_section, weak_value);
    if (sw == 3)
        die(4);
    return unused_var;
}
