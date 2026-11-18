/* GNU C: an enumeration with a fixed underlying type (C23 6.7.2.2; Clang
   and GCC accept it in C as an extension, strict C11 has no such syntax).
   The constants and the enumerated type have the underlying type. The
   macOS SDK's <malloc/malloc.h> declares
   `typedef enum : uint64_t { ... } malloc_zone_malloc_options_t;`, and
   SQLite reaches it. */
typedef unsigned long long u64;
int printf(const char *, ...);

typedef enum : u64 { A0 = 0u, A1 = 1u << 0, A2 = 1u << 1, } A;
enum B : unsigned char { B0, B1 = 200, B2 };
enum C : short { C0 = -5, C1 };
enum D : long long { D0 = 0x7fffffff, D1 };
enum : const unsigned { K0 = 0xfffffff0u, K1 };

/* The underlying type is in force while the list is read. */
enum L : long { L0 = sizeof(enum L), L1 = _Generic(L0, long: 1, default: 2) };

static u64 take(A a) { return a; }

/* The type of an expression as a number: 1 unsigned char, 2 short,
   3 unsigned int, 4 long long, 5 unsigned long long. */
static void show(const char *what, int type, unsigned long size) {
    printf("%s %d %lu\n", what, type, size);
}

int main(void) {
    A a = A2;
    enum B b = B2;
    show("A0", _Generic(A0, unsigned char: 1, short: 2, unsigned: 3, long long: 4, unsigned long long: 5), sizeof(A));
    show("a", _Generic(a, unsigned char: 1, short: 2, unsigned: 3, long long: 4, unsigned long long: 5), sizeof a);
    show("A2|A1", _Generic(A2 | A1, unsigned char: 1, short: 2, unsigned: 3, long long: 4, unsigned long long: 5), sizeof(A2 | A1));
    printf("A %llu\n", take(A1 | A2));
    show("B1", _Generic(B1, unsigned char: 1, short: 2, unsigned: 3, long long: 4, unsigned long long: 5), sizeof(enum B));
    show("b", _Generic(b, unsigned char: 1, short: 2, unsigned: 3, long long: 4, unsigned long long: 5), sizeof b);
    printf("B %d %d\n", B2, (int)sizeof(B0));
    show("C1", _Generic(C1, unsigned char: 1, short: 2, unsigned: 3, long long: 4, unsigned long long: 5), sizeof C1);
    show("D1", _Generic(D1, unsigned char: 1, short: 2, unsigned: 3, long long: 4, unsigned long long: 5), sizeof D0);
    printf("C %ld D %lld\n", (long)C1, (long long)D1);
    show("K1", _Generic(K1, unsigned char: 1, short: 2, unsigned: 3, long long: 4, unsigned long long: 5), sizeof K1);
    printf("K %u L %d %d\n", K1, (int)L0, (int)L1);
    return 0;
}
