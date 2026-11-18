int printf(const char *, ...);

typedef union { long double d; void *p; } A;
typedef union { long double d; int i; } B;

void take(int a, A x, double f, B y, int b);
void many(double d0, double d1, double d2, double d3, double d4, double d5, double d6,
          A x, double d7, double d8, long tail);
A make(long v);
B make_b(int v);

int main(void) {
    A x = {0};
    B y = {0};
    x.p = (void *)0x1234;
    y.i = -7;
    take(1, x, 2.5, y, 3);
    many(1, 2, 3, 4, 5, 6, 7, x, 8, 9, 10);
    printf("make %p %d\n", make(0x4321).p, make_b(42).i);
    return 0;
}
