#include <stdio.h>
#include <stdlib.h>
extern void abort();
int f();
int f(int x) { return x + 1; }
double twice();
static int count();
int sum();
double g(double x) { return x * 2; }
int leave() { return 7; }
int main() {
    double (*p)();
    int (*q)(int) = leave;
    p = &g;
    int r = f(1) - 2;
    r += (int)p(2.0) - 4;
    r += (int)twice(1.5f) - 3;
    r += count() - 0;
    r += sum(3, 'a', (short)2, 1.0f) - 103;
    r += q(5) - 7;
    {
        int h();
        r += h(4) - 8;
    }
    if (r) abort();
    printf("%d\n", r);
    return r;
}
double twice(double x) { return x * 2; }
static int count(void) { return 0; }
int sum(int n, int c, int s, double d) { return n + c + s + (int)d; }
int h(int v) { return v * 2; }
