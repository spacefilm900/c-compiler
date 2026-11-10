#include <math.h>
#include <stdio.h>

int main(void) {
    float f = -1.5f;
    double d = 2.0;
    long double l = -0.0L;
    printf("%d %d %d\n", __builtin_signbitf(f), __builtin_signbit(d), __builtin_signbitl(l));
    printf("%d %d\n", signbit(0.0), signbit(-0.0));
    printf("%d %d %d\n", signbit(1.0f), signbit(-2.0), signbit(3.0L));
    return 0;
}
