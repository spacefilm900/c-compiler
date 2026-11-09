#if (1 + 2) * 3 == 9
#define P 1
#else
#define P 0
#endif

#if -1 < 1u
#define Q 1
#else
#define Q 0
#endif

#ifndef MISSING
#define R 1
#else
#define R 0
#endif

#if 0
#error should be skipped
#elif 1
#define S 2
#else
#define S 0
#endif

int main(void) {
    return (P == 1 && Q == 0 && R == 1 && S == 2) ? 0 : 1;
}
