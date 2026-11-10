int printf(const char *, ...);

float gf = 1.5f;
double gd = 2.25;
static double gs = 1.0 + 0.5;
float ga[3] = {1.0f, 2.0f, 3.5f};

struct Pair {
    int i;
    double d;
};

struct Pair gp = {3, 4.5};

union Bits {
    float f;
    int i;
};

double add(double a, double b) {
    return a + b;
}

int main(void) {
    float f = 1.5f;
    double d = 2.5;
    if (f + 1.0f != 2.5f) {
        return 1;
    }
    if (d - 0.5 != 2.0) {
        return 2;
    }
    if (f * 2.0f != 3.0f) {
        return 3;
    }
    if (d / 2.0 != 1.25) {
        return 4;
    }
    if (!(f < d) || f > d || f == d) {
        return 5;
    }
    if ((-f) != -1.5f) {
        return 6;
    }
    if (!0.0 != 1 || !!1.5 != 1) {
        return 7;
    }
    if (0.0 && (f = 9.0f)) {
        return 8;
    }
    if (f != 1.5f) {
        return 9;
    }
    if ((d != 0.0) ? 0 : 1) {
        return 10;
    }
    f += 0.5f;
    if (f != 2.0f) {
        return 11;
    }
    if (++d != 3.5 || d-- != 3.5 || d != 2.5) {
        return 12;
    }
    int n = 0;
    while (d > 0.0) {
        d -= 1.0;
        n += 1;
    }
    if (n != 2) {
        return 13;
    }
    float acc = 0.0f;
    for (int i = 0; i < 3; i += 1) {
        acc += ga[i];
    }
    if (acc != 6.5f) {
        return 14;
    }
    if (gf != 1.5f || gd != 2.25 || gs != 1.5) {
        return 15;
    }
    if (gp.i != 3 || gp.d != 4.5) {
        return 16;
    }
    struct Pair local = {1, 2.0};
    local.d += local.i;
    if (local.d != 3.0) {
        return 17;
    }
    if ((int)2.9 != 2 || (int)-2.9 != -2) {
        return 18;
    }
    if ((double)3 != 3.0 || (float)4 != 4.0f) {
        return 19;
    }
    double sum = add(1.25, 2.75);
    if (sum != 4.0) {
        return 20;
    }
    union Bits bits;
    bits.f = 1.0f;
    if (bits.i != 0x3f800000) {
        return 21;
    }
    static float sf = 0.5f;
    sf += 0.5f;
    if (sf != 1.0f) {
        return 22;
    }
    int seen = 0;
    double z = 0.0;
    if (z) {
        seen = 1;
    }
    if (seen || !z != 1) {
        return 23;
    }
    printf("%.1f %.2f\n", f, gd);
    return 0;
}
