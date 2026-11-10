#include <string.h>

int sum_ints(int n, ...);
double last_double(int n, ...);
struct Pair {
    int a;
    int b;
};
int pair_sum(int tag, ...);
int pick_ptr(int tag, ...);

int main(void) {
    if (sum_ints(3, 10, 20, 30) != 60) {
        return 1;
    }
    double d = last_double(3, 1.0, 2.0, 4.5);
    unsigned long long bits = 0;
    memcpy(&bits, &d, sizeof bits);
    if (bits != 0x4012000000000000ULL) {
        return 2;
    }
    struct Pair p;
    p.a = 8;
    p.b = 9;
    if (pair_sum(1, p) != 17) {
        return 3;
    }
    int cell = 5;
    if (pick_ptr(1, &cell, 6) != 11) {
        return 4;
    }
    return 0;
}
