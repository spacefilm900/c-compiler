/* GNU C: an enumeration with a fixed underlying type (C23 6.7.2.2; Clang
   and GCC accept it in C as an extension, strict C11 has no such syntax).
   The constants and the enumerated type have the underlying type. The
   macOS SDK's <malloc/malloc.h> declares
   `typedef enum : uint64_t { ... } malloc_zone_malloc_options_t;`, and
   SQLite reaches it. */
typedef unsigned long long u64;
int printf(const char *, ...);

#define T(x) _Generic((x), char: "char", signed char: "schar", unsigned char: "uchar", \
                      short: "short", unsigned short: "ushort", int: "int",         \
                      unsigned: "unsigned", long: "long", unsigned long: "ulong",    \
                      long long: "llong", unsigned long long: "ullong", default: "other")

typedef enum : u64 { A0 = 0u, A1 = 1u << 0, A2 = 1u << 1, } A;
enum B : unsigned char { B0, B1 = 200, B2 };
enum C : short { C0 = -5, C1 };
enum D : long long { D0 = 0x7fffffff, D1 };
enum : const unsigned { K0 = 0xfffffff0u, K1 };

static u64 take(A a) { return a; }

int main(void) {
    A a = A2;
    enum B b = B2;
    printf("A %s %s %zu %s %llu\n", T(A0), T(a), sizeof(A), T(A2 | A1), take(A1 | A2));
    printf("B %s %zu %d %s %d\n", T(B1), sizeof(enum B), B2, T(b), (int)sizeof(B0));
    printf("C %s %ld D %s %lld %zu\n", T(C1), (long)C1, T(D1), (long long)D1, sizeof D0);
    printf("K %s %u\n", T(K1), K1);
    return 0;
}
