/* GNU forward-declared enums (issue #68): incomplete until the enumerator
 * list, then the enumerated type in every earlier use. `Neg` is defined
 * with a negative value, so its type is int (not the unsigned int a
 * nonnegative enumeration gets outside MSVC); `Pos` is nonnegative. */
int printf(const char *, ...);

typedef enum Neg Neg;
typedef enum Pos Pos;
typedef Neg *NegPtr;

extern Neg neg_obj;
Neg neg_tentative;
Pos *pos_ptr;
struct Holder {
    Neg *neg;
    Pos *pos;
};
Neg pick(int i);
int scale(Pos p);

enum Neg { N_MINUS = -1, N_ZERO, N_ONE };
enum Pos { P_ONE = 1, P_SEVEN = 7 };

Neg neg_obj = N_ONE;
static Pos pos_store = P_SEVEN;

Neg pick(int i) { return i < 0 ? N_MINUS : N_ONE; }
int scale(Pos p) { return (int)p * 3; }

int main(void) {
    NegPtr np = &neg_obj;
    struct Holder h = { &neg_tentative, &pos_store };
    pos_ptr = &pos_store;
    neg_tentative = pick(-5);
    printf("%d %d %d\n", (int)sizeof(Neg), (int)sizeof(Pos), (int)_Alignof(Pos));
    printf("%d %d %d\n", *np, *h.neg, *h.neg < 0);
    printf("%d %d\n", scale(*pos_ptr), scale(*h.pos - 6));
    printf("%d\n", (Neg)-3 < 0);
    return neg_tentative == N_MINUS ? 0 : 1;
}
