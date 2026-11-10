float idf(float x);
double idd(double x);
float mixf(int n, float x);
double mixd(double x, int n);

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

struct FF idff(struct FF s);
struct FD idfd(struct FD s);
struct ID idid(struct ID s);
struct DI iddi(struct DI s);
struct D2 idd2(struct D2 s);
struct CD idcd(struct CD s);
double via_only(struct OnlyD s);
double after_xmms(double a, double b, double c, double d, double e, double f,
                  double g, double h, struct OnlyD s);
double spill_pair(double a, double b, double c, double d, double e, double f,
                  double g, struct D2 s, float tail);
int many_scalar(float a, double b, int c, float d);

int main(void) {
    if (idf(1.5f) != 2.5f) {
        return 1;
    }
    if (idd(2.25) != 3.25) {
        return 2;
    }
    if (mixf(2, 0.5f) != 2.5f) {
        return 3;
    }
    if (mixd(1.5, 4) != 5.5) {
        return 4;
    }
    struct FF ff = {1.0f, 2.0f};
    ff = idff(ff);
    if (ff.a != 2.0f || ff.b != 4.0f) {
        return 5;
    }
    struct FD fd = {1.5f, 2.5};
    fd = idfd(fd);
    if (fd.f != 1.5f || fd.d != 2.5) {
        return 6;
    }
    struct ID id = {7, 8.5};
    id = idid(id);
    if (id.i != 7 || id.d != 8.5) {
        return 7;
    }
    struct DI di = {1.25, 9};
    di = iddi(di);
    if (di.d != 1.25 || di.i != 9) {
        return 8;
    }
    struct D2 d2 = {1.0, 2.0};
    d2 = idd2(d2);
    if (d2.a != 1.0 || d2.b != 2.0) {
        return 9;
    }
    struct CD cd = {3, 4.0};
    cd = idcd(cd);
    if (cd.c != 3 || cd.d != 4.0) {
        return 10;
    }
    struct OnlyD only = {6.5};
    if (via_only(only) != 6.5) {
        return 11;
    }
    if (after_xmms(1, 0, 0, 0, 0, 0, 0, 2, only) != 9.5) {
        return 12;
    }
    if (spill_pair(0, 0, 0, 0, 0, 0, 1.5, d2, 0.5f) != 5.0) {
        return 13;
    }
    if (many_scalar(1.5f, 2.5, 3, 4.5f) != 4) {
        return 14;
    }
    return 0;
}
