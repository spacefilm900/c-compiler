float idf(float x) { return x + 1.0f; }
double idd(double x) { return x + 1.0; }
float mixf(int n, float x) { return x + (float)n; }
double mixd(double x, int n) { return x + (double)n; }

struct FF {
    float a;
    float b;
};
struct FD {
    float f;
    double d;
};
struct ID {
    int i;
    double d;
};
struct DI {
    double d;
    int i;
};
struct D2 {
    double a;
    double b;
};
struct CD {
    char c;
    double d;
};
struct OnlyD {
    double d;
};

struct FF idff(struct FF s) {
    s.a += 1.0f;
    s.b += 2.0f;
    return s;
}
struct FD idfd(struct FD s) { return s; }
struct ID idid(struct ID s) { return s; }
struct DI iddi(struct DI s) { return s; }
struct D2 idd2(struct D2 s) { return s; }
struct CD idcd(struct CD s) { return s; }
double via_only(struct OnlyD s) { return s.d; }

double after_xmms(double a, double b, double c, double d, double e, double f,
                  double g, double h, struct OnlyD s) {
    return a + h + s.d;
}

double spill_pair(double a, double b, double c, double d, double e, double f,
                  double g, struct D2 s, float tail) {
    return g + s.a + s.b + (double)tail;
}

int many_scalar(float a, double b, int c, float d) {
    return (a == 1.5f) + (b == 2.5) + (c == 3) + (d == 4.5f);
}
