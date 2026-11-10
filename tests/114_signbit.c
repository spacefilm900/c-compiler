int printf(const char *, ...);

int main(void) {
    float f = -1.5f;
    double d = 2.0;
    long double l = -0.0L;
    printf("%d %d %d\n", __builtin_signbitf(f), __builtin_signbit(d), __builtin_signbitl(l));
    printf("%d %d\n", __builtin_signbit(0.0), __builtin_signbit(-0.0));
    printf("%d %d %d\n", __builtin_signbitf(1.0f), __builtin_signbit(-2.0), __builtin_signbitl(3.0L));
    return 0;
}
