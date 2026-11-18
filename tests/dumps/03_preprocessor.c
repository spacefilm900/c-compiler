/* Golden dump test: macros, conditional inclusion, constants of every
   kind, enumerations, sizeof and _Generic. */
#define SQUARE(x) ((x) * (x))
#define NAME(n) #n
#define JOIN(a, b) a##b
#define LIMIT 0x10u

#if defined(LIMIT) && LIMIT > 8
#define BIG 1
#else
#define BIG 0
#endif

enum colour { RED, GREEN = 5, BLUE };

typedef unsigned long size_type;

static const char *label = NAME(hello);

int pick(int c) {
    switch (c) {
    case RED:
        return 'r';
    case GREEN:
    case BLUE:
        return SQUARE(c);
    default:
        return _Generic(c, int: 1, default: 2);
    }
}

int main(void) {
    size_type n = sizeof(long double) + sizeof label;
    long double ld = 1.25L;
    unsigned long long big = 18446744073709551615ULL;
    int JOIN(var, 1) = BIG + L'x' - 'x';
    return pick(BLUE) - 36 + (int)(n - 24) + (ld < 1) + (int)(big >> 63) - var1 + (int)LIMIT - 16;
}
