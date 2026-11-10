#include <float.h>
#include <stdio.h>

int main(void) {
    printf("%a\n", (double)FLT_MAX);
    printf("%a\n", (double)FLT_MIN);
    printf("%a\n", (double)FLT_EPSILON);
    printf("%a\n", (double)FLT_TRUE_MIN);
    printf("%a\n", DBL_MAX);
    printf("%a\n", DBL_MIN);
    printf("%a\n", DBL_EPSILON);
    printf("%a\n", DBL_TRUE_MIN);
    printf("%d %d %d\n", FLT_RADIX, FLT_ROUNDS, FLT_EVAL_METHOD);
    printf("%d %d %d\n", FLT_MANT_DIG, DBL_MANT_DIG, LDBL_MANT_DIG);
    return 0;
}
