/* GNU forward-declared enums (issue #68): incomplete until the enumerator
 * list, then the enumerated type in every earlier use. The definitions
 * choose int (a negative value) and a 64-bit type (a value outside int). */
#include <stdio.h>

typedef enum Neg Neg;
typedef enum Wide Wide;
typedef Neg *NegPtr;

extern Neg neg_obj;
Neg neg_tentative;
Wide *wide_ptr;
struct Holder {
    Neg *neg;
    Wide *wide;
};
Neg pick(int i);
long widen(Wide w);

enum Neg { N_MINUS = -1, N_ZERO, N_ONE };
enum Wide { W_SMALL = 1, W_BIG = 0x100000000 };

Neg neg_obj = N_ONE;
static Wide wide_store = W_BIG;

Neg pick(int i) { return i < 0 ? N_MINUS : N_ONE; }
long widen(Wide w) { return (long)(w >> 16); }

int main(void) {
    NegPtr np = &neg_obj;
    struct Holder h = { &neg_tentative, &wide_store };
    wide_ptr = &wide_store;
    neg_tentative = pick(-5);
    printf("%d %d %d\n", (int)sizeof(Neg), (int)sizeof(Wide), (int)_Alignof(Wide));
    printf("%d %d %d\n", *np, *h.neg, *h.neg < 0);
    printf("%ld %ld\n", widen(*wide_ptr), widen(*h.wide - 1));
    printf("%d\n", (Neg)-3 < 0);
    return neg_tentative == N_MINUS ? 0 : 1;
}
