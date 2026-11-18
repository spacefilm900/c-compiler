/* GNU C: enumerators outside int (a constraint violation in C11 6.7.2.2p2).
   While the list is read, such a constant has the type of its value; after
   it, the enumerated type is unsigned int or unsigned long when no value is
   negative, else long. The type of a constant of such a list that fits in
   int is not printed: GCC and Apple clang 21 give it the enumerated type,
   Clang 19 keeps int. The
   macOS SDK's <mach_debug/ipc_info.h> declares IPC_OTYPE_UNKNOWN = ~0u in
   an enum on Intel and Apple Silicon alike; SQLite reaches it. */
int printf(const char *, ...);

#define T(x) _Generic((x), int: "int", unsigned: "unsigned", long: "long", \
                      unsigned long: "ulong", default: "other")

enum E { E0 = 0, E1 = ~0u };
enum F { F0 = -1, F1 = 0x80000000u };
enum G { G0 = 0x100000000 };
enum I { I0 = 1, I1 = 0xffffffffu, I2 };
enum M { M0 = 0x7fffffff, M1 };
enum N { N0 = ~0u, N1 = _Generic(N0, unsigned: 1, default: 2), N2 = sizeof(N0) };
enum P { P0 = -1, P1 = 0x7fffffffffffffffL };
enum Q { Q0 = 0xffffffffffffffffUL };
enum R { R0 = -2, R1 = 0xffffffffu };
enum S { S0 = 5000000000, S1 = 1 };
typedef enum { U0 = 3, U1 = 0xfffffffeu } U;

static unsigned long pass(enum E e) { return e; }

int main(void) {
    enum E e = E1;
    U u = U1;
    printf("E %d %s %zu %s %lu %d\n", E0, T(E1), sizeof(enum E), T(e), pass(E1), e > 0);
    printf("F %d %s %zu %ld\n", (int)F0, T(F1), sizeof(enum F), (long)F0);
    printf("G %s %zu %lu\n", T(G0), sizeof(enum G), (unsigned long)G0);
    printf("I %d %s %s %zu %lu\n", (int)I0, T(I1), T(I2), sizeof(enum I), (unsigned long)I2);
    printf("M %d %s %zu %ld\n", (int)M0, T(M1), sizeof(enum M), (long)M1);
    printf("N %d %d %s\n", N1, N2, T(N0));
    printf("P %s %zu Q %s %zu %lu\n", T(P1), sizeof(enum P), T(Q0), sizeof(enum Q),
           (unsigned long)Q0);
    printf("R %s %zu %ld S %s %d %zu\n", T(R1), sizeof(enum R), (long)R0, T(S0), (int)S1,
           sizeof(enum S));
    printf("U %s %zu %u %d\n", T(u), sizeof u, u, (int)U0);
    return 0;
}
