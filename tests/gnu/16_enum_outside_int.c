/* GNU C: enumerators outside int (a constraint violation in C11 6.7.2.2p2).
   While the list is read, such a constant has the type of its value; after
   it, the enumerated type is unsigned int or unsigned long when no value is
   negative, else long. The type of a constant of such a list that fits in
   int is not printed: GCC and Apple clang 21 give it the enumerated type,
   Clang 19 keeps int. The
   macOS SDK's <mach_debug/ipc_info.h> declares IPC_OTYPE_UNKNOWN = ~0u in
   an enum on Intel and Apple Silicon alike; SQLite reaches it. */
int printf(const char *, ...);

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

/* An implicit constant after one outside int has, while the list is read,
   the previous constant's type if the value fits, else long after a signed
   one and unsigned long after an unsigned one (GCC and Clang alike). */
enum V { V0 = 0x80000000L, V1, V2 = _Generic(V1, int: 1, unsigned: 2, long: 3, unsigned long: 4) };
enum W { W0 = ~0u, W1, W2 = _Generic(W1, int: 1, unsigned: 2, long: 3, unsigned long: 4) };
enum X { X0 = 0x7fffffff, X1, X2 = _Generic(X1, int: 1, unsigned: 2, long: 3, unsigned long: 4) };
enum Y { Y0 = 0xfffffffeu, Y1, Y2 = _Generic(Y1, int: 1, unsigned: 2, long: 3, unsigned long: 4) };

/* In GNU mode a colon after `enum E` starts an underlying type only when a
   type follows: here it is an unnamed bit-field and a _Generic association. */
struct Z { enum E : 3; int z; };
static int pick(enum E e) { return _Generic(e, enum E: 1, default: 2); }

static unsigned long pass(enum E e) { return e; }

/* The type of an expression as a number: 1 int, 2 unsigned int, 3 long,
   4 unsigned long. */
static void show(const char *what, int type, unsigned long size) {
    printf("%s %d %lu\n", what, type, size);
}

int main(void) {
    enum E e = E1;
    U u = U1;
    show("E1", _Generic(E1, int: 1, unsigned: 2, long: 3, unsigned long: 4), sizeof(enum E));
    show("e", _Generic(e, int: 1, unsigned: 2, long: 3, unsigned long: 4), sizeof e);
    printf("E %d %lu %d\n", E0, pass(E1), e > 0);
    show("F1", _Generic(F1, int: 1, unsigned: 2, long: 3, unsigned long: 4), sizeof(enum F));
    printf("F %d %ld\n", (int)F0, (long)F0);
    show("G0", _Generic(G0, int: 1, unsigned: 2, long: 3, unsigned long: 4), sizeof(enum G));
    printf("G %lu\n", (unsigned long)G0);
    show("I1", _Generic(I1, int: 1, unsigned: 2, long: 3, unsigned long: 4), sizeof(enum I));
    show("I2", _Generic(I2, int: 1, unsigned: 2, long: 3, unsigned long: 4), sizeof I2);
    printf("I %d %lu\n", (int)I0, (unsigned long)I2);
    show("M1", _Generic(M1, int: 1, unsigned: 2, long: 3, unsigned long: 4), sizeof(enum M));
    printf("M %d %ld\n", (int)M0, (long)M1);
    show("N0", _Generic(N0, int: 1, unsigned: 2, long: 3, unsigned long: 4), sizeof N0);
    printf("N %d %d\n", N1, N2);
    show("P1", _Generic(P1, int: 1, unsigned: 2, long: 3, unsigned long: 4), sizeof(enum P));
    show("Q0", _Generic(Q0, int: 1, unsigned: 2, long: 3, unsigned long: 4), sizeof(enum Q));
    printf("Q %lu\n", (unsigned long)Q0);
    show("R1", _Generic(R1, int: 1, unsigned: 2, long: 3, unsigned long: 4), sizeof(enum R));
    show("S0", _Generic(S0, int: 1, unsigned: 2, long: 3, unsigned long: 4), sizeof(enum S));
    printf("R %ld S %d\n", (long)R0, (int)S1);
    show("u", _Generic(u, int: 1, unsigned: 2, long: 3, unsigned long: 4), sizeof u);
    printf("U %u %d\n", u, (int)U0);
    printf("V %d W %d X %d Y %d\n", (int)V2, (int)W2, (int)X2, (int)Y2);
    printf("Z %lu %d\n", (unsigned long)sizeof(struct Z), pick(E0));
    return 0;
}
