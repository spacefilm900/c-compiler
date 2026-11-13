/* GNU C: case ranges (GCC manual, "Case Ranges"): `case low ... high:`
   matches every value from low to high inclusive. */

int printf(const char *, ...);

static int classify(int c)
{
    switch (c) {
    case '0' ... '9':
        return 1;
    case 'a' ... 'z':
    case 'A' ... 'Z':
        return 2;
    case -100 ... -1:
        return 3;
    case 1000:
        return 4;
    case 1001 ... 1001:
        return 5;
    default:
        return 0;
    }
}

static int digits(unsigned long long n)
{
    switch (n) {
    case 0 ... 9: return 1;
    case 10 ... 99: return 2;
    case 100 ... 999999999999ULL: return 3;
    case 1000000000000ULL ... 18446744073709551615ULL: return 4;
    }
    return -1;
}

static const char *grade(unsigned char score)
{
    switch (score) {
    case 90 ... 255: return "A";
    case 80 ... 89: return "B";
    case 0 ... 79: return "C";
    }
    return "?";
}

int main(void)
{
    const int samples[] = {'5', 'q', 'Q', -50, -1, 0, 1000, 1001, 1002, -101, '@'};
    for (unsigned i = 0; i < sizeof samples / sizeof samples[0]; i++)
        printf("%d ", classify(samples[i]));
    printf("\n%d %d %d %d %d\n", digits(7), digits(42), digits(123456), digits(1ULL << 50),
           digits(18446744073709551615ULL));
    printf("%s %s %s\n", grade(95), grade(85), grade(10));
    long x = 3000000000L;
    switch (x) {
    case 2000000000L ... 4000000000L:
        printf("big\n");
        break;
    default:
        printf("small\n");
    }
    return 0;
}
