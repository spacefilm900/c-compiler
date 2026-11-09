#define A 2
#define B A + A
#define C (A + A)
#define ID(x) x
#define ADD(a, b) ((a) + (b))
#define MUL(a, b) ((a) * (b))

int main(void) {
    return MUL(ADD(ID(1), 2), C) + (B * 2 == 6 ? 0 : 100);
}
